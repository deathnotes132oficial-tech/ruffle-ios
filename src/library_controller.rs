use std::cell::{OnceCell, RefCell};

use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{class, define_class, msg_send, sel, AllocAnyThread, DefinedClass as _, Message};
use objc2_core_data::{
    NSFetchedResultsChangeType, NSFetchedResultsController, NSFetchedResultsControllerDelegate,
    NSFetchedResultsSectionInfo,
};
use objc2_foundation::{
    ns_string, MainThreadMarker, NSArray, NSBundle, NSCoder, NSIndexPath, NSInteger, NSObject,
    NSObjectProtocol, NSString, NSURL,
};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_ui_kit::{
    NSDataAsset, UIBarButtonItem, UIButton, UIButtonType, UIColor, UIControlEvents,
    UIControlState, UIDocumentPickerDelegate, UIDocumentPickerViewController, UIFont, UILabel,
    UIPasteboard, UITableView, UITableViewCell, UITableViewCellEditingStyle,
    UITableViewController, UITableViewDataSource, UITableViewRowAnimation, UIView,
};
#[allow(deprecated)]
use objc2_ui_kit::{UIDocumentPickerMode, UIStoryboardSegue};
use ruffle_core::tag_utils::SwfMovie;
use ruffle_core::PlayerBuilder;
use ruffle_frontend_utils::backends::audio::CpalAudioBackend;

use crate::edit_controller::EditController;
use crate::storage::Movie;
use crate::scene_delegate::tocar_endereco_no_nav;
use crate::{storage, PlayerController, PlayerView};

/// O AVISO QUE A LICENCA EXIGE.
///
/// O Ruffle e o ruffle-ios sao MIT ou Apache 2.0, a escolha de quem usa. A MIT
/// foi a escolhida, e ela cobra uma coisa so: que este aviso apareca junto com
/// o programa. E por isso que ele esta aqui, e nao num arquivo que ninguem abre.
const LICENCA: &str = "Este aplicativo usa o Ruffle, um tocador de Flash de \
     codigo aberto.\n\n\
     Copyright (c) 2018-2022 Ruffle LLC <ruffle@ruffle.rs> e colaboradores do \
     Ruffle (https://github.com/ruffle-rs/ruffle)\n\n\
     Permission is hereby granted, free of charge, to any person obtaining a \
     copy of this software and associated documentation files (the \"Software\"), \
     to deal in the Software without restriction, including without limitation \
     the rights to use, copy, modify, merge, publish, distribute, sublicense, \
     and/or sell copies of the Software, and to permit persons to whom the \
     Software is furnished to do so, subject to the following conditions:\n\n\
     The above copyright notice and this permission notice shall be included in \
     all copies or substantial portions of the Software.\n\n\
     THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR \
     IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, \
     FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL \
     THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER \
     LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING \
     FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER \
     DEALINGS IN THE SOFTWARE.\n\n\
     CF Client nao e o Ruffle e nao fala pelo projeto Ruffle.";

// There is no standardized UTI for SWFs, so this is one we picked.
pub const SWF_UTI: &str = "com.adobe.swf";

// Temporary until we publish the package
pub const RUF_UTI: &str = "com.example.rs.ruffle.bundle";

#[derive(Debug)]
pub struct Ivars {
    logo_view: OnceCell<Retained<PlayerView>>,
    /// O cabecalho que diz o que o aplicativo faz. Ver `montar_cabecalho`.
    cabecalho: RefCell<Option<Retained<UIView>>>,
    /// As pecas dele, guardadas pra recolocar quando a largura mudar.
    pecas: RefCell<
        Option<(
            Retained<UILabel>,
            Retained<UILabel>,
            Retained<UIButton>,
            Retained<UIButton>,
        )>,
    >,
    fetched_movies: Retained<NSFetchedResultsController<Movie>>,
}

define_class!(
    #[unsafe(super(UITableViewController))]
    #[name = "LibraryController"]
    #[ivars = Ivars]
    #[derive(Debug)]
    pub struct LibraryController;

    unsafe impl NSObjectProtocol for LibraryController {}

    /// UIViewController.
    impl LibraryController {
        #[unsafe(method_id(initWithNibName:bundle:))]
        fn _init_with_nib_name_bundle(
            this: Allocated<Self>,
            nib_name_or_nil: Option<&NSString>,
            nib_bundle_or_nil: Option<&NSBundle>,
        ) -> Retained<Self> {
            tracing::info!("library init");
            let this = this.set_ivars(Ivars {
                logo_view: Default::default(),
                cabecalho: RefCell::new(None),
                pecas: RefCell::new(None),
                fetched_movies: storage::all_movies(),
            });
            let this: Retained<Self> = unsafe {
                msg_send![super(this), initWithNibName: nib_name_or_nil, bundle: nib_bundle_or_nil]
            };
            unsafe {
                this.ivars()
                    .fetched_movies
                    .setDelegate(Some(ProtocolObject::from_ref(&*this)))
            };
            this
        }

        #[unsafe(method_id(initWithCoder:))]
        fn _init_with_coder(this: Allocated<Self>, coder: &NSCoder) -> Option<Retained<Self>> {
            tracing::info!("library init");
            let this = this.set_ivars(Ivars {
                logo_view: Default::default(),
                cabecalho: RefCell::new(None),
                pecas: RefCell::new(None),
                fetched_movies: storage::all_movies(),
            });
            let this: Option<Retained<Self>> =
                unsafe { msg_send![super(this), initWithCoder: coder] };
            if let Some(this) = &this {
                unsafe {
                    this.ivars()
                        .fetched_movies
                        .setDelegate(Some(ProtocolObject::from_ref(&**this)));
                }
            }
            this
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

        #[unsafe(method(prepareForSegue:sender:))]
        #[allow(deprecated)]
        fn _prepare_for_segue(&self, segue: &UIStoryboardSegue, sender: Option<&NSObject>) {
            self.prepare_for_segue(segue, sender.expect("has sender"));
        }
    }

    /// Storyboard
    /// See storyboard_connections.h
    impl LibraryController {
        #[unsafe(method(setLogoView:))]
        fn _set_logo_view(&self, view: Option<&AnyObject>) {
            tracing::trace!("library set logo view");
            let view = view
                .expect("logo view not null")
                .downcast_ref::<PlayerView>()
                .expect("logo view not a PlayerView");
            self.ivars()
                .logo_view
                .set(view.retain())
                .expect("only set logo view once");
        }

        #[unsafe(method(toggleEditing:))]
        fn _toggle_editing(&self, button: Option<&AnyObject>) {
            tracing::trace!("library toggle editing");
            let button = button
                .expect("edit button not null")
                .downcast_ref::<UIBarButtonItem>()
                .expect("edit button not UIBarButtonItem");
            self.toggle_editing(button);
        }

        #[unsafe(method(cancelEditItem:))]
        #[allow(deprecated)]
        fn _cancel_edit_item(&self, _segue: &UIStoryboardSegue) {}

        #[unsafe(method(saveEditItem:))]
        #[allow(deprecated)]
        fn _save_edit_item(&self, segue: &UIStoryboardSegue) {
            self.save_item(segue);
        }

        #[unsafe(method(showDocumentPicker:))]
        #[allow(deprecated)]
        fn _show_document_picker(&self, _sender: Option<&AnyObject>) {
            self.show_document_picker();
        }

        #[unsafe(method(abrirEndereco:))]
        fn _abrir_endereco(&self, _sender: Option<&AnyObject>) {
            self.abrir_endereco();
        }

        #[unsafe(method(mostrarLicencas:))]
        fn _mostrar_licencas(&self, _sender: Option<&AnyObject>) {
            self.mostrar_licencas();
        }

        #[unsafe(method(mostrarRegistro:))]
        fn _mostrar_registro(&self, _sender: Option<&AnyObject>) {
            self.mostrar_registro();
        }
    }

    #[allow(non_snake_case)]
    unsafe impl UITableViewDataSource for LibraryController {
        #[unsafe(method(tableView:numberOfRowsInSection:))]
        fn tableView_numberOfRowsInSection(
            &self,
            _table_view: &UITableView,
            section: NSInteger,
        ) -> NSInteger {
            // Sem secao ainda e uma resposta legitima: a tabela pergunta
            // antes da busca terminar, e zero e a verdade nesse instante.
            let Some(sections) = (unsafe { self.ivars().fetched_movies.sections() }) else {
                return 0;
            };
            if section as usize >= sections.count() {
                return 0;
            }
            let section_info = sections.objectAtIndex(section as usize);
            unsafe { section_info.numberOfObjects() as isize }
        }

        #[unsafe(method(numberOfSectionsInTableView:))]
        fn numberOfSectionsInTableView(&self, _table_view: &UITableView) -> NSInteger {
            match unsafe { self.ivars().fetched_movies.sections() } {
                Some(s) => s.count() as NSInteger,
                None => 0,
            }
        }

        #[unsafe(method_id(tableView:cellForRowAtIndexPath:))]
        fn tableView_cellForRowAtIndexPath(
            &self,
            table_view: &UITableView,
            index_path: &NSIndexPath,
        ) -> Retained<UITableViewCell> {
            let cell = table_view.dequeueReusableCellWithIdentifier_forIndexPath(
                ns_string!("library-item"),
                index_path,
            );
            self.configure_cell(&cell, index_path);
            cell
        }

        #[unsafe(method_id(tableView:titleForHeaderInSection:))]
        fn tableView_titleForHeaderInSection(
            &self,
            _table_view: &UITableView,
            _section: NSInteger,
        ) -> Option<Retained<NSString>> {
            Some(NSString::from_str("Arquivos neste aparelho"))
        }

        #[unsafe(method(tableView:commitEditingStyle:forRowAtIndexPath:))]
        fn tableView_commitEditingStyle_forRowAtIndexPath(
            &self,
            _table_view: &UITableView,
            editing_style: UITableViewCellEditingStyle,
            index_path: &NSIndexPath,
        ) {
            if editing_style == UITableViewCellEditingStyle::Delete {
                let movie = unsafe { self.ivars().fetched_movies.objectAtIndexPath(&index_path) };
                storage::delete_movie(&movie);
            }
        }

        // TODO: Implement moving (requires keeping the order in CoreData).
    }

    // For usage, see:
    // https://developer.apple.com/library/archive/samplecode/CoreDataBooks/Listings/Classes_RootViewController_m.html
    #[allow(non_snake_case)]
    unsafe impl NSFetchedResultsControllerDelegate for LibraryController {
        #[unsafe(method(controllerWillChangeContent:))]
        fn controllerWillChangeContent(&self, _controller: &NSFetchedResultsController) {
            self.tableView().unwrap().beginUpdates();
        }

        #[unsafe(method(controller:didChangeObject:atIndexPath:forChangeType:newIndexPath:))]
        fn controller_didChangeObject_atIndexPath_forChangeType_newIndexPath(
            &self,
            _controller: &NSFetchedResultsController,
            _an_object: &AnyObject,
            index_path: Option<&NSIndexPath>,
            r#type: NSFetchedResultsChangeType,
            new_index_path: Option<&NSIndexPath>,
        ) {
            let table_view = self.tableView().unwrap();

            match r#type {
                NSFetchedResultsChangeType::Insert => table_view
                    .insertRowsAtIndexPaths_withRowAnimation(
                        &NSArray::from_slice(&[new_index_path.unwrap()]),
                        UITableViewRowAnimation::Automatic,
                    ),
                NSFetchedResultsChangeType::Delete => table_view
                    .deleteRowsAtIndexPaths_withRowAnimation(
                        &NSArray::from_slice(&[index_path.unwrap()]),
                        UITableViewRowAnimation::Automatic,
                    ),
                NSFetchedResultsChangeType::Update => self.configure_cell(
                    &table_view
                        .cellForRowAtIndexPath(index_path.unwrap())
                        .unwrap(),
                    index_path.unwrap(),
                ),
                NSFetchedResultsChangeType::Move => {
                    table_view.deleteRowsAtIndexPaths_withRowAnimation(
                        &NSArray::from_slice(&[index_path.unwrap()]),
                        UITableViewRowAnimation::Automatic,
                    );
                    table_view.insertRowsAtIndexPaths_withRowAnimation(
                        &NSArray::from_slice(&[new_index_path.unwrap()]),
                        UITableViewRowAnimation::Automatic,
                    );
                }
                _ => {}
            }
        }

        #[unsafe(method(controllerDidChangeContent:))]
        fn controllerDidChangeContent(&self, _controller: &NSFetchedResultsController) {
            self.tableView().unwrap().endUpdates();
        }
    }

    #[allow(non_snake_case)]
    unsafe impl UIDocumentPickerDelegate for LibraryController {
        #[unsafe(method(documentPickerWasCancelled:))]
        fn documentPickerWasCancelled(&self, _controller: &UIDocumentPickerViewController) {
            tracing::info!("cancelled document picker");
        }

        #[unsafe(method(documentPicker:didPickDocumentAtURL:))]
        fn documentPicker_didPickDocumentAtURL(
            &self,
            _controller: &UIDocumentPickerViewController,
            url: &NSURL,
        ) {
            tracing::info!("completed document picker: {url:?}");
            if storage::movie_from_url(&url).is_none() {
                storage::add_movie(&url);
            } else {
                // TODO: Give the user an option here?
                tracing::error!("did not add existing movie {url:?}");
            }
        }
    }
);

impl LibraryController {
    /// A ANIMACAO PODE NAO EXISTIR MAIS.
    ///
    /// Ela era o cabecalho da tabela: uma faixa preta de 96 pontos com um
    /// logotipo de 40 pixels perdido no meio, ocupando um quarto da tela. Saiu
    /// do storyboard, e com ela foi embora a unica fonte de `logo_view`.
    ///
    /// Entao isto devolve Option e quem chama confere. Antes era um `expect`,
    /// e um `expect` aqui derrubaria o aplicativo no arranque.
    fn logo_view(&self) -> Option<&PlayerView> {
        self.ivars().logo_view.get().map(|v| &**v)
    }

    fn view_did_load(&self) {
        tracing::info!("library viewDidLoad");

        self.montar_barra_de_baixo();
        self.garantir_exemplo();
        self.setup_logo();

        unsafe {
            self.ivars()
                .fetched_movies
                .performFetch()
                .expect("failed fetching movies")
        };

        // O CABECALHO SO DEPOIS DA BUSCA, E ISSO DERRUBOU O APLICATIVO.
        //
        // Montar o cabecalho obriga a tabela a se medir na hora, e medir-se
        // e perguntar a fonte de dados quantas secoes existem. Com a chamada
        // la em cima, a pergunta chegava ANTES do performFetch: o
        // fetched_movies ainda nao tinha secao nenhuma, o sections() devolvia
        // None, e o unwrap derrubava tudo no arranque.
        //
        // A foto do simulador mostrou a tela inicial do iPhone no lugar do
        // aplicativo, e o registro trouxe a linha exata.
        self.montar_cabecalho();
    }

    /// UM ARQUIVO DE EXEMPLO, PRA BIBLIOTECA NUNCA NASCER VAZIA.
    ///
    /// Sem isto, quem abre o aplicativo pela primeira vez ve uma lista vazia e
    /// um botao de escolher arquivo — e nao tem arquivo nenhum pra escolher,
    /// porque ninguem guarda .swf no iPhone. O aplicativo parece nao fazer
    /// nada, e quem so vai abrir uma vez (um revisor, um curioso) desiste ali.
    ///
    /// O exemplo e a mesma animacao do cabecalho: ja esta dentro do aplicativo,
    /// nao pesa nada a mais, e prova o que o aplicativo faz com um toque.
    ///
    /// Vale so na primeira vez. Se o jogador apagar da lista, nao volta.
    fn garantir_exemplo(&self) {
        // CERCA DE SEGURANCA: NADA AQUI DENTRO PODE DERRUBAR O APLICATIVO.
        //
        // Isto aqui e uma gentileza — uma amostra pra lista nao nascer vazia.
        // Gentileza que fecha o aplicativo e pior que gentileza nenhuma, e
        // foi o que aconteceu: um panic! la no fundo do caminho (no acesso
        // com escopo de seguranca) matava o programa, e so na PRIMEIRA
        // abertura, porque depois esta funcao nem roda.
        //
        // Os pontos conhecidos ja foram consertados um a um. Esta cerca
        // existe pros que ninguem viu ainda: se algo aqui dentro estourar, o
        // aplicativo perde a amostra e segue de pe.
        let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Self::criar_exemplo();
        }));
        if resultado.is_err() {
            tracing::error!("nao deu pra criar o exemplo; seguindo sem ele");
        }
    }

    /// O trabalho em si. Chamado so pelo garantir_exemplo, dentro da cerca.
    fn criar_exemplo() {
        // RECURSO PROPRIO, SEPARADO DA MARCA DO CABECALHO.
        //
        // Antes a amostra era a mesma animacao do cabecalho: um palco de
        // 1000x600 quase todo vazio com o logotipo pequeno passeando. Ela
        // TOCAVA — as fotos do simulador provam — mas na tela lia como tela
        // vazia, e foi o que a Apple chamou de "the sample file did not
        // appear". O cabecalho continua com a marca; a amostra passou a ser
        // uma animacao que ocupa o palco inteiro.
        let Some(asset) = NSDataAsset::initWithName(NSDataAsset::alloc(), ns_string!("exemplo-anim"))
        else {
            return;
        };

        let Ok(casa) = std::env::var("HOME") else {
            return;
        };
        let alvo = std::path::PathBuf::from(casa)
            .join("Documents")
            .join("Exemplo.swf");

        // Ja existe: ou ja foi criado antes, ou o jogador apagou da lista e o
        // arquivo ficou. Nos dois casos, nao se mexe.
        if alvo.exists() {
            return;
        }

        let dados = unsafe { asset.data() };
        // SAFETY: so lemos os bytes, e o NSData vive ate o fim desta funcao.
        let bytes = unsafe { dados.as_bytes_unchecked() };
        if std::fs::write(&alvo, bytes).is_err() {
            return;
        }

        let Some(caminho) = alvo.to_str() else {
            return;
        };
        let texto = NSString::from_str(caminho);
        // Pelo seletor, e nao pela traducao: o nome do metodo do Objective-C
        // nao muda de versao pra versao.
        let url: Retained<NSURL> = unsafe { msg_send![class!(NSURL), fileURLWithPath: &*texto] };

        if storage::movie_from_url(&url).is_none() {
            storage::add_movie(&url);
        }
    }

    /// O CABECALHO QUE DIZ O QUE O APLICATIVO FAZ.
    ///
    /// ── O QUE ESTAVA AQUI ──
    ///
    /// Uma PlayerView de 96 pontos tocando a animacao do logotipo: uma faixa
    /// preta ocupando um quarto da tela, com um desenho de 40 pixels perdido
    /// no meio dela. Em cima, "Editar" e "+" nos cantos. Nada na tela dizia o
    /// que o aplicativo e, nem o que fazer ali.
    ///
    /// A Apple recusou duas vezes por isso: "interface apertada ou disposta de
    /// um jeito que dificulta completar tarefas".
    ///
    /// ── O QUE ENTRA ──
    ///
    /// Uma linha dizendo o que o aplicativo toca, e as duas acoes escritas por
    /// extenso, grandes o bastante pra se acertar com o dedo. A tarefa deixa
    /// de ser adivinhacao.
    ///
    /// ── POR QUE NA MAO, E NAO PELO STORYBOARD ──
    ///
    /// O storyboard fixava altura de rodape em 18 pontos e o texto precisava
    /// de 60; corrigido isso, a barra de baixo do iOS 26 passou a flutuar
    /// sobre o conteudo e cobriu o mesmo texto. Duas sobreposicoes seguidas,
    /// as duas por eu depender de medida automatica que nao controlo. Aqui
    /// cada peca tem posicao e tamanho que eu escrevo, e sobrepor vira
    /// impossivel em vez de improvavel.
    fn montar_cabecalho(&self) {
        let mtm = MainThreadMarker::from(self);

        let fundo = unsafe { UIView::initWithFrame(mtm.alloc(), CGRect::ZERO) };

        // A marca, pequena e no canto, em vez de uma faixa preta.
        let marca = unsafe { UILabel::initWithFrame(mtm.alloc(), CGRect::ZERO) };
        unsafe {
            marca.setText(Some(ns_string!("DN")));
            marca.setFont(Some(&UIFont::boldSystemFontOfSize(26.0)));
            if let Some(ouro) = Self::cor("dn-ouro") {
                marca.setTextColor(Some(&ouro));
            }
        }
        fundo.addSubview(&marca);

        let frase = unsafe { UILabel::initWithFrame(mtm.alloc(), CGRect::ZERO) };
        unsafe {
            frase.setText(Some(&NSString::from_str(
                "Toca arquivos Flash (.swf) do seu aparelho ou da internet.",
            )));
            frase.setFont(Some(&UIFont::systemFontOfSize(15.0)));
            frase.setNumberOfLines(2);
            frase.setTextColor(Some(&UIColor::colorWithWhite_alpha(1.0, 0.75)));
        }
        fundo.addSubview(&frase);

        let abrir = self.botao_grande("Abrir arquivo .swf", sel!(showDocumentPicker:), true);
        let colar = self.botao_grande("Colar endereço", sel!(abrirEndereco:), false);
        fundo.addSubview(&abrir);
        fundo.addSubview(&colar);

        *self.ivars().cabecalho.borrow_mut() = Some(fundo);
        *self.ivars().pecas.borrow_mut() = Some((marca, frase, abrir, colar));
        self.posicionar_cabecalho();
    }

    /// Um botao de verdade: alto o bastante pro dedo, com o texto escrito.
    fn botao_grande(
        &self,
        titulo: &str,
        acao: objc2::runtime::Sel,
        cheio: bool,
    ) -> Retained<UIButton> {
        let mtm = MainThreadMarker::from(self);
        let b = unsafe { UIButton::buttonWithType(UIButtonType::System, mtm) };
        let texto = NSString::from_str(titulo);
        let ouro = Self::cor("dn-ouro");

        unsafe {
            b.setTitle_forState(Some(&texto), UIControlState::Normal);
            if let Some(rotulo) = b.titleLabel() {
                rotulo.setFont(Some(&UIFont::boldSystemFontOfSize(17.0)));
            }
            if cheio {
                if let Some(ouro) = ouro.as_ref() {
                    b.setBackgroundColor(Some(ouro));
                }
                // Texto escuro sobre o dourado: contraste de leitura.
                b.setTitleColor_forState(
                    Some(&UIColor::colorWithRed_green_blue_alpha(0.15, 0.20, 0.31, 1.0)),
                    UIControlState::Normal,
                );
            } else if let Some(ouro) = ouro.as_ref() {
                b.setTitleColor_forState(Some(ouro), UIControlState::Normal);
                let cg: *mut objc2::runtime::AnyObject = msg_send![&**ouro, CGColor];
                let _: () = msg_send![&*b.layer(), setBorderColor: cg];
                b.layer().setBorderWidth(1.5);
            }
            b.layer().setCornerRadius(12.0);
            b.addTarget_action_forControlEvents(Some(self), acao, UIControlEvents::TouchUpInside);
        }
        b
    }

    /// Uma cor do catalogo, pelo nome. None quando ela nao existir.
    fn cor(nome: &str) -> Option<Retained<UIColor>> {
        let n = NSString::from_str(nome);
        unsafe { msg_send![class!(UIColor), colorNamed: &*n] }
    }

    /// AS MEDIDAS, REFEITAS SEMPRE QUE A LARGURA MUDA.
    ///
    /// Posicao fixa nao serve: a tela gira, e a largura em paisagem nao e a
    /// mesma de um aparelho pro outro. Cada peca sai de uma conta sobre a
    /// largura de AGORA.
    fn posicionar_cabecalho(&self) {
        let Some(tabela) = self.tableView() else {
            return;
        };
        let largura = tabela.bounds().size.width;
        if largura < 2.0 {
            return;
        }

        let borda = 24.0;
        let util = largura - borda * 2.0;

        let topo = 18.0;
        let alto_frase = 44.0;
        let vao = 16.0;
        let alto_botao = 48.0;
        let rodape = 20.0;
        let altura = topo + alto_frase + vao + alto_botao + rodape;

        if let Some((marca, frase, abrir, colar)) = self.ivars().pecas.borrow().as_ref() {
            let largo_marca = 46.0;
            marca.setFrame(CGRect::new(
                CGPoint::new(borda, topo),
                CGSize::new(largo_marca, alto_frase),
            ));
            frase.setFrame(CGRect::new(
                CGPoint::new(borda + largo_marca + 12.0, topo),
                CGSize::new(util - largo_marca - 12.0, alto_frase),
            ));

            let largo_botao = (util - 14.0) / 2.0;
            let y_botao = topo + alto_frase + vao;
            abrir.setFrame(CGRect::new(
                CGPoint::new(borda, y_botao),
                CGSize::new(largo_botao, alto_botao),
            ));
            colar.setFrame(CGRect::new(
                CGPoint::new(borda + largo_botao + 14.0, y_botao),
                CGSize::new(largo_botao, alto_botao),
            ));
        }

        // O TABLEHEADERVIEW SO ACEITA ALTURA NOVA SE FOR REATRIBUIDO.
        //
        // Mudar o frame dele sozinho nao move a lista uma linha: a tabela
        // guardou a altura antiga. Atribuir de novo e o que faz ela medir.
        if let Some(cabecalho) = self.ivars().cabecalho.borrow().as_ref() {
            cabecalho.setFrame(CGRect::new(CGPoint::ZERO, CGSize::new(largura, altura)));
            unsafe {
                let _: () = msg_send![&*tabela, setTableHeaderView: &**cabecalho];
            }
        }
    }

    /// NAO HA MAIS BARRA DE BAIXO.
    ///
    /// Ela tinha "Colar endereço" e "Licenças". O primeiro subiu pro cabecalho
    /// e virou botao escrito; o segundo foi pra barra de cima, do lado do
    /// nome.
    ///
    /// O motivo de ela sumir por inteiro: no iOS 26 essa barra FLUTUA sobre o
    /// conteudo. Com o rodape de texto, ela cobria o texto; sem ele, a foto do
    /// simulador mostrou a pilula de "Licenças" pousada em cima da linha do
    /// arquivo. Enquanto existir uma barra flutuante, sempre havera alguma
    /// coisa embaixo dela.
    ///
    /// Sem barra, nao ha o que cobrir.
    fn montar_barra_de_baixo(&self) {
        unsafe {
            if let Some(nav) = self.navigationController() {
                let _: () = msg_send![&*nav, setToolbarHidden: true, animated: false];
            }
        }
    }

    /// Abre o endereço que estiver na área de transferência.
    ///
    /// Sem campo de digitação de propósito: ninguém digita um endereço de jogo
    /// à mão no celular — ele chega copiado, de uma mensagem ou de uma página.
    /// Colar é o gesto que a pessoa já ia fazer.
    fn abrir_endereco(&self) {
        let area = UIPasteboard::generalPasteboard();
        let copiado = unsafe { area.string() }.map(|s| s.to_string());

        let texto = match copiado {
            Some(t) if !t.trim().is_empty() => t.trim().to_string(),
            _ => {
                self.avisar(
                    "Nada copiado",
                    "Copie o endereço de um arquivo .swf e toque aqui de novo.",
                );
                return;
            }
        };

        let mtm = MainThreadMarker::from(self);
        let tocou = self
            .navigationController()
            .and_then(|nav| tocar_endereco_no_nav(&nav, mtm, &texto));

        if tocou.is_none() {
            self.avisar(
                "Endereço não serve",
                "O que está copiado não é um endereço que eu consiga abrir.",
            );
        }
    }

    /// O aviso de licenças que a MIT exige.
    fn mostrar_licencas(&self) {
        self.avisar("Licenças", LICENCA);
    }

    /// Uma caixinha com título, texto e um "OK".
    ///
    /// Montada por seletor do Objective-C em vez das traduções do objc2: são os
    /// mesmos nomes de sempre, e assim não há nome de tradução pra errar.
    /// O REGISTRO, NA AREA DE TRANSFERENCIA.
    ///
    /// O aplicativo anota o tempo todo quanta memoria esta usando e por onde
    /// passou, e guarda tambem o registro da sessao passada. Isso e o que
    /// explica uma morte por falta de memoria — e nao servia pra nada,
    /// porque nao havia como tirar do aparelho.
    ///
    /// Copiar e colar e o caminho mais curto: a pessoa ja sabe fazer, nao
    /// depende de cabo, de computador nem de permissao.
    ///
    /// Quando a sessao passada terminou sozinha, ela vem JUNTO e primeiro —
    /// e justamente a que ninguem consegue pedir na hora, porque quando o
    /// sistema encerra o aplicativo nao sobra tela pra avisar.
    fn mostrar_registro(&self) {
        let morreu = crate::registro::anterior_morreu();
        let mut texto = String::new();

        if morreu {
            if let Some(antes) = crate::registro::anterior() {
                texto.push_str("===== SESSAO ANTERIOR (terminou sozinha) =====\n");
                texto.push_str(&antes);
                texto.push_str("\n\n");
            }
        }

        if let Some(agora) = crate::registro::atual() {
            texto.push_str("===== SESSAO DE AGORA =====\n");
            texto.push_str(&agora);
        }

        if texto.is_empty() {
            self.avisar("Registro", "Ainda não há nada registrado.");
            return;
        }

        let linhas = texto.lines().count();
        let ns = NSString::from_str(&texto);
        let area = UIPasteboard::generalPasteboard();
        unsafe {
            let _: () = msg_send![&*area, setString: &*ns];
        }

        let mut mensagem = format!("Registro copiado ({linhas} linhas). Cole numa mensagem e envie.");
        if morreu {
            mensagem.push_str(
                "\n\nA sessão anterior terminou sozinha — o aplicativo foi encerrado pelo sistema.",
            );
        }
        self.avisar("Registro", &mensagem);
    }

    fn avisar(&self, titulo: &str, mensagem: &str) {
        let t = NSString::from_str(titulo);
        let m = NSString::from_str(mensagem);
        let ok = NSString::from_str("OK");

        unsafe {
            let caixa: Retained<AnyObject> = msg_send![
                class!(UIAlertController),
                alertControllerWithTitle: &*t,
                message: &*m,
                preferredStyle: 1isize,      // UIAlertControllerStyleAlert
            ];
            let acao: Retained<AnyObject> = msg_send![
                class!(UIAlertAction),
                actionWithTitle: &*ok,
                style: 0isize,               // UIAlertActionStyleDefault
                handler: std::ptr::null::<AnyObject>(),
            ];
            let _: () = msg_send![&*caixa, addAction: &*acao];
            let _: () = msg_send![
                self,
                presentViewController: &*caixa,
                animated: true,
                completion: std::ptr::null::<AnyObject>(),
            ];
        }
    }

    fn setup_logo(&self) {
        let Some(view) = self.logo_view() else {
            return;
        };
        // SEM LOGO, SEM DRAMA.
        //
        // A animacao que vinha aqui e a marca do Ruffle, e marca a licenca nao
        // da: MIT e Apache entregam o codigo e dizem, com todas as letras, que
        // nome e logo do autor continuam dele. Entao se o arquivo nao estiver
        // mais na caixa de recursos, a tela simplesmente abre sem ele — em vez
        // de o aplicativo morrer no arranque por causa de um enfeite.
        let Some(asset) = NSDataAsset::initWithName(NSDataAsset::alloc(), ns_string!("logo-anim"))
        else {
            tracing::info!("sem animacao de abertura; seguindo sem ela");
            return;
        };
        let data = unsafe { asset.data() };
        // SAFETY: SwfMovie::from_data won't modify the NSData.
        let bytes = unsafe { data.as_bytes_unchecked() };
        // O QUARTO ARGUMENTO E NOVO no Ruffle.
        //
        // Ele descreve um SWF carregado por Loader.loadBytes, caso em que o
        // carregador manda na caixa de areia do carregado. Esta animacao vem
        // de dentro do proprio aplicativo, nao foi carregada por ninguem:
        // None e a resposta certa, nao um preenchimento.
        // Enfeite nao derruba aplicativo: se o arquivo vier torto, a tela abre
        // sem ele. Um panic aqui mataria o app no arranque por causa de um
        // desenho de cabecalho.
        let Ok(movie) = SwfMovie::from_data(bytes, "file://logo-anim.swf".into(), None, None)
        else {
            tracing::warn!("a animacao de abertura nao pode ser lida; seguindo sem ela");
            return;
        };

        let renderer = view.create_renderer();

        let mut builder = PlayerBuilder::new()
            .with_renderer(renderer)
            .with_movie(movie);

        match CpalAudioBackend::new(None) {
            Ok(audio) => builder = builder.with_audio(audio),
            Err(e) => tracing::error!("Unable to create audio device: {e}"),
        }

        view.set_player(builder.build());
        // HACK: Skip first frame to avoid a flicker on startup
        // FIXME: This probably indicates a bug in our timing code?
        view.player_lock().run_frame();
    }

    fn view_is_appearing(&self, _animated: bool) {
        tracing::info!("library viewIsAppearing:");

        // A largura so e real a partir daqui, e muda quando a tela gira.
        self.posicionar_cabecalho();

        if let Some(view) = self.logo_view() {
            view.start();
        }
    }

    fn view_will_disappear(&self, _animated: bool) {
        tracing::info!("library viewWillDisappear:");

        if let Some(view) = self.logo_view() {
            view.stop();
        }
    }

    fn view_did_disappear(&self, _animated: bool) {
        tracing::info!("library viewDidDisappear:");

        if let Some(view) = self.logo_view() {
            view.player_lock().flush_shared_objects();
        }
    }

    #[allow(deprecated)]
    fn prepare_for_segue(&self, segue: &UIStoryboardSegue, sender: &NSObject) {
        let destination = segue.destinationViewController();
        tracing::info!(?destination, "prepareForSegue");

        // Identifiers are set up in the Storyboard
        let identifier = segue.identifier().expect("segue to have identifier");
        if &*identifier == ns_string!("add-item") {
            // No need to configure AddController
        } else if &*identifier == ns_string!("edit-item") {
            let edit_controller = destination.downcast_ref::<EditController>().unwrap();
            let cell = sender.downcast_ref::<UITableViewCell>().unwrap();

            let index_path = self.tableView().unwrap().indexPathForCell(&cell).unwrap();
            let movie = unsafe { self.ivars().fetched_movies.objectAtIndexPath(&index_path) };
            edit_controller.setup_movie(&movie);
        } else if &*identifier == ns_string!("run-item") {
            let player_controller = destination.downcast_ref::<PlayerController>().unwrap();
            let cell = sender.downcast_ref::<UITableViewCell>().unwrap();

            let index_path = self.tableView().unwrap().indexPathForCell(&cell).unwrap();
            let movie = unsafe { self.ivars().fetched_movies.objectAtIndexPath(&index_path) };
            player_controller.setup_movie(&movie);
        } else {
            unreachable!("unknown identifier for segue: {identifier:?}")
        }
    }

    #[allow(deprecated)]
    fn save_item(&self, segue: &UIStoryboardSegue) {
        tracing::info!("saveEditItem");
        let edit_controller = segue.sourceViewController();
        let edit_controller = edit_controller.downcast_ref::<EditController>().unwrap();
        dbg!(edit_controller); // TODO
    }

    #[allow(deprecated)]
    fn show_document_picker(&self) {
        tracing::info!("show document picker");
        let mtm = MainThreadMarker::from(self);
        let picker = UIDocumentPickerViewController::initWithDocumentTypes_inMode(
            mtm.alloc(),
            &NSArray::from_slice(&[ns_string!(RUF_UTI), ns_string!(SWF_UTI)]),
            UIDocumentPickerMode::Open,
        );
        picker.setDelegate(Some(ProtocolObject::from_ref(self)));
        // TODO: Consider setting picker.directoryURL to NSDownloadsDirectory,
        // as that's the likely place that people will have their SWFs.

        self.presentViewController_animated_completion(&picker, true, None);
    }

    fn toggle_editing(&self, button: &UIBarButtonItem) {
        let table_view = self.tableView().expect("has table view");
        let is_editing = !table_view.isEditing();
        table_view.setEditing_animated(is_editing, true);
        button.setTitle(Some(if is_editing {
            ns_string!("Pronto")
        } else {
            ns_string!("Editar")
        }));
    }

    fn configure_cell(&self, cell: &UITableViewCell, index_path: &NSIndexPath) {
        let subviews = cell.contentView().subviews();
        let title = subviews.objectAtIndex(1).downcast::<UILabel>().unwrap();
        let subtitle = subviews.objectAtIndex(2).downcast::<UILabel>().unwrap();

        let movie = unsafe { self.ivars().fetched_movies.objectAtIndexPath(index_path) };
        let cached_name = movie.cachedName();
        let url = movie.link();

        title.setText(Some(&cached_name));

        if url.isFileURL() {
            if let Some(_access) = storage::SecurityScopedResource::access(&url) {
                dbg!(&url, url.filePathURL());
                subtitle.setText(Some(
                    &url.filePathURL().unwrap().lastPathComponent().unwrap(),
                ));
            } else {
                subtitle.setText(Some(ns_string!("Arquivo deste aparelho")));
            }
        } else {
            subtitle.setText(Some(&url.absoluteString().unwrap()));
        }
    }
}
