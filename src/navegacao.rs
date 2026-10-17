//! O NAVEGADOR DA PILHA DE TELAS.
//!
//! POR QUE ISTO PRECISOU EXISTIR
//! -----------------------------
//! O PlayerController sempre disse, em supportedInterfaceOrientations, que o
//! jogo e deitado. Essa resposta nunca foi lida por ninguem.
//!
//! O motivo e uma regra do UIKit que nao esta escrita em lugar nenhum obvio:
//! quando as telas vivem dentro de um UINavigationController, quem o sistema
//! pergunta e o NAVEGADOR, nao a tela que esta em cima. E o navegador de
//! fabrica responde por si mesmo — no iPad, "aceito tudo".
//!
//! No iPhone isso passou despercebido, porque o Info.plist ja limitava o
//! aplicativo inteiro a paisagem. No iPad, onde a tela de abertura precisa
//! girar pra preencher o aparelho, a mesma permissao vazava pro jogo: o
//! tocador abria em pe, e os botoes de jogo — pensados pra uma tela larga —
//! se amontoavam num canto com metade da tela vazia.
//!
//! Esta classe nao faz nada alem de repassar a pergunta pra tela de cima.
//! Com isso cada tela volta a decidir a propria orientacao: a de abertura
//! gira e preenche o iPad, o jogo fica deitado.

use objc2::rc::{Allocated, Retained};
use objc2::{define_class, msg_send};
use objc2_foundation::{NSBundle, NSCoder, NSObjectProtocol, NSString};
use objc2_ui_kit::{UIInterfaceOrientationMask, UINavigationController, UIViewController};

#[derive(Default, Debug)]
pub struct Ivars {}

define_class!(
    #[unsafe(super(UINavigationController))]
    #[name = "NavegacaoPrincipal"]
    #[ivars = Ivars]
    #[derive(Debug)]
    pub struct NavegacaoPrincipal;

    unsafe impl NSObjectProtocol for NavegacaoPrincipal {}

    impl NavegacaoPrincipal {
        #[unsafe(method_id(initWithNibName:bundle:))]
        fn _init_with_nib_name_bundle(
            this: Allocated<Self>,
            nib_name_or_nil: Option<&NSString>,
            nib_bundle_or_nil: Option<&NSBundle>,
        ) -> Retained<Self> {
            let this = this.set_ivars(Ivars::default());
            unsafe {
                msg_send![super(this), initWithNibName: nib_name_or_nil, bundle: nib_bundle_or_nil]
            }
        }

        #[unsafe(method_id(initWithCoder:))]
        fn _init_with_coder(this: Allocated<Self>, coder: &NSCoder) -> Option<Retained<Self>> {
            let this = this.set_ivars(Ivars::default());
            unsafe { msg_send![super(this), initWithCoder: coder] }
        }

        /// A PERGUNTA, REPASSADA PRA TELA DE CIMA.
        ///
        /// Sem tela nenhuma na pilha — so acontece antes da primeira aparecer —
        /// responde o que o de fabrica responderia.
        #[unsafe(method(supportedInterfaceOrientations))]
        fn supportedInterfaceOrientations(&self) -> UIInterfaceOrientationMask {
            let topo: Option<Retained<UIViewController>> =
                unsafe { msg_send![self, topViewController] };
            match topo {
                Some(tela) => unsafe { msg_send![&*tela, supportedInterfaceOrientations] },
                None => unsafe { msg_send![super(self), supportedInterfaceOrientations] },
            }
        }

        #[unsafe(method(shouldAutorotate))]
        fn shouldAutorotate(&self) -> bool {
            let topo: Option<Retained<UIViewController>> =
                unsafe { msg_send![self, topViewController] };
            match topo {
                Some(tela) => unsafe { msg_send![&*tela, shouldAutorotate] },
                None => true,
            }
        }
    }
);
