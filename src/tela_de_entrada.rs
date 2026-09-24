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

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{define_class, msg_send, DefinedClass as _, MainThreadOnly};
use objc2_core_foundation::CGRect;
use objc2_foundation::{MainThreadMarker, NSObjectProtocol, NSString, NSURLRequest, NSURL};
use objc2_ui_kit::{UIColor, UIViewAutoresizing, UIViewController};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKWebView,
    WKWebViewConfiguration,
};

use crate::scene_delegate::tocar_endereco_no_nav;

/// A pagina que o aplicativo abre.
const ENDERECO_DA_ENTRADA: &str = "https://deathnotestore.com.br/jogar/";

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

            let vista = self.view().expect("o controlador sempre tem vista aqui");

            // Fundo escuro: enquanto a pagina nao pinta, o que aparece e isto,
            // e branco piscando antes de uma tela escura fica feio.
            unsafe { vista.setBackgroundColor(Some(&UIColor::blackColor())) };

            let navegador = self.ivars().navegador.clone();
            unsafe {
                navegador.setFrame(vista.bounds());
                navegador.setAutoresizingMask(
                    UIViewAutoresizing::FlexibleWidth | UIViewAutoresizing::FlexibleHeight,
                );
                navegador.setNavigationDelegate(Some(ProtocolObject::from_ref(self)));
            }
            vista.addSubview(&navegador);

            abrir_a_pagina(&navegador);
        }
    }

    unsafe impl WKNavigationDelegate for TelaDeEntrada {
        // O WEBKIT PERGUNTA, E QUEM RESPONDE E O BLOCO.
        //
        // Este metodo nao devolve a decisao: ele recebe um bloco e a decisao
        // e chamar esse bloco. Chamar UMA vez, sempre — se o caminho sair
        // daqui sem chamar, o WebKit fica esperando pra sempre e a pagina
        // trava sem dizer por que.
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        fn decidir(
            &self,
            _navegador: &WKWebView,
            acao: &WKNavigationAction,
            decisao: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let endereco = unsafe { acao.request().URL() };
            let texto = endereco
                .as_ref()
                .and_then(|u| unsafe { u.absoluteString() })
                .map(|s| s.to_string())
                .unwrap_or_default();

            if texto.starts_with("deathnote:") {
                tracing::info!("tela de entrada: abrindo o jogo");
                decisao.call((WKNavigationActionPolicy::Cancel,));

                if let Some(nav) = self.navigationController() {
                    tocar_endereco_no_nav(&nav, self.mtm(), &texto);
                } else {
                    tracing::error!("tela de entrada: sem controlador de navegacao");
                }
                return;
            }

            decisao.call((WKNavigationActionPolicy::Allow,));
        }
    }
);

impl TelaDeEntrada {
    pub fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let config = unsafe { WKWebViewConfiguration::new(mtm) };
        let navegador = unsafe {
            WKWebView::initWithFrame_configuration(
                WKWebView::alloc(mtm),
                CGRect::ZERO,
                &config,
            )
        };

        let this = Self::alloc(mtm).set_ivars(Ivars { navegador });
        unsafe { msg_send![super(this), init] }
    }
}

fn abrir_a_pagina(navegador: &WKWebView) {
    let endereco = NSURL::URLWithString(&NSString::from_str(ENDERECO_DA_ENTRADA));
    let Some(endereco) = endereco else {
        tracing::error!("endereco da entrada invalido: {ENDERECO_DA_ENTRADA}");
        return;
    };
    let pedido = unsafe { NSURLRequest::requestWithURL(&endereco) };
    let _ = unsafe { navegador.loadRequest(&pedido) };
    tracing::info!("tela de entrada: pedindo {ENDERECO_DA_ENTRADA}");
}
