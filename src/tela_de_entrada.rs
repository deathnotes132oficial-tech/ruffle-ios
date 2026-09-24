//! A TELA DE ENTRADA DO APLICATIVO.
//!
//! POR QUE ELA E UMA PAGINA DA INTERNET, E NAO UM FORMULARIO DE VERDADE
//! -------------------------------------------------------------------
//! Entrar no DDTank nao e "conferir uma senha": sao duas conversas com o
//! servidor do 337, com cookies no meio, e no fim a leitura do endereco do
//! jogo dentro do HTML que eles devolvem. Tudo isso ja existe, escrito e
//! testado, em deathnotestore.com.br/jogar/ — e e a mesma tela que o jogador
//! ve no navegador e no aplicativo de Android.
//!
//! Reescrever isso aqui em Rust seria escrever pela segunda vez algo que ja
//! funciona, e passar a ter dois lugares pra consertar quando o 337 mudar
//! alguma coisa. Entao o aplicativo mostra a propria pagina.
//!
//! COMO O JOGO SAI DA PAGINA E ENTRA NO RUFFLE
//! -------------------------------------------
//! Depois do login, a pagina oferece um botao cujo endereco comeca com
//! "deathnote://". Esse endereco nao existe na internet: e um combinado nosso.
//! Aqui embaixo, toda navegacao passa por decidePolicyForNavigationAction, e
//! quando o endereco e desse tipo nos dizemos "nao navegue" e abrimos o jogo
//! no Ruffle, com usuario e chave que vieram dentro dele.
//!
//! Fora do aplicativo o mesmo botao continua servindo: no Safari, ele faz o
//! iPhone chamar o aplicativo. Um botao so, dois caminhos.
//!
//! POR QUE O WKWebView E DECLARADO AQUI, NA MAO
//! --------------------------------------------
//! A biblioteca pronta (objc2-web-kit) declara o WKWebView com
//! `#[cfg(target_os = "macos")]`: no iPhone a classe simplesmente nao existe
//! do lado do Rust, e nenhuma opcao do Cargo faz ela aparecer — a versao
//! 0.3.2 e a ultima publicada, entao nao ha pra onde atualizar.
//!
//! A classe existe no aparelho: e a mesma do Safari, e o iOS traz ela desde
//! sempre. So falta a traducao pro Rust. Como usamos tres metodos dela,
//! escrever essa traducao e menor do que parece, e nos tira a dependencia
//! inteira de cima.

use block2::DynBlock;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, NSObject};
use objc2::{define_class, extern_class, msg_send, DefinedClass as _, MainThreadOnly};
use objc2_core_foundation::CGRect;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSString, NSURLRequest, NSURL};
use objc2_ui_kit::{UIColor, UIResponder, UIView, UIViewAutoresizing, UIViewController};

use crate::scene_delegate::tocar_endereco_no_nav;

/// A pagina que o aplicativo abre.
const ENDERECO_DA_ENTRADA: &str = "https://deathnotestore.com.br/jogar/";

/// As duas respostas possiveis pro WebKit, quando ele pergunta se pode
/// navegar. Sao os valores de WKNavigationActionPolicy, que e um NSInteger.
const NAO_NAVEGUE: isize = 0;
const PODE_NAVEGAR: isize = 1;

// CARREGAR A BIBLIOTECA DO WEBKIT.
//
// Sem esta linha o programa compila, abre, e MORRE na primeira vez que
// procura a classe: "class WKWebView could not be found". A classe existe no
// aparelho — o que faltava era mandar o ligador trazer a caixa onde ela mora.
//
// Quem fazia isso era a dependencia objc2-web-kit, que tivemos que remover
// porque ela nao traz o WKWebView no iPhone. Ao tirar a dependencia, este
// pedaco dela veio junto sem querer.
#[link(name = "WebKit", kind = "framework")]
extern "C" {}

extern_class!(
    /// O navegador embutido do sistema — o mesmo motor do Safari.
    #[unsafe(super(UIView, UIResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[derive(Debug, PartialEq, Eq, Hash)]
    pub struct WKWebView;
);

pub struct Ivars {
    navegador: Retained<WKWebView>,
}

define_class!(
    #[unsafe(super(UIViewController))]
    #[name = "TelaDeEntrada"]
    #[ivars = Ivars]
    pub struct TelaDeEntrada;

    unsafe impl NSObjectProtocol for TelaDeEntrada {}

    impl TelaDeEntrada {
        #[unsafe(method(viewDidLoad))]
        fn view_did_load(&self) {
            let _: () = unsafe { msg_send![super(self), viewDidLoad] };
            tracing::info!("tela de entrada: viewDidLoad");

            let Some(vista) = self.view() else {
                tracing::error!("tela de entrada: sem vista");
                return;
            };

            // Fundo escuro: enquanto a pagina nao pinta, o que aparece e isto,
            // e branco piscando antes de uma tela escura fica feio.
            unsafe { vista.setBackgroundColor(Some(&UIColor::blackColor())) };

            let navegador = self.ivars().navegador.clone();
            unsafe {
                navegador.setFrame(vista.bounds());
                navegador.setAutoresizingMask(
                    UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
                );
                // Quem responde as perguntas do navegador somos nos.
                let _: () = msg_send![&*navegador, setNavigationDelegate: &*self];
            }
            vista.addSubview(&navegador);

            abrir_a_pagina(&navegador);
        }

        // O WEBKIT PERGUNTA, E QUEM RESPONDE E O BLOCO.
        //
        // Este metodo nao devolve a decisao: ele recebe um bloco, e a decisao
        // e chamar esse bloco. Chamar UMA vez, sempre — se o caminho sair
        // daqui sem chamar, o WebKit fica esperando pra sempre e a pagina
        // trava sem dizer por que.
        //
        // O metodo e declarado solto, sem dizer que a classe segue o
        // protocolo WKNavigationDelegate: no iPhone esse protocolo vem sem
        // este metodo, porque a assinatura dele menciona o WKWebView que a
        // biblioteca nao traz. Nao faz falta — o Objective-C pergunta ao
        // objeto se ele atende o recado, e atende quem implementa.
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decidir(
            &self,
            _navegador: &AnyObject,
            acao: &AnyObject,
            decisao: &DynBlock<dyn Fn(isize)>,
        ) {
            let texto = endereco_da_acao(acao).unwrap_or_default();

            if texto.starts_with("deathnote:") {
                tracing::info!("tela de entrada: abrindo o jogo");
                decisao.call((NAO_NAVEGUE,));

                if let Some(nav) = self.navigationController() {
                    tocar_endereco_no_nav(&nav, self.mtm(), &texto);
                } else {
                    tracing::error!("tela de entrada: sem controlador de navegacao");
                }
                return;
            }

            decisao.call((PODE_NAVEGAR,));
        }
    }
);

impl TelaDeEntrada {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        // initWithFrame: e suficiente — o WKWebView monta sozinho a
        // configuracao padrao, que e a que queremos.
        let navegador: Retained<WKWebView> =
            unsafe { msg_send![WKWebView::alloc(mtm), initWithFrame: CGRect::ZERO] };

        let this = Self::alloc(mtm).set_ivars(Ivars { navegador });
        unsafe { msg_send![super(this), init] }
    }
}

/// O endereco que o WebKit quer visitar, como texto.
fn endereco_da_acao(acao: &AnyObject) -> Option<String> {
    let pedido: Retained<NSURLRequest> = unsafe { msg_send![acao, request] };
    let endereco = unsafe { pedido.URL() }?;
    Some(unsafe { endereco.absoluteString() }?.to_string())
}

fn abrir_a_pagina(navegador: &WKWebView) {
    let Some(endereco) = NSURL::URLWithString(&NSString::from_str(ENDERECO_DA_ENTRADA)) else {
        tracing::error!("endereco da entrada invalido: {ENDERECO_DA_ENTRADA}");
        return;
    };
    let pedido = unsafe { NSURLRequest::requestWithURL(&endereco) };
    let _: () = unsafe { msg_send![navegador, loadRequest: &*pedido] };
    tracing::info!("tela de entrada: pedindo {ENDERECO_DA_ENTRADA}");
}
