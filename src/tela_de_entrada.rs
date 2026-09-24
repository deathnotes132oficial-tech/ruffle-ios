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

use crate::registro;
use crate::scene_delegate::tocar_endereco_no_nav;

/// A pagina que o aplicativo abre.
const ENDERECO_DA_ENTRADA: &str = "https://deathnotestore.com.br/jogar/";

/// O toque em "CONTINUAR" na tela do registro. Nao e endereco de internet:
/// e um recado da pagina pra ca, pelo mesmo caminho do botao do jogo.
const CONTINUAR: &str = "deathnote://continuar";

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

            // SE A SESSAO PASSADA MORREU NO MEIO, ISSO VEM PRIMEIRO.
            //
            // Quem fechou o aplicativo normalmente nunca ve esta tela: sair
            // passa pelo fundo, e o registro sabe disso. So aparece quando a
            // sessao terminou sem aviso — que e exatamente o caso que a gente
            // esta cacando.
            match registro::anterior() {
                Some(texto) if registro::anterior_morreu() => {
                    tracing::info!("tela de entrada: a sessao passada morreu, mostrando");
                    mostrar_o_registro(&navegador, &texto);
                }
                _ => abrir_a_pagina(&navegador),
            }
        }

        // A BARRA DE CIMA SO APARECE QUANDO SERVE PRA ALGO.
        //
        // Na tela de entrada ela e uma faixa azul vazia — azul do Ruffle, em
        // cima de uma tela escura que nao e do Ruffle. Nao ha pra onde voltar
        // daqui: esta e a primeira tela.
        //
        // Ela volta ao sair: quando o jogo abre por cima, a barra e o caminho
        // de volta pra ca. Por isso esconder e mostrar andam em par — esconder
        // sem mostrar deixaria o jogador preso dentro do jogo.
        #[unsafe(method(viewWillAppear:))]
        fn view_will_appear(&self, animado: bool) {
            let _: () = unsafe { msg_send![super(self), viewWillAppear: animado] };
            if let Some(nav) = self.navigationController() {
                unsafe { nav.setNavigationBarHidden_animated(true, animado) };
            }
        }

        #[unsafe(method(viewWillDisappear:))]
        fn view_will_disappear(&self, animado: bool) {
            let _: () = unsafe { msg_send![super(self), viewWillDisappear: animado] };
            if let Some(nav) = self.navigationController() {
                unsafe { nav.setNavigationBarHidden_animated(false, animado) };
            }
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

            // O "continuar" vem ANTES do teste geral de "deathnote:", senao
            // ele cairia no caminho do jogo e seria lido como endereco de
            // partida — que nao e.
            if texto.starts_with(CONTINUAR) {
                decisao.call((NAO_NAVEGUE,));
                registro::limpar_anterior();
                abrir_a_pagina(&self.ivars().navegador);
                return;
            }

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

/// A TELA DO REGISTRO.
///
/// E uma pagina montada aqui mesmo, sem sair do aparelho: nada e enviado a
/// lugar nenhum. O jogador ve, tira um print se quiser, e toca em CONTINUAR —
/// que apaga o registro e leva pra tela de entrada de sempre.
fn mostrar_o_registro(navegador: &WKWebView, texto: &str) {
    let corpo = escapar(texto);
    let pagina = format!(
        r#"<!DOCTYPE html>
<html lang="pt-BR"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<style>
  :root {{ color-scheme: dark; }}
  * {{ box-sizing: border-box; }}
  body {{ margin: 0; padding: 18px; background: #0B0A0D; color: #ECE5D8;
         font-family: system-ui, -apple-system, sans-serif; }}
  h1 {{ font-size: 17px; margin: 0 0 4px; color: #F0616D; }}
  p.sub {{ font-size: 13px; color: #787284; margin: 0 0 14px; line-height: 1.5; }}
  pre {{ background: #16141B; border: 1px solid #2A2632; border-radius: 12px;
         padding: 12px; font-size: 11px; line-height: 1.6; color: #CBD5E1;
         white-space: pre-wrap; word-break: break-word; margin: 0 0 16px; }}
  a.seguir {{ display: block; text-align: center; text-decoration: none;
              background: #C8102E; color: #fff; font-size: 16px; font-weight: 700;
              letter-spacing: .12em; padding: 15px 14px; border-radius: 14px; }}
</style></head><body>
<h1>O APLICATIVO FECHOU SOZINHO DA ULTIMA VEZ</h1>
<p class="sub">Tire um print desta tela e mande pro suporte. Depois toque em
continuar para entrar no jogo normalmente.</p>
<pre id="oque">{corpo}</pre>
<a class="seguir" href="{CONTINUAR}">CONTINUAR</a>
<script>
  // Os horarios estao guardados em segundos desde 1970, que e o formato que
  // nao depende de fuso nem de idioma. Aqui viram data legivel.
  var alvo = document.getElementById("oque");
  alvo.textContent = alvo.textContent.replace(/^\[(\d+)\]/gm, function (_, s) {{
    return "[" + new Date(parseInt(s, 10) * 1000).toLocaleString("pt-BR") + "]";
  }});
</script>
</body></html>"#
    );

    let vazio: Option<&NSURL> = None;
    let _: () = unsafe {
        msg_send![navegador, loadHTMLString: &*NSString::from_str(&pagina), baseURL: vazio]
    };
}

/// Impede que o conteudo do registro seja lido como marcacao da pagina.
fn escapar(texto: &str) -> String {
    texto
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
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
