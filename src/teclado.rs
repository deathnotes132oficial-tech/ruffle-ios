//! O TECLADO DO IPHONE.
//!
//! POR QUE ISTO PRECISOU EXISTIR
//! -----------------------------
//! Tocar no chat do jogo nao abria teclado nenhum. Nao era defeito: era
//! ausencia. O Ruffle AVISA quando o jogo poe o cursor num campo de texto —
//! ele chama open_virtual_keyboard — mas o aviso chegava numa implementacao
//! vazia, a que vem de fabrica:
//!
//!     core/src/backend/ui.rs   fn open_virtual_keyboard(&self) {}
//!
//! Ninguem tinha escrito a nossa. No Ruffle do Android esse mesmo aviso chega
//! em alguem que abre o teclado, e e por isso que la funciona.
//!
//! POR QUE UMA BANDEIRA, E NAO A TELA DIRETO
//! -----------------------------------------
//! O caminho obvio seria esta peca guardar a tela do jogo e manda-la abrir o
//! teclado. Nao da: o Ruffle exige que ela possa atravessar threads, e objeto
//! de interface do iPhone nao pode sair da linha principal.
//!
//! Entao ela so levanta uma bandeira — um valor que qualquer thread pode ler.
//! O laco de quadro, que ja roda o tempo todo na linha principal, ve a
//! bandeira e abre ou fecha o teclado. Sem espera, sem trava, e o atraso e de
//! no maximo um quadro.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use ruffle_core::backend::ui::{
    DialogResultFuture, FileFilter, FontDefinition, FullscreenError, LanguageIdentifier,
    MouseCursor, MultiDialogResultFuture, NullUiBackend, UiBackend,
};
use ruffle_core::font::FontQuery;
use url::Url;

/// A bandeira: levantada, o teclado deve estar na tela.
///
/// Copiar isto copia a bandeira, nao o estado — as copias continuam apontando
/// pro mesmo valor. E assim que o Ruffle e a tela conversam sem se conhecerem.
#[derive(Clone, Default)]
pub struct PedidoDeTeclado(Arc<AtomicBool>);

impl PedidoDeTeclado {
    /// O jogo esta pedindo teclado agora?
    pub fn aberto(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    fn definir(&self, aberto: bool) {
        self.0.store(aberto, Ordering::Relaxed);
    }
}

/// A nossa interface, que e a de fabrica mais o teclado.
///
/// Tudo o que nao e teclado e repassado pra implementacao vazia original. Isso
/// e de proposito: area de transferencia, escolha de arquivo e tela cheia nao
/// existem dentro do jogo no iPhone, e inventar comportamento pra elas seria
/// criar defeito onde nao ha.
pub struct InterfaceDoIphone {
    base: NullUiBackend,
    teclado: PedidoDeTeclado,
}

impl InterfaceDoIphone {
    pub fn new(teclado: PedidoDeTeclado) -> Self {
        Self {
            base: NullUiBackend::new(),
            teclado,
        }
    }
}

impl UiBackend for InterfaceDoIphone {
    // ------------------------------------------------ o que nos importa

    fn open_virtual_keyboard(&self) {
        tracing::info!("o jogo pediu o teclado");
        self.teclado.definir(true);
    }

    fn close_virtual_keyboard(&self) {
        tracing::info!("o jogo dispensou o teclado");
        self.teclado.definir(false);
    }

    // ------------------------------------------------ o resto, repassado

    fn mouse_visible(&self) -> bool {
        self.base.mouse_visible()
    }

    fn set_mouse_visible(&mut self, visible: bool) {
        self.base.set_mouse_visible(visible);
    }

    fn set_mouse_cursor(&mut self, cursor: MouseCursor) {
        self.base.set_mouse_cursor(cursor);
    }

    fn clipboard_content(&mut self) -> String {
        self.base.clipboard_content()
    }

    fn set_clipboard_content(&mut self, content: String) {
        self.base.set_clipboard_content(content);
    }

    fn set_fullscreen(&mut self, is_full: bool) -> Result<(), FullscreenError> {
        self.base.set_fullscreen(is_full)
    }

    fn display_root_movie_download_failed_message(&self, invalid_swf: bool, fetch_error: String) {
        self.base
            .display_root_movie_download_failed_message(invalid_swf, fetch_error);
    }

    fn message(&self, message: &str) {
        self.base.message(message);
    }

    fn display_unsupported_video(&self, url: Url) {
        self.base.display_unsupported_video(url);
    }

    fn load_device_font(&self, query: &FontQuery, register: &mut dyn FnMut(FontDefinition)) {
        self.base.load_device_font(query, register);
    }

    fn sort_device_fonts(
        &self,
        query: &FontQuery,
        register: &mut dyn FnMut(FontDefinition),
    ) -> Vec<FontQuery> {
        self.base.sort_device_fonts(query, register)
    }

    fn language(&self) -> LanguageIdentifier {
        self.base.language()
    }

    fn display_file_open_dialog(&mut self, filters: Vec<FileFilter>) -> Option<DialogResultFuture> {
        self.base.display_file_open_dialog(filters)
    }

    fn display_file_open_dialog_multiple(
        &mut self,
        filters: Vec<FileFilter>,
    ) -> Option<MultiDialogResultFuture> {
        self.base.display_file_open_dialog_multiple(filters)
    }

    fn display_file_save_dialog(
        &mut self,
        file_name: String,
        title: String,
    ) -> Option<DialogResultFuture> {
        self.base.display_file_save_dialog(file_name, title)
    }

    fn close_file_dialog(&mut self) {
        self.base.close_file_dialog();
    }
}
