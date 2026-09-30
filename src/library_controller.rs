use std::cell::OnceCell;

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
use objc2_ui_kit::{
    NSDataAsset, UIBarButtonItem, UIDocumentPickerDelegate, UIDocumentPickerViewController,
    UILabel, UIPasteboard, UITableView, UITableViewCell, UITableViewCellEditingStyle,
    UITableViewController, UITableViewDataSource, UITableViewRowAnimation,
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
     DN Flash nao e o Ruffle e nao fala pelo projeto Ruffle.";

// There is no standardized UTI for SWFs, so this is one we picked.
pub const SWF_UTI: &str = "com.adobe.swf";

// Temporary until we publish the package
pub const RUF_UTI: &str = "com.example.rs.ruffle.bundle";

#[derive(Debug)]
pub struct Ivars {
    logo_view: OnceCell<Retained<PlayerView>>,
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
    }

    #[allow(non_snake_case)]
    unsafe impl UITableViewDataSource for LibraryController {
        #[unsafe(method(tableView:numberOfRowsInSection:))]
        fn tableView_numberOfRowsInSection(
            &self,
            _table_view: &UITableView,
            section: NSInteger,
        ) -> NSInteger {
            let sections = unsafe { self.ivars().fetched_movies.sections().unwrap() };
            let section_info = sections.objectAtIndex(section as usize);
            unsafe { section_info.numberOfObjects() as isize }
        }

        #[unsafe(method(numberOfSectionsInTableView:))]
        fn numberOfSectionsInTableView(&self, _table_view: &UITableView) -> NSInteger {
            unsafe { self.ivars().fetched_movies.sections().unwrap().count() as NSInteger }
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
            Some(NSString::from_str("Library"))
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
    fn logo_view(&self) -> &PlayerView {
        self.ivars().logo_view.get().expect("logo view initialized")
    }

    fn view_did_load(&self) {
        tracing::info!("library viewDidLoad");

        self.montar_barra_de_baixo();
        self.setup_logo();

        unsafe {
            self.ivars()
                .fetched_movies
                .performFetch()
                .expect("failed fetching movies")
        };
    }

    /// A BARRA DE BAIXO: ENDERECO E LICENCAS.
    ///
    /// Embaixo, e nao em cima, porque a barra de cima ja vem montada pelo
    /// storyboard com o "+" e o "Editar". Mexer nela seria adivinhar o que ja
    /// esta la; a de baixo esta vazia e nao briga com ninguem.
    fn montar_barra_de_baixo(&self) {
        let mtm = MainThreadMarker::from(self);

        let botao = |titulo: &str, acao| -> Retained<UIBarButtonItem> {
            let texto = NSString::from_str(titulo);
            unsafe {
                msg_send![
                    mtm.alloc::<UIBarButtonItem>(),
                    initWithTitle: &*texto,
                    style: 0isize,          // UIBarButtonItemStylePlain
                    target: self,
                    action: acao,
                ]
            }
        };

        // O espaco flexivel entre os dois empurra um pra cada ponta.
        let espaco: Retained<UIBarButtonItem> = unsafe {
            msg_send![
                mtm.alloc::<UIBarButtonItem>(),
                initWithBarButtonSystemItem: 5isize,   // FlexibleSpace
                target: Option::<&AnyObject>::None,
                action: Option::<objc2::runtime::Sel>::None,
            ]
        };

        let endereco = botao("Abrir endereço", sel!(abrirEndereco:));
        let licencas = botao("Licenças", sel!(mostrarLicencas:));
        let itens = NSArray::from_retained_slice(&[endereco, espaco, licencas]);

        unsafe {
            let _: () = msg_send![self, setToolbarItems: &*itens, animated: false];
            if let Some(nav) = self.navigationController() {
                let _: () = msg_send![&*nav, setToolbarHidden: false, animated: false];
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
        let view = self.logo_view();
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

        self.logo_view().start();
    }

    fn view_will_disappear(&self, _animated: bool) {
        tracing::info!("library viewWillDisappear:");

        self.logo_view().stop();
    }

    fn view_did_disappear(&self, _animated: bool) {
        tracing::info!("library viewDidDisappear:");

        self.logo_view().player_lock().flush_shared_objects();
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
            ns_string!("Done")
        } else {
            ns_string!("Edit")
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
                subtitle.setText(Some(ns_string!("Unknown")));
            }
        } else {
            subtitle.setText(Some(&url.absoluteString().unwrap()));
        }
    }
}
