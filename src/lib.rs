use objc2::runtime::AnyClass;
use objc2::ClassType;
use objc2_foundation::{MainThreadMarker, NSString};
use objc2_ui_kit::UIApplication;
use tracing_subscriber::fmt::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

// O ALOCADOR: MIMALLOC, E NAO O DO SISTEMA.
//
// Medido no simulador (27/10), abrindo e fechando a Sociedade do DDTank: de
// 5279 MB de footprint, 4243 MB estavam na zona de blocos pequenos do malloc
// do sistema (MALLOC_NANO), e ela cresceu 985 MB numa unica volta. O que
// estava em uso de verdade (residente) era uns 800 MB — e o coletor do
// Ruffle percorre tudo o que esta vivo o tempo todo, entao memoria viva nao
// fica parada. Era memoria JA LIBERADA que o alocador do sistema guarda e nao
// devolve; o iOS cobra ela do aplicativo. No navegador o alocador e outro, e
// o vazamento grande nao aparece.
//
// O mimalloc devolve ao sistema as paginas que ficam livres.
#[global_allocator]
static ALOCADOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod add_controller;
mod app_delegate;
mod edit_controller;
mod library_controller;
mod navegacao;
mod player_controller;
mod player_view;
mod registro;
mod roteiro;
mod scene_delegate;
mod teclado;
mod storage;

pub use self::app_delegate::AppDelegate;
pub use self::player_controller::PlayerController;
pub use self::player_view::PlayerView;

/// Emit logging to either OSLog or stderr, depending on if using Mac
/// Catalyst or native.
///
/// TODO: If running Mac Catalyst under Xcode
pub fn init_logging() {
    let subscriber = tracing_subscriber::registry();

    let env_filter = tracing_subscriber::EnvFilter::builder()
        .parse_lossy(std::env::var("RUST_LOG").as_deref().unwrap_or("info"));

    let subscriber = subscriber.with(env_filter);

    #[cfg(target_abi = "macabi")]
    let subscriber = subscriber.with(Layer::new().with_writer(std::io::stderr));

    #[cfg(not(target_abi = "macabi"))]
    let subscriber = subscriber.with(tracing_oslog::OsLogger::default());

    subscriber.init();
}

pub fn launch(app_class: Option<&AnyClass>, delegate_class: Option<&AnyClass>) {
    // ANTES DE QUALQUER COISA: passar a guardar o motivo de uma quebra.
    //
    // Se o Ruffle quebrar la na frente, a mensagem dele e a unica pista que
    // existe — e sem isto ela morre junto com o aplicativo. Instalado aqui,
    // no comeco, porque quebra que acontece durante a partida ja estaria
    // fora do alcance se instalassemos depois.
    registro::iniciar_sessao();

    // Set inside Info.plist
    let _ = scene_delegate::SceneDelegate::class();

    // These classes are loaded from a storyboard,
    // and hence need to be initialized first.
    // See also [storyboard_connections.h]
    let _ = player_view::PlayerView::class();
    let _ = player_controller::PlayerController::class();
    let _ = library_controller::LibraryController::class();
    let _ = navegacao::NavegacaoPrincipal::class();
    let _ = edit_controller::EditController::class();
    let _ = add_controller::AddController::class();

    // This is loaded by CoreData
    let _ = storage::Movie::class();
    let _ = storage::MovieData::class();

    let mtm = MainThreadMarker::new().unwrap();
    UIApplication::main(
        app_class.map(|cls| NSString::from_class(cls)).as_deref(),
        delegate_class
            .map(|cls| NSString::from_class(cls))
            .as_deref(),
        mtm,
    );
}
