use std::cell::Cell;

use objc2::rc::{Allocated, Retained};
use objc2::{define_class, msg_send, DefinedClass as _, MainThreadOnly, Message};
use objc2_foundation::{NSObjectProtocol, NSProcessInfo, NSSet, NSURL};
use objc2_ui_kit::{
    UINavigationController, UIOpenURLContext, UIResponder, UIScene, UISceneConnectionOptions,
    UISceneDelegate, UISceneSession, UIWindow, UIWindowScene, UIWindowSceneDelegate,
};
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_frontend_utils::player_options::PlayerOptions;
use url::Url;

use crate::{storage, PlayerController};

pub struct Ivars {
    window: Cell<Option<Retained<UIWindow>>>,
    /// Pra so abrir o jogo de teste uma vez, e nao a cada vez que o
    /// aplicativo volta pra frente.
    ja_abriu_de_teste: Cell<bool>,
}

define_class!(
    #[unsafe(super(UIResponder))]
    #[name = "SceneDelegate"]
    #[ivars = Ivars]
    pub struct SceneDelegate;

    /// Called by UIStoryboard
    impl SceneDelegate {
        #[unsafe(method_id(init))]
        fn init(this: Allocated<Self>) -> Retained<Self> {
            tracing::info!("init scene");
            let this = this.set_ivars(Ivars {
                window: Cell::new(None),
                ja_abriu_de_teste: Cell::new(false),
            });
            unsafe { msg_send![super(this), init] }
        }
    }

    unsafe impl NSObjectProtocol for SceneDelegate {}

    #[allow(non_snake_case)]
    unsafe impl UISceneDelegate for SceneDelegate {
        #[unsafe(method(scene:willConnectToSession:options:))]
        fn scene_willConnectToSession_options(
            &self,
            _scene: &UIScene,
            _session: &UISceneSession,
            _connection_options: &UISceneConnectionOptions,
        ) {
            tracing::info!("scene:willConnectToSession:options:");
        }

        #[unsafe(method(sceneDidDisconnect:))]
        fn sceneDidDisconnect(&self, _scene: &UIScene) {
            tracing::info!("sceneDidDisconnect:");
        }

        #[unsafe(method(sceneDidBecomeActive:))]
        fn sceneDidBecomeActive(&self, scene: &UIScene) {
            tracing::info!("sceneDidBecomeActive:");

            // ENTRADA DE TESTE: --jogo <endereco>
            //
            // Existe porque o iPhone pergunta "abrir no Ruffle?" quando o
            // endereco vem de fora, e maquina nenhuma responde essa pergunta.
            // Passando o endereco na abertura do aplicativo, nao ha pergunta.
            //
            // Pro jogador isso nunca aparece: ele toca no link do site,
            // responde a pergunta uma vez, e pronto.
            if !self.ivars().ja_abriu_de_teste.get() {
                self.ivars().ja_abriu_de_teste.set(true);
                if let Some(endereco) = endereco_de_teste() {
                    tracing::info!("abrindo por argumento: {endereco}");
                    tocar_endereco(scene, &endereco);
                }
            }

            // Restart playing.
            let nav = get_navigation_controller(scene);
            for controller in nav.viewControllers() {
                if let Some(controller) = controller.downcast_ref::<PlayerController>() {
                    controller.view().start();
                }
            }
        }

        #[unsafe(method(sceneWillResignActive:))]
        fn sceneWillResignActive(&self, scene: &UIScene) {
            tracing::info!("sceneWillResignActive:");

            // Stop playing.
            let nav = get_navigation_controller(scene);
            for controller in nav.viewControllers() {
                if let Some(controller) = controller.downcast_ref::<PlayerController>() {
                    controller.view().stop();
                }
            }
        }

        #[unsafe(method(sceneWillEnterForeground:))]
        fn sceneWillEnterForeground(&self, _scene: &UIScene) {
            tracing::info!("sceneWillEnterForegrounds:");
        }

        #[unsafe(method(sceneDidEnterBackground:))]
        fn sceneDidEnterBackground(&self, scene: &UIScene) {
            tracing::info!("sceneDidEnterBackground:");

            // Flush when going to the background.
            let nav = get_navigation_controller(scene);
            for controller in nav.viewControllers() {
                if let Some(controller) = controller.downcast_ref::<PlayerController>() {
                    controller.view().flush();
                }
            }
        }

        #[unsafe(method(scene:openURLContexts:))]
        fn scene_openURLContexts(&self, scene: &UIScene, url_contexts: &NSSet<UIOpenURLContext>) {
            tracing::info!(?url_contexts, "scene:openURLContexts:");

            // ENDERECO DA INTERNET: toca direto, sem passar pela biblioteca.
            //
            // O Ruffle nao le usuario e chave do endereco — ele espera receber
            // esses valores separados. O DDTank precisa deles pra logar, e sem
            // isso o jogo abre numa tela vazia.
            //
            // Entao, quando o endereco vem de fora e e da internet, a gente
            // separa o que vem depois da interrogacao e entrega como parametros
            // do jogo. E o mesmo que o navegador faz sozinho.
            for context in url_contexts {
                let url = context.URL();
                if tocar_da_internet(scene, &url).is_some() {
                    return;
                }
            }

            for context in url_contexts {
                let url = context.URL();

                // TODO: Do something else when this is set?
                let _ = context.options().openInPlace();

                if storage::movie_from_url(&url).is_none() {
                    storage::add_movie(&url);
                } else {
                    // This is intentional, when the user opens URLs from outside
                    // the app, we only want to add them to the library if not
                    // already there.
                    tracing::debug!("did not add existing movie {url:?}");
                }
            }

            if url_contexts.count() == 1 {
                let context = url_contexts.anyObject().unwrap();
                let url = context.URL();
                // Start playing this one immediately
                play_url(scene, &url);
            }
        }
    }

    #[allow(non_snake_case)]
    unsafe impl UIWindowSceneDelegate for SceneDelegate {
        #[unsafe(method_id(window))]
        fn window(&self) -> Option<Retained<UIWindow>> {
            let window = self.ivars().window.take();
            self.ivars().window.set(window.clone());
            window
        }

        #[unsafe(method(setWindow:))]
        fn setWindow(&self, window: Option<&UIWindow>) {
            self.ivars().window.set(window.map(|w| w.retain()));
        }
    }
);

impl Drop for SceneDelegate {
    fn drop(&mut self) {
        tracing::info!("drop scene");
    }
}

fn get_navigation_controller(scene: &UIScene) -> Retained<UINavigationController> {
    let scene = scene.downcast_ref::<UIWindowScene>().unwrap();
    // FIXME: Assumes single-window
    let window = scene.windows().firstObject().unwrap();
    let root = window.rootViewController().unwrap();
    root.downcast::<UINavigationController>().unwrap()
}

/// Toca um jogo que mora na internet, com os parametros do endereco.
///
/// Aceita duas formas:
///
///   http://servidor/Loading.swf?user=fulano&key=abc
///   deathnote://jogar?u=<o endereco acima, codificado>
///
/// A segunda existe porque o iPhone nao deixa um link comum da internet abrir
/// um aplicativo — ele abriria o navegador. Com um esquema proprio, um botao
/// numa pagina abre o jogo aqui dentro.
///
/// Devolve None quando o endereco nao e da internet, e ai o caminho antigo
/// (biblioteca de arquivos) segue normalmente.
fn tocar_da_internet(scene: &UIScene, nsurl: &NSURL) -> Option<()> {
    let texto = nsurl.absoluteString()?.to_string();
    tocar_endereco(scene, &texto)
}

/// Le o endereco passado na abertura do aplicativo, quando houver.
fn endereco_de_teste() -> Option<String> {
    let argumentos = NSProcessInfo::processInfo().arguments();
    let mut anterior = String::new();
    for argumento in argumentos.iter() {
        let atual = argumento.to_string();
        if anterior == "--jogo" {
            return Some(atual);
        }
        anterior = atual;
    }
    None
}

fn tocar_endereco(scene: &UIScene, texto: &str) -> Option<()> {
    let endereco = Url::parse(texto).ok()?;

    let alvo = match endereco.scheme() {
        "deathnote" => {
            // O endereco de verdade vem dentro do parametro "u".
            let dentro = endereco
                .query_pairs()
                .find(|(chave, _)| chave == "u")?
                .1
                .into_owned();
            Url::parse(&dentro).ok()?
        }
        "http" | "https" => endereco,
        _ => return None,
    };

    // Tudo que vem depois da interrogacao vira parametro do jogo — e assim
    // que o DDTank recebe usuario, chave, servidor e o resto.
    let parametros: Vec<(String, String)> = alvo
        .query_pairs()
        .map(|(chave, valor)| (chave.into_owned(), valor.into_owned()))
        .collect();

    tracing::info!("tocando da internet: {alvo} ({} parametros)", parametros.len());

    let nav = get_navigation_controller(scene);
    nav.popToRootViewControllerAnimated(false);

    // O campo la dentro se chama "parameters"; a variavel daqui esta em
    // portugues. Escrever so "parameters," procuraria uma variavel com esse
    // nome — foi o primeiro erro de compilacao.
    let opcoes = PlayerOptions {
        parameters: parametros,
        ..Default::default()
    };
    // DirectFile nao aceita um endereco cru: ele quer um ContentDescriptor,
    // que e o endereco mais o que o Ruffle precisa saber sobre ele. Pra coisa
    // que mora na internet, a propria biblioteca oferece o new_remote.
    let controller = PlayerController::new(
        scene.mtm(),
        PlayingContent::DirectFile(ContentDescriptor::new_remote(alvo)),
        opcoes,
    );
    nav.pushViewController_animated(&controller, true);

    Some(())
}

fn play_url(scene: &UIScene, url: &NSURL) -> Option<()> {
    let _span = tracing::info_span!("play_url").entered();

    let nav = get_navigation_controller(scene);

    // TODO: Investigate if we really want to do this?
    nav.popToRootViewControllerAnimated(true);

    let movie = storage::movie_from_url(url).expect("we just added the movie");
    let player_controller = PlayerController::empty(scene.mtm());
    player_controller.setup_movie(&movie);
    nav.pushViewController_animated(&player_controller, true);

    Some(())
}
