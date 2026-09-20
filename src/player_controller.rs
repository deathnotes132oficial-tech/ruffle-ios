use std::cell::{Cell, OnceCell, RefCell};
use std::fs::File;
use std::path::Path;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use std::{fmt, io};

use block2::RcBlock;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, sel, DefinedClass as _, MainThreadOnly, Message};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{
    MainThreadMarker, NSBundle, NSCoder, NSObjectProtocol, NSRunLoop, NSString,
};
use objc2_ui_kit::{
    UIButton, UIButtonType, UIColor, UIControlEvents, UIControlState,
    UIInterfaceOrientationMask, UINavigationController, UIViewController,
};
use ruffle_core::backend::navigator::OwnedFuture;
use ruffle_core::backend::storage::StorageBackend;
use ruffle_core::config::Letterbox;
use ruffle_core::events::{KeyDescriptor, KeyLocation, LogicalKey, PhysicalKey};
use ruffle_core::{LoadBehavior, Player, PlayerBuilder, PlayerEvent};
use ruffle_frontend_utils::backends::audio::CpalAudioBackend;
use ruffle_frontend_utils::backends::navigator::{
    self, ExternalNavigatorBackend, NavigatorInterface,
};
use ruffle_frontend_utils::content::PlayingContent;
use ruffle_frontend_utils::player_options::PlayerOptions;
use ruffle_render::quality::StageQuality;
use url::Url;

use crate::player_view::PlayerView;
use crate::storage::{self, Movie, SecurityScopedResource};

#[derive(Clone, Debug)]
pub struct FutureSpawner {
    mtm: MainThreadMarker,
    main_run_loop: Retained<NSRunLoop>,
}

impl FutureSpawner {
    fn run_later(&self, closure: impl FnOnce() + 'static) {
        let cell = Cell::new(Some(closure));

        let _ = self.mtm;
        // SAFTY: We hold MainThreadMarker, so it's fine to send a non-send
        // closures to be run later on the main thread.
        unsafe {
            self.main_run_loop.performBlock(&RcBlock::new(move || {
                let closure = cell.take().expect("called twice");
                closure();
            }))
        };
    }
}

impl<E: std::error::Error + 'static> navigator::FutureSpawner<E> for FutureSpawner {
    fn spawn(&self, future: OwnedFuture<(), E>) {
        // Discard any errors.
        let future = async {
            if let Err(e) = future.await {
                tracing::error!("Async error: {}", e);
            }
        };

        let scheduler = move |task: async_task::Runnable| {
            self.run_later(|| {
                task.run();
            });
        };

        // SAFETY: TODO
        let (runnable, task) = unsafe { async_task::spawn_unchecked(future, scheduler) };

        // The future should run in the background.
        task.detach();
        // Immediately schedule the future to be polled for the first time.
        runnable.schedule();
    }
}

#[derive(Default)]
pub struct Ivars {
    // Populated to be used in `viewDidLoad`.
    content: Cell<Option<PlayingContent>>,
    user_options: Cell<Option<PlayerOptions>>,
    storage_backend: Cell<Option<Box<dyn StorageBackend>>>,

    /// Used to keep the bundle resource alive while we're using it.
    _scoped_resource: Cell<Option<SecurityScopedResource>>,

    player: OnceCell<Arc<Mutex<Player>>>,

    /// Os botoes de toque, guardados pra poder recoloca-los quando a tela
    /// muda de tamanho (girar o aparelho, por exemplo).
    controles: RefCell<Vec<Retained<UIButton>>>,
}

impl fmt::Debug for Ivars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ivars").finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct Navigator;

impl NavigatorInterface for Navigator {
    fn navigate_to_website(&self, _url: Url) {}

    async fn open_file(&self, path: &Path) -> io::Result<File> {
        tracing::info!("trying to open: {path:?}");
        File::open(path)
    }

    async fn confirm_socket(&self, _host: &str, _port: u16) -> bool {
        true
    }
}

define_class!(
    #[unsafe(super(UIViewController))]
    #[name = "PlayerController"]
    #[ivars = Ivars]
    #[derive(Debug)]
    pub struct PlayerController;

    unsafe impl NSObjectProtocol for PlayerController {}

    /// UIViewController.
    impl PlayerController {
        #[unsafe(method_id(initWithNibName:bundle:))]
        fn _init_with_nib_name_bundle(
            this: Allocated<Self>,
            nib_name_or_nil: Option<&NSString>,
            nib_bundle_or_nil: Option<&NSBundle>,
        ) -> Retained<Self> {
            tracing::info!("player controller init");
            let this = this.set_ivars(Ivars::default());
            unsafe {
                msg_send![super(this), initWithNibName: nib_name_or_nil, bundle: nib_bundle_or_nil]
            }
        }

        #[unsafe(method_id(initWithCoder:))]
        fn _init_with_coder(this: Allocated<Self>, coder: &NSCoder) -> Option<Retained<Self>> {
            tracing::info!("player controller init");
            let this = this.set_ivars(Ivars::default());
            unsafe { msg_send![super(this), initWithCoder: coder] }
        }

        #[unsafe(method(loadView))]
        fn _load_view(&self) {
            self.load_view();
        }

        #[unsafe(method(viewDidLoad))]
        fn _view_did_load(&self) {
            // Xcode template calls super at the beginning
            let _: () = unsafe { msg_send![super(self), viewDidLoad] };
            self.view_did_load();
        }

        #[unsafe(method(viewIsAppearing:))]
        fn _view_is_appearing(&self, animated: bool) {
            self.view_is_appearing(animated);
            // Docs say to call super
            let _: () = unsafe { msg_send![super(self), viewIsAppearing: animated] };
        }

        #[unsafe(method(viewDidLayoutSubviews))]
        fn _view_did_layout_subviews(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidLayoutSubviews] };
            self.posicionar_controles();
        }

        #[unsafe(method(viewWillDisappear:))]
        fn _view_will_disappear(&self, animated: bool) {
            self.view_will_disappear(animated);
            // Docs say to call super
            let _: () = unsafe { msg_send![super(self), viewWillDisappear: animated] };
        }

        #[unsafe(method(viewDidDisappear:))]
        fn _view_did_disappear(&self, animated: bool) {
            self.view_did_disappear(animated);
            // Docs say to call super
            let _: () = unsafe { msg_send![super(self), viewDidDisappear: animated] };
        }
    }

    /// Os botoes de toque chamam estes dois.
    #[allow(non_snake_case)]
    impl PlayerController {
        #[unsafe(method(controleApertado:))]
        fn controleApertado(&self, botao: &UIButton) {
            self.mandar_tecla(botao.tag(), true);
        }

        #[unsafe(method(controleSolto:))]
        fn controleSolto(&self, botao: &UIButton) {
            self.mandar_tecla(botao.tag(), false);
        }

        // JOGO E DEITADO.
        //
        // O DDTank tem 1000 por 600: em pe ele ocupa uma faixa fina no meio e
        // os botoes nao cabem. Aqui a gente pede ao sistema pra so aceitar
        // deitado enquanto o jogo esta aberto.
        //
        // Isto sozinho pode nao bastar: o Info.plist tem a palavra final sobre
        // o que o aplicativo inteiro aceita. Os dois precisam concordar.
        #[unsafe(method(supportedInterfaceOrientations))]
        fn supportedInterfaceOrientations(&self) -> UIInterfaceOrientationMask {
            UIInterfaceOrientationMask::Landscape
        }

        #[unsafe(method(shouldAutorotate))]
        fn shouldAutorotate(&self) -> bool {
            true
        }
    }

    /// UIResponder
    #[allow(non_snake_case)]
    impl PlayerController {
        #[unsafe(method(canBecomeFirstResponder))]
        fn canBecomeFirstResponder(&self) -> bool {
            true
        }

        #[unsafe(method(becomeFirstResponder))]
        fn becomeFirstResponder(&self) -> bool {
            tracing::info!("player controller becomeFirstResponder");
            self.view().becomeFirstResponder();
            true
        }

        #[unsafe(method(canResignFirstResponder))]
        fn canResignFirstResponder(&self) -> bool {
            true
        }

        #[unsafe(method(resignFirstResponder))]
        fn resignFirstResponder(&self) -> bool {
            tracing::info!("controller resignFirstResponder");
            true
        }
    }
);

impl PlayerController {
    /// For use by run_swf.rs
    pub fn new(
        mtm: MainThreadMarker,
        content: PlayingContent,
        options: PlayerOptions,
    ) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(Ivars {
            content: Cell::new(Some(content)),
            user_options: Cell::new(Some(options)),
            storage_backend: Cell::new(None),
            // run_swf.rs doesn't need security scoping.
            _scoped_resource: Cell::new(None),
            player: OnceCell::new(),
            controles: RefCell::new(Vec::new()),
        });
        let nil = None::<&AnyObject>;
        unsafe { msg_send![super(this), initWithNibName: nil, bundle: nil] }
    }

    pub fn empty(mtm: MainThreadMarker) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(Default::default());
        let nil = None::<&AnyObject>;
        unsafe { msg_send![super(this), initWithNibName: nil, bundle: nil] }
    }

    /// Prepare the controller for playing the given movie.
    pub fn setup_movie(&self, movie: &Movie) {
        let nsurl = movie.link();

        self.ivars()
            .content
            .set(Some(storage::get_playing_content(&nsurl)));
        self.ivars().user_options.set(Some(movie.user_options()));
        self.ivars()
            .storage_backend
            .set(Some(Box::new(storage::MovieStorageBackend {
                movie: movie.retain(),
            })));
        self.ivars()._scoped_resource.set(if nsurl.isFileURL() {
            Some(SecurityScopedResource::access(&nsurl).expect("failed accessing NSURL"))
        } else {
            None
        });
    }

    fn load_view(&self) {
        tracing::info!("player loadView");
        let mtm = MainThreadMarker::from(self);
        let view = PlayerView::initWithFrame(
            mtm.alloc(),
            CGRect::new(CGPoint::ZERO, CGSize::new(1.0, 1.0)),
        );
        self.setView(Some(&view));
    }

    fn view_did_load(&self) {
        tracing::info!("player viewDidLoad");

        // TODO: Specify safe area somehow
        let view = self.view();
        let renderer = view.create_renderer();

        let future_spawner = FutureSpawner {
            mtm: self.mtm(),
            main_run_loop: NSRunLoop::mainRunLoop(),
        };

        let content = self.ivars().content.take().unwrap();

        let player_options = self.ivars().user_options.take().unwrap();
        let player_options = match &content {
            PlayingContent::DirectFile(_) => player_options.clone(),
            PlayingContent::Bundle(_, bundle) => player_options.or(&bundle.information().player),
        };

        let movie_url = content.initial_swf_url().clone();
        let navigator = ExternalNavigatorBackend::new(
            player_options
                .base
                .to_owned()
                .unwrap_or_else(|| movie_url.clone()),
            player_options.referer.clone(),
            player_options.cookie.clone(),
            future_spawner,
            None,
            player_options.upgrade_to_https.unwrap_or_default(),
            Default::default(),
            ruffle_core::backend::navigator::SocketMode::Allow,
            Rc::new(content),
            Navigator,
        );

        let mut builder = PlayerBuilder::new()
            .with_renderer(renderer)
            .with_navigator(navigator)
            .with_letterbox(player_options.letterbox.unwrap_or(Letterbox::On))
            .with_max_execution_duration(
                player_options
                    .max_execution_duration
                    .unwrap_or(Duration::MAX),
            )
            .with_quality(player_options.quality.unwrap_or(StageQuality::High))
            .with_align(
                player_options.align.unwrap_or_default(),
                player_options.force_align.unwrap_or_default(),
            )
            // FORCAR a escala: o DDTank manda o palco nao redimensionar
            // (scaleMode = NoScale) logo no comeco. Sem forcar, ele aparece em
            // tamanho original, cortado na direita e com faixa preta embaixo —
            // foi o que a primeira foto do simulador mostrou.
            .with_scale_mode(
                player_options.scale.unwrap_or_default(),
                player_options.force_scale.unwrap_or(true),
            )
            .with_load_behavior(
                player_options
                    .load_behavior
                    .unwrap_or(LoadBehavior::Streaming),
            )
            .with_spoofed_url(player_options.spoof_url.clone().map(|url| url.to_string()))
            .with_page_url(player_options.spoof_url.clone().map(|url| url.to_string()))
            .with_player_version(player_options.player_version)
            .with_player_runtime(player_options.player_runtime.unwrap_or_default())
            .with_frame_rate(player_options.frame_rate);

        if player_options.dummy_external_interface.unwrap_or_default() {
            // TODO
        }

        match CpalAudioBackend::new(None) {
            Ok(audio) => builder = builder.with_audio(audio),
            Err(e) => tracing::error!("Unable to create audio device: {e}"),
        }

        if let Some(storage) = self.ivars().storage_backend.take() {
            builder = builder.with_storage(storage);
        }

        let player = builder.build();

        let mut player_lock = player.lock().unwrap();
        player_lock.fetch_root_movie(
            movie_url.to_string(),
            player_options.parameters.to_owned(),
            Box::new(|metadata| {
                eprintln!("got movie: {metadata:?}");
            }),
        );
        drop(player_lock);

        self.criar_controles();

        view.set_player(player.clone());
        self.ivars()
            .player
            .set(player)
            .unwrap_or_else(|_| panic!("viewDidLoad once"));
    }

    fn view_is_appearing(&self, _animated: bool) {
        // A barra de navegacao come uma faixa do alto da tela e fica por cima
        // do jogo. Quem joga nao precisa dela; pra voltar, basta o gesto de
        // arrastar da borda esquerda.
        if let Some(nav) = self.navigationController() {
            unsafe { nav.setNavigationBarHidden_animated(true, _animated) };
        }
        self.posicionar_controles();

        tracing::info!("player viewIsAppearing:");

        self.view().start();
    }

    fn view_will_disappear(&self, _animated: bool) {
        tracing::info!("player viewWillDisappear:");

        self.view().stop();
    }

    fn view_did_disappear(&self, _animated: bool) {
        tracing::info!("player viewDidDisappear:");

        self.view().flush();
    }

    /// As teclas dos botoes, na ordem em que eles sao criados.
    ///
    /// Sao as mesmas do APK de Android: 1 2 3 4 / Z X C / setas / espaco.
    fn tabela_de_teclas() -> &'static [(&'static str, PhysicalKey, char)] {
        &[
            ("1", PhysicalKey::Digit1, '1'),
            ("2", PhysicalKey::Digit2, '2'),
            ("3", PhysicalKey::Digit3, '3'),
            ("4", PhysicalKey::Digit4, '4'),
            ("Z", PhysicalKey::KeyZ, 'z'),
            ("X", PhysicalKey::KeyX, 'x'),
            ("C", PhysicalKey::KeyC, 'c'),
            ("\u{2190}", PhysicalKey::ArrowLeft, '\u{0}'),
            ("\u{2191}", PhysicalKey::ArrowUp, '\u{0}'),
            ("\u{2192}", PhysicalKey::ArrowRight, '\u{0}'),
            ("\u{2193}", PhysicalKey::ArrowDown, '\u{0}'),
            ("ESPACO", PhysicalKey::Space, ' '),
        ]
    }

    /// Monta os botoes por cima do jogo, uma vez so.
    fn criar_controles(&self) {
        let mtm = MainThreadMarker::from(self);
        let view = self.view();
        let mut guardados = self.ivars().controles.borrow_mut();
        if !guardados.is_empty() {
            return;
        }

        for (indice, (rotulo, _, _)) in Self::tabela_de_teclas().iter().enumerate() {
            let botao = unsafe { UIButton::buttonWithType(UIButtonType::System, mtm) };
            unsafe {
                botao.setTitle_forState(
                    Some(&NSString::from_str(rotulo)),
                    UIControlState::Normal,
                );
                botao.setTitleColor_forState(
                    Some(&UIColor::whiteColor()),
                    UIControlState::Normal,
                );
                botao.setBackgroundColor(Some(&UIColor::colorWithRed_green_blue_alpha(
                    0.04, 0.07, 0.13, 0.55,
                )));
                botao.setTag(indice as isize);

                // Apertar manda a tecla; soltar solta. Tres formas de soltar
                // porque o dedo pode sair de cima do botao antes de levantar.
                botao.addTarget_action_forControlEvents(
                    Some(self),
                    sel!(controleApertado:),
                    UIControlEvents::TouchDown,
                );
                botao.addTarget_action_forControlEvents(
                    Some(self),
                    sel!(controleSolto:),
                    UIControlEvents::TouchUpInside
                        | UIControlEvents::TouchUpOutside
                        | UIControlEvents::TouchCancel,
                );
            }
            botao.layer().setCornerRadius(10.0);
            view.addSubview(&botao);
            guardados.push(botao);
        }
        // eprintln, e nao tracing: em Release as mensagens informativas do
        // tracing nao saem, e a gente fica sem saber se isto rodou.
        eprintln!("CONTROLES criados: {}", guardados.len());
        drop(guardados);
        // Posiciona ja: se a gente esperar so pelo momento em que a tela se
        // arruma, e ele nao vier, os botoes ficam com tamanho zero — existem,
        // mas ninguem ve.
        self.posicionar_controles();
    }

    /// Recoloca os botoes conforme o tamanho atual da tela.
    ///
    /// As proporcoes sao as mesmas do APK: as teclas de acao a esquerda em
    /// cima, as setas em cruz embaixo a esquerda, e o espaco a direita, meio
    /// bloco pra dentro — a coluna da ponta fica livre pros botoes do jogo.
    fn posicionar_controles(&self) {
        let guardados = self.ivars().controles.borrow();
        if guardados.is_empty() {
            return;
        }
        let limites = self.view().bounds();
        let largura = limites.size.width;
        let altura = limites.size.height;
        eprintln!("CONTROLES posicionando em {largura} x {altura}");
        if largura < 2.0 || altura < 2.0 {
            // A tela ainda nao tem tamanho de verdade; volta quando tiver.
            return;
        }

        // O tamanho vem do LADO MENOR da tela, nao da altura.
        //
        // Medindo pela altura, em pe os botoes saiam gigantes: o 4 ficava fora
        // da tela e o espaco caia em cima das setas. Pelo lado menor, eles
        // ficam certos deitado (que e o normal) e continuam utilizaveis em pe.
        let menor = if largura < altura { largura } else { altura };
        let lado = menor * 0.13;
        let folga = lado * 0.15;
        let passo = lado + folga;

        let por = |indice: usize, x: f64, y: f64, w: f64, h: f64| {
            if let Some(botao) = guardados.get(indice) {
                botao.setFrame(CGRect::new(CGPoint::new(x, y), CGSize::new(w, h)));
            }
        };

        // 1 2 3 4
        for i in 0..4 {
            por(i, folga + (i as f64) * passo, folga, lado, lado);
        }
        // Z X C
        for i in 0..3 {
            por(4 + i, folga + (i as f64) * passo, folga + passo, lado, lado);
        }
        // setas em cruz
        let celula = lado * 0.85;
        let base = altura - celula * 3.0 - folga;
        por(7, folga, base + celula, celula, celula); // esquerda
        por(8, folga + celula, base, celula, celula); // cima
        por(9, folga + celula * 2.0, base + celula, celula, celula); // direita
        por(10, folga + celula, base + celula * 2.0, celula, celula); // baixo
        // espaco
        let largo = lado * 2.4;
        let alto = lado * 1.3;
        por(
            11,
            largura - folga - largo - passo,
            altura - folga - alto,
            largo,
            alto,
        );
    }

    /// Manda a tecla pro jogo, como se fosse um teclado de verdade.
    fn mandar_tecla(&self, indice: isize, apertando: bool) {
        let Some((_, fisica, caractere)) =
            Self::tabela_de_teclas().get(indice as usize).copied()
        else {
            return;
        };
        let logica = if caractere == '\u{0}' {
            LogicalKey::Unknown
        } else {
            LogicalKey::Character(caractere)
        };
        let key = KeyDescriptor {
            physical_key: fisica,
            logical_key: logica,
            key_location: KeyLocation::Standard,
        };
        let evento = if apertando {
            PlayerEvent::KeyDown { key }
        } else {
            PlayerEvent::KeyUp { key }
        };
        if self.ivars().player.get().is_some() {
            let mut player_lock = self.player_lock();
            player_lock.handle_event(evento);
        }
    }

    pub fn view(&self) -> Retained<PlayerView> {
        let view = (**self).view().expect("controller loads view");
        view.downcast().expect("must have correct view type")
    }

    #[track_caller]
    pub fn player_lock(&self) -> MutexGuard<'_, Player> {
        self.ivars()
            .player
            .get()
            .expect("player initialized")
            .lock()
            .expect("player lock")
    }
}
