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
    MainThreadMarker, NSBundle, NSCoder, NSObjectProtocol, NSRunLoop, NSString, NSUserDefaults,
};
use objc2_ui_kit::{
    UIButton, UIButtonType, UIColor, UIControlEvents, UIControlState,
    UIGestureRecognizerState, UIInterfaceOrientationMask, UILabel,
    UILongPressGestureRecognizer, UINavigationController, UIPanGestureRecognizer,
    UITapGestureRecognizer, UIView, UIViewController,
};
use ruffle_core::backend::navigator::OwnedFuture;
use ruffle_core::backend::storage::StorageBackend;
use ruffle_core::config::Letterbox;
use ruffle_core::events::{KeyDescriptor, KeyLocation, LogicalKey, MouseButton, PhysicalKey};
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

/// Quantos botoes personalizaveis existem, igual ao APK.
const LIVRES: usize = 7;
/// Onde comecam os personalizaveis na lista de botoes.
const LIVRE_0: isize = 12;
/// O olho, que esconde tudo menos ele.
const OLHO: isize = 19;
/// O botao que troca as duas fileiras de cima pelas teclas personalizaveis.
const PALETA: isize = 20;

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

    /// Com a paleta aberta, as duas fileiras de cima viram as teclas
    /// personalizaveis. E o botao de trocar, igual ao do APK.
    paleta_aberta: Cell<bool>,

    /// O olho: esconde tudo menos ele proprio.
    escondidos: Cell<bool>,

    /// Qual tecla cada botao personalizavel manda hoje.
    /// None = vazio (mostra "+"), Some(i) = posicao na lista de opcoes.
    livres: RefCell<Vec<Option<usize>>>,

    /// A area onde o dedo desliza pra mover o ponteiro, como um touchpad.
    area_mouse: RefCell<Option<Retained<UIView>>>,
    /// A setinha desenhada por cima do jogo.
    seta: RefCell<Option<Retained<UILabel>>>,
    /// Onde o ponteiro esta agora, em coordenadas da tela.
    onde_mouse: Cell<CGPoint>,
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
            let indice = botao.tag();
            match indice {
                OLHO => self.alternar_olho(),
                PALETA => self.alternar_paleta(),
                _ => self.mandar_tecla(indice, true),
            }
        }

        /// Deslizar o dedo na area move o ponteiro, como num touchpad.
        ///
        /// E movimento RELATIVO, e nao absoluto: o dedo empurra a setinha a
        /// partir de onde ela esta. Assim da pra mirar com precisao numa area
        /// pequena, que e todo o motivo de o ponteiro existir.
        #[unsafe(method(moverMouse:))]
        fn moverMouse(&self, gesto: &UIPanGestureRecognizer) {
            let view = self.view();
            let passo = unsafe { gesto.translationInView(Some(&view)) };
            unsafe { gesto.setTranslation_inView(CGPoint::ZERO, Some(&view)) };

            let atual = self.ivars().onde_mouse.get();
            let limites = view.bounds();
            let x = (atual.x + passo.x).clamp(0.0, limites.size.width);
            let y = (atual.y + passo.y).clamp(0.0, limites.size.height);
            self.ivars().onde_mouse.set(CGPoint::new(x, y));
            self.desenhar_seta();
            self.mandar_mouse(0);
        }

        /// Tocar na area clica onde a setinha esta.
        #[unsafe(method(clicarMouse:))]
        fn clicarMouse(&self, _gesto: &UITapGestureRecognizer) {
            self.mandar_mouse(1);
            self.mandar_mouse(2);
        }

        /// Segurar na area aperta e so solta quando o dedo sai.
        #[unsafe(method(segurarMouse:))]
        fn segurarMouse(&self, gesto: &UILongPressGestureRecognizer) {
            match gesto.state() {
                UIGestureRecognizerState::Began => self.mandar_mouse(1),
                UIGestureRecognizerState::Ended | UIGestureRecognizerState::Cancelled => {
                    self.mandar_mouse(2)
                }
                _ => {}
            }
        }

        /// Segurar um botao personalizavel troca a tecla dele.
        #[unsafe(method(trocarTecla:))]
        fn trocarTecla(&self, gesto: &UILongPressGestureRecognizer) {
            if gesto.state() != UIGestureRecognizerState::Began {
                return;
            }
            let Some(view) = gesto.view() else { return };
            let Some(botao) = view.downcast_ref::<UIButton>() else {
                return;
            };
            self.proxima_tecla_livre(botao.tag());
        }

        #[unsafe(method(controleSolto:))]
        fn controleSolto(&self, botao: &UIButton) {
            let indice = botao.tag();
            if indice != OLHO && indice != PALETA {
                self.mandar_tecla(indice, false);
            }
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
            paleta_aberta: Cell::new(false),
            escondidos: Cell::new(false),
            livres: RefCell::new(Vec::new()),
            area_mouse: RefCell::new(None),
            seta: RefCell::new(None),
            onde_mouse: Cell::new(CGPoint::new(200.0, 200.0)),
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

    /// As opcoes que um botao personalizavel pode assumir.
    ///
    /// Mesma lista do APK. Segurar o botao anda pra proxima.
    fn opcoes_livres() -> &'static [(&'static str, PhysicalKey, char)] {
        &[
            ("5", PhysicalKey::Digit5, '5'),
            ("6", PhysicalKey::Digit6, '6'),
            ("7", PhysicalKey::Digit7, '7'),
            ("8", PhysicalKey::Digit8, '8'),
            ("9", PhysicalKey::Digit9, '9'),
            ("0", PhysicalKey::Digit0, '0'),
            ("Q", PhysicalKey::KeyQ, 'q'),
            ("W", PhysicalKey::KeyW, 'w'),
            ("E", PhysicalKey::KeyE, 'e'),
            ("R", PhysicalKey::KeyR, 'r'),
            ("T", PhysicalKey::KeyT, 't'),
            ("Y", PhysicalKey::KeyY, 'y'),
            ("A", PhysicalKey::KeyA, 'a'),
            ("S", PhysicalKey::KeyS, 's'),
            ("D", PhysicalKey::KeyD, 'd'),
            ("F", PhysicalKey::KeyF, 'f'),
            ("G", PhysicalKey::KeyG, 'g'),
            ("H", PhysicalKey::KeyH, 'h'),
            ("V", PhysicalKey::KeyV, 'v'),
            ("B", PhysicalKey::KeyB, 'b'),
            ("P", PhysicalKey::KeyP, 'p'),
            ("TAB", PhysicalKey::Tab, '\u{0009}'),
            ("ESC", PhysicalKey::Escape, '\u{0}'),
            ("ENTER", PhysicalKey::Enter, '\u{000D}'),
        ]
    }

    /// As teclas dos botoes fixos, na ordem em que eles sao criados.
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

        // Recupera o que a pessoa escolheu da ultima vez.
        {
            let mut livres = self.ivars().livres.borrow_mut();
            livres.clear();
            let padroes = unsafe { NSUserDefaults::standardUserDefaults() };
            for i in 0..LIVRES {
                let chave = NSString::from_str(&format!("tecla_livre_{i}"));
                let guardado = unsafe { padroes.integerForKey(&chave) };
                livres.push(if guardado > 0 {
                    Some((guardado - 1) as usize)
                } else {
                    None
                });
            }
        }

        // Os rotulos de todos os botoes, na ordem dos indices.
        let mut rotulos: Vec<String> = Self::tabela_de_teclas()
            .iter()
            .map(|(r, _, _)| r.to_string())
            .collect();
        for i in 0..LIVRES {
            rotulos.push(self.rotulo_livre(i));
        }
        rotulos.push(String::new()); // olho, desenhado com simbolo
        rotulos.push("\u{21C4}".to_string()); // trocar teclas
        let rotulos: Vec<(usize, String)> = rotulos.into_iter().enumerate().collect();

        for (indice, rotulo) in rotulos.iter() {
            let indice = *indice;
            let rotulo = if indice == OLHO as usize {
                "\u{25C9}".to_string()
            } else {
                rotulo.clone()
            };
            let rotulo = &rotulo;
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

            // Segurar um personalizavel troca a tecla dele.
            if (LIVRE_0..LIVRE_0 + LIVRES as isize).contains(&(indice as isize)) {
                let gesto = unsafe {
                    UILongPressGestureRecognizer::initWithTarget_action(
                        mtm.alloc(),
                        Some(self),
                        Some(sel!(trocarTecla:)),
                    )
                };
                botao.addGestureRecognizer(&gesto);
            }

            view.addSubview(&botao);
            guardados.push(botao);
        }
        // eprintln, e nao tracing: em Release as mensagens informativas do
        // tracing nao saem, e a gente fica sem saber se isto rodou.
        eprintln!("CONTROLES criados: {}", guardados.len());
        drop(guardados);
        self.criar_mouse();
        self.aplicar_paleta();
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

        // 1 2 3 4 — e, no mesmo lugar, os quatro primeiros personalizaveis.
        for i in 0..4 {
            let x = folga + (i as f64) * passo;
            por(i, x, folga, lado, lado);
            por(LIVRE_0 as usize + i, x, folga, lado, lado);
        }
        // Z X C — e os tres personalizaveis seguintes, no mesmo lugar.
        for i in 0..3 {
            let x = folga + (i as f64) * passo;
            por(4 + i, x, folga + passo, lado, lado);
            por(LIVRE_0 as usize + 4 + i, x, folga + passo, lado, lado);
        }
        // O olho fica depois do C, e nunca troca de lugar.
        por(OLHO as usize, folga + 3.0 * passo, folga + passo, lado, lado);
        // O trocar fica embaixo do Z.
        por(PALETA as usize, folga, folga + 2.0 * passo, lado, lado);
        // setas em cruz
        let celula = lado * 0.85;
        let base = altura - celula * 3.0 - folga;
        por(7, folga, base + celula, celula, celula); // esquerda
        por(8, folga + celula, base, celula, celula); // cima
        por(9, folga + celula * 2.0, base + celula, celula, celula); // direita
        por(10, folga + celula, base + celula * 2.0, celula, celula); // baixo
        // espaco, embaixo a direita — meio bloco pra dentro, pra deixar a
        // coluna da ponta livre pros botoes do proprio jogo.
        let largo = lado * 2.4;
        let alto = lado * 1.3;
        let x_espaco = largura - folga - largo - passo;
        let y_espaco = altura - folga - alto;
        por(11, x_espaco, y_espaco, largo, alto);

        // a area do mouse fica EM CIMA do espaco, igual ao APK
        let alto_area = lado * 2.6;
        if let Some(area) = self.ivars().area_mouse.borrow().as_ref() {
            area.setFrame(CGRect::new(
                CGPoint::new(x_espaco, y_espaco - folga - alto_area),
                CGSize::new(largo, alto_area),
            ));
        }
        self.desenhar_seta();
    }

    /// Monta a area do ponteiro e a setinha.
    fn criar_mouse(&self) {
        let mtm = MainThreadMarker::from(self);
        let view = self.view();

        let area = unsafe { UIView::initWithFrame(mtm.alloc(), CGRect::ZERO) };
        unsafe {
            area.setBackgroundColor(Some(&UIColor::colorWithRed_green_blue_alpha(
                0.04, 0.07, 0.13, 0.35,
            )));
        }
        area.layer().setCornerRadius(10.0);

        let pan = unsafe {
            UIPanGestureRecognizer::initWithTarget_action(
                mtm.alloc(),
                Some(self),
                Some(sel!(moverMouse:)),
            )
        };
        let toque = unsafe {
            UITapGestureRecognizer::initWithTarget_action(
                mtm.alloc(),
                Some(self),
                Some(sel!(clicarMouse:)),
            )
        };
        let segurar = unsafe {
            UILongPressGestureRecognizer::initWithTarget_action(
                mtm.alloc(),
                Some(self),
                Some(sel!(segurarMouse:)),
            )
        };
        area.addGestureRecognizer(&pan);
        area.addGestureRecognizer(&toque);
        area.addGestureRecognizer(&segurar);
        view.addSubview(&area);

        // A setinha e so um texto: desenhar uma seta de verdade exigiria
        // codigo de desenho, e o simbolo faz o mesmo servico.
        let seta = unsafe { UILabel::initWithFrame(mtm.alloc(), CGRect::ZERO) };
        unsafe {
            seta.setText(Some(&NSString::from_str("\u{27A4}")));
            seta.setTextColor(Some(&UIColor::whiteColor()));
        }
        seta.setUserInteractionEnabled(false);
        view.addSubview(&seta);

        *self.ivars().area_mouse.borrow_mut() = Some(area);
        *self.ivars().seta.borrow_mut() = Some(seta);
        eprintln!("MOUSE criado");
    }

    /// Poe a setinha onde o ponteiro esta.
    fn desenhar_seta(&self) {
        if let Some(seta) = self.ivars().seta.borrow().as_ref() {
            let onde = self.ivars().onde_mouse.get();
            seta.setFrame(CGRect::new(onde, CGSize::new(30.0, 34.0)));
        }
    }

    /// Manda o evento de mouse pro jogo, na posicao da setinha.
    ///
    /// 0 = mover, 1 = apertar, 2 = soltar.
    fn mandar_mouse(&self, tipo: u8) {
        if self.ivars().player.get().is_none() {
            return;
        }
        let onde = self.ivars().onde_mouse.get();
        // A mesma conta que o aplicativo faz com o toque comum: de pontos da
        // tela pra pontos do desenho.
        let escala = self.view().contentScaleFactor() as f64;
        let x = onde.x * escala;
        let y = onde.y * escala;

        let mut player_lock = self.player_lock();
        player_lock.set_mouse_in_stage(true);
        let evento = match tipo {
            1 => PlayerEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                index: Some(1),
            },
            2 => PlayerEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            },
            _ => PlayerEvent::MouseMove { x, y },
        };
        player_lock.handle_event(evento);
    }

    /// O rotulo de um botao personalizavel: a tecla escolhida, ou "+".
    fn rotulo_livre(&self, posicao: usize) -> String {
        match self.ivars().livres.borrow().get(posicao).copied().flatten() {
            Some(i) => Self::opcoes_livres()[i].0.to_string(),
            None => "+".to_string(),
        }
    }

    /// Segurar o botao anda pra proxima tecla da lista, e depois volta ao
    /// vazio. E mais simples que a grade do APK e faz o mesmo servico.
    fn proxima_tecla_livre(&self, indice: isize) {
        let posicao = (indice - LIVRE_0) as usize;
        if posicao >= LIVRES {
            return;
        }
        let proxima = {
            let mut livres = self.ivars().livres.borrow_mut();
            let atual = livres[posicao];
            let proxima = match atual {
                None => Some(0),
                Some(i) if i + 1 < Self::opcoes_livres().len() => Some(i + 1),
                Some(_) => None,
            };
            livres[posicao] = proxima;
            proxima
        };

        // Guarda a escolha pra proxima vez que abrir o jogo.
        let padroes = unsafe { NSUserDefaults::standardUserDefaults() };
        let chave = NSString::from_str(&format!("tecla_livre_{posicao}"));
        let valor = proxima.map(|i| i as isize + 1).unwrap_or(0);
        unsafe { padroes.setInteger_forKey(valor, &chave) };

        let rotulo = self.rotulo_livre(posicao);
        if let Some(botao) = self.ivars().controles.borrow().get(indice as usize) {
            unsafe {
                botao.setTitle_forState(
                    Some(&NSString::from_str(&rotulo)),
                    UIControlState::Normal,
                )
            };
        }
    }

    /// O olho: esconde todos os botoes menos ele proprio.
    fn alternar_olho(&self) {
        let escondendo = !self.ivars().escondidos.get();
        self.ivars().escondidos.set(escondendo);
        self.aplicar_paleta();
    }

    /// Troca as duas fileiras de cima entre as teclas do jogo e as suas.
    fn alternar_paleta(&self) {
        let aberta = !self.ivars().paleta_aberta.get();
        self.ivars().paleta_aberta.set(aberta);
        self.aplicar_paleta();
    }

    /// Decide quem aparece: o olho manda em todos, a paleta manda nas duas
    /// fileiras de cima.
    fn aplicar_paleta(&self) {
        let escondidos = self.ivars().escondidos.get();
        let paleta = self.ivars().paleta_aberta.get();
        let guardados = self.ivars().controles.borrow();
        for (indice, botao) in guardados.iter().enumerate() {
            let indice = indice as isize;
            let visivel = if indice == OLHO {
                true
            } else if escondidos {
                false
            } else if (LIVRE_0..LIVRE_0 + LIVRES as isize).contains(&indice) {
                paleta
            } else if (0..7).contains(&indice) {
                !paleta
            } else {
                true
            };
            botao.setHidden(!visivel);
        }
    }

    /// Manda a tecla pro jogo, como se fosse um teclado de verdade.
    fn mandar_tecla(&self, indice: isize, apertando: bool) {
        let ficha = if (LIVRE_0..LIVRE_0 + LIVRES as isize).contains(&indice) {
            // Personalizavel: vale a tecla que a pessoa escolheu, se houver.
            let escolha = self.ivars().livres.borrow()[(indice - LIVRE_0) as usize];
            escolha.and_then(|i| Self::opcoes_livres().get(i).copied())
        } else {
            Self::tabela_de_teclas().get(indice as usize).copied()
        };
        let Some((_, fisica, caractere)) = ficha else {
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
