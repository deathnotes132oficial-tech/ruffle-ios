//! Interface for storing user data with CoreData.
//!
//! Overview:
//! - Movie
//!   - link
//!   - userOptions
//!   - movieData
//!     - key/value
//!
//! External methods will be filled in dynamically by CoreData.
//!
//! TODO:
//! - Figure out sync failures.
//! - Better error handling.
//! - Use `define_class!` once we can create ivars with specific names in
//!   that (required for NSManagedObject to work).
//!
//! To generate data model interface to compare with, modify .xcdatamodeld and
//! set codegen = Class definition on every entity. Then run:
//! /Applications/Xcode.app/Contents/Developer/usr/bin/momc --action generate ./Ruffle.xcdatamodeld storage
#![allow(non_snake_case)]

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;
use std::sync::OnceLock;

use block2::RcBlock;
use objc2::encode::{Encoding, RefEncode};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, ClassBuilder};
use objc2::{extern_conformance, extern_methods, msg_send, AllocAnyThread, ClassType, Message};
use objc2_core_data::{
    NSFetchRequest, NSFetchRequestResult, NSFetchedResultsController, NSManagedObject,
    NSManagedObjectContext, NSPersistentContainer, NSPersistentStoreDescription,
};
use objc2_foundation::{
    ns_string, NSArray, NSData, NSError, NSObject, NSObjectProtocol, NSSet, NSSortDescriptor,
    NSString, NSURL,
};
use ruffle_core::backend::storage::StorageBackend;
use ruffle_frontend_utils::bundle::source::BundleSourceError;
use ruffle_frontend_utils::bundle::{Bundle, BundleError};
use ruffle_frontend_utils::content::{ContentDescriptor, PlayingContent};
use ruffle_frontend_utils::player_options::PlayerOptions;
use url::Url;

/// The data relevant for an SWF movie / a Ruffle Bundle.
#[repr(transparent)]
#[derive(Debug)]
pub struct Movie {
    superclass: NSManagedObject,
}

unsafe impl RefEncode for Movie {
    const ENCODING_REF: Encoding = NSManagedObject::ENCODING_REF;
}

unsafe impl Message for Movie {}

impl Deref for Movie {
    type Target = NSManagedObject;

    fn deref(&self) -> &Self::Target {
        &self.superclass
    }
}

extern_conformance!(
    // HACK
    unsafe impl NSObjectProtocol for Movie {}
);

extern_conformance!(
    // HACK
    unsafe impl NSFetchRequestResult for Movie {}
);

impl Movie {
    pub fn class() -> &'static AnyClass {
        static CLS: OnceLock<&'static AnyClass> = OnceLock::new();

        CLS.get_or_init(|| {
            let mut builder = ClassBuilder::new(c"Movie", NSManagedObject::class()).unwrap();

            // FIXME: Deallocation of these in `dealloc`.
            builder.add_ivar::<*mut NSURL>(c"link");
            builder.add_ivar::<*mut NSString>(c"cachedName");
            builder.add_ivar::<*mut NSData>(c"userOptions");
            builder.add_ivar::<*mut NSSet<MovieData>>(c"movieData");

            builder.register()
        })
    }

    // NSManagedObject initializers.

    fn initWithContext(this: Allocated<Self>, moc: &NSManagedObjectContext) -> Retained<Self> {
        unsafe { msg_send![this, initWithContext: moc] }
    }

    fn fetchRequest() -> Retained<NSFetchRequest<Self>> {
        unsafe { msg_send![Self::class(), fetchRequest] }
    }

    // Properties
    extern_methods!(
        /// Reference/bookmark to a Ruffle Bundle or SWF.
        /// - Either a bookmarked link to the actual bundle/SWF stored on user's device.
        /// - Or http/https link to externally stored bundle/SWF.
        ///
        /// TODO: Store bookmark data internally.
        #[unsafe(method(link))]
        pub fn link(&self) -> Retained<NSURL>;

        #[unsafe(method(setLink:))]
        pub fn setLink(&self, value: &NSURL);

        /// A cached value of the name of the bundle/SWF. Allows us to avoid
        /// reading the link when displaying the list of movies.
        #[unsafe(method(cachedName))]
        pub fn cachedName(&self) -> Retained<NSString>;

        #[unsafe(method(setCachedName:))]
        pub fn setCachedName(&self, value: &NSString);

        /// Any user-specified settings (overrides the Ruffle Bundle's preconfigured settings).
        #[unsafe(method(userOptions))]
        fn _userOptions(&self) -> Retained<NSData>;

        #[unsafe(method(setUserOptions:))]
        fn _setUserOptions(&self, value: &NSData);

        /// Data the SWF itself may have stored (the `.sol` key-value store).
        #[unsafe(method(movieData))]
        pub fn movieData(&self) -> Retained<NSSet<MovieData>>;

        #[unsafe(method(setMovieData:))]
        pub fn setMovieData(&self, values: &NSSet<MovieData>);
    );

    pub fn user_options(&self) -> PlayerOptions {
        // TODO: Convert from binary data in _userOptions.
        // Maybe using serde?
        PlayerOptions::default()
    }

    pub fn set_user_options(&self, _options: &PlayerOptions) {
        // TODO: Convert to binary data.
        self._setUserOptions(&NSData::with_bytes(b"{}"));
    }

    // Perhaps: `cachedName`, to allow easily finding relevant settings for an SWF in case the user deleted?

    // Generated accessors
    extern_methods!(
        #[unsafe(method(addMovieDataObject:))]
        pub fn addMovieDataObject(&self, value: &MovieData);

        #[unsafe(method(removeMovieDataObject:))]
        pub fn removeMovieDataObject(&self, value: &MovieData);

        #[unsafe(method(addMovieData:))]
        pub fn addMovieData(&self, values: &NSSet<MovieData>);

        #[unsafe(method(removeMovieData:))]
        pub fn removeMovieData(&self, values: &NSSet<MovieData>);
    );
}

/// Key/value pairs of data that the movie itself wants to store (.sol).
///
/// Intended invariant: Keys are unique.
#[repr(transparent)]
#[derive(Debug)]
pub struct MovieData {
    superclass: NSManagedObject,
}

unsafe impl RefEncode for MovieData {
    const ENCODING_REF: Encoding = NSManagedObject::ENCODING_REF;
}

unsafe impl Message for MovieData {}

impl Deref for MovieData {
    type Target = NSManagedObject;

    fn deref(&self) -> &Self::Target {
        &self.superclass
    }
}

impl MovieData {
    pub fn class() -> &'static AnyClass {
        static CLS: OnceLock<&'static AnyClass> = OnceLock::new();

        CLS.get_or_init(|| {
            let mut builder = ClassBuilder::new(c"MovieData", NSManagedObject::class()).unwrap();

            // FIXME: Deallocation of these in `dealloc`.
            builder.add_ivar::<*mut NSString>(c"key");
            builder.add_ivar::<*mut NSData>(c"value");

            builder.register()
        })
    }

    // NSManagedObject initializers.

    fn initWithContext(this: Allocated<Self>, moc: &NSManagedObjectContext) -> Retained<Self> {
        unsafe { msg_send![this, initWithContext: moc] }
    }

    // Properties
    extern_methods!(
        #[unsafe(method(key))]
        pub fn key(&self) -> Retained<NSString>;

        #[unsafe(method(setKey:))]
        pub fn setKey(&self, value: &NSString);

        #[unsafe(method(value))]
        pub fn value(&self) -> Retained<NSData>;

        #[unsafe(method(setValue:))]
        pub fn setValue(&self, value: &NSData);

        #[unsafe(method(movie))]
        pub fn movie(&self) -> Retained<Movie>;

        #[unsafe(method(setMovie:))]
        pub fn setMovie(&self, value: &Movie);
    );
}

#[derive(Debug, Clone)]
pub struct MovieStorageBackend {
    pub movie: Retained<Movie>,
}

impl MovieStorageBackend {
    fn lookup_data(&self, key: &NSString) -> Option<Retained<MovieData>> {
        // TODO: Do this lookup on the CoreData model directly?
        // Maybe using NSPredicate?
        self.movie
            .movieData()
            .iter()
            .find(|data| &*data.key() == key)
    }
}

impl StorageBackend for MovieStorageBackend {
    fn get(&self, name: &str) -> Option<Vec<u8>> {
        let key = NSString::from_str(name);
        let data = self.lookup_data(&key)?;
        Some(data.value().to_vec())
    }

    fn put(&mut self, name: &str, value: &[u8]) -> bool {
        let key = NSString::from_str(name);
        let value = NSData::with_bytes(value);
        if let Some(existing) = self.lookup_data(&key) {
            existing.setValue(&value);
        } else {
            let data = unsafe { msg_send![MovieData::class(), alloc] };
            let data = MovieData::initWithContext(data, unsafe { &container().viewContext() });
            data.setKey(&key);
            data.setValue(&value);
            self.movie.addMovieDataObject(&data);
        }

        // Flush changes to disk.
        match unsafe { container().viewContext().save() } {
            Ok(()) => true,
            Err(err) => {
                eprintln!("failed saving key {name:?}: {err}");
                false
            }
        }
    }

    fn remove_key(&mut self, name: &str) {
        let key = NSString::from_str(name);
        if let Some(existing) = self.lookup_data(&key) {
            unsafe { container().viewContext().deleteObject(&existing) };
        }

        // Flush changes to disk.
        unsafe { container().viewContext().save() }.unwrap_or_else(|err| {
            eprintln!("failed removing key {name:?}: {err}");
        })
    }
}

static PERSISTENT: OnceLock<Retained<NSPersistentContainer>> = OnceLock::new();

pub fn setup() {
    let persistent = PERSISTENT.get_or_init(|| unsafe {
        NSPersistentContainer::persistentContainerWithName(ns_string!("Ruffle"))
    });

    let block = RcBlock::new(
        |descriptor: NonNull<NSPersistentStoreDescription>, err: *mut NSError| {
            if let Some(err) = unsafe { err.as_ref() } {
                panic!("failed loading: {err}");
            }
            let descriptor = unsafe { descriptor.as_ref() };
            tracing::info!("loading {descriptor:?}");
        },
    );
    unsafe { persistent.loadPersistentStoresWithCompletionHandler(&block) };

    tracing::info!("finished storage setup");
}

fn container() -> &'static NSPersistentContainer {
    PERSISTENT
        .get()
        .expect("NSPersistentContainer must be initialized")
}

pub struct SecurityScopedResource {
    url: Retained<NSURL>,
}

impl SecurityScopedResource {
    pub fn access(url: &NSURL) -> Option<Self> {
        if unsafe { url.startAccessingSecurityScopedResource() } {
            Some(Self { url: url.retain() })
        } else {
            None
        }
    }
}

impl Drop for SecurityScopedResource {
    fn drop(&mut self) {
        unsafe { self.url.stopAccessingSecurityScopedResource() };
    }
}

fn url_to_path(url: &NSURL) -> PathBuf {
    // TODO: Use fileSystemRepresentation?
    let path = url.filePathURL().unwrap().path().unwrap();
    PathBuf::from(path.to_string())
}

pub fn get_playing_content(url: &NSURL) -> PlayingContent {
    if !url.isFileURL() {
        let s = url.absoluteString().unwrap().to_string();
        let url = Url::parse(&s).unwrap();
        return PlayingContent::DirectFile(ContentDescriptor {
            url,
            root_content_path: None, // TODO
        });
    }

    // FALTA DE ESCOPO DE SEGURANCA NAO E ERRO FATAL.
    //
    // O QUE ESTAVA AQUI
    // -----------------
    //     let _access = SecurityScopedResource::access(url)
    //         .unwrap_or_else(|| panic!("failed accessing NSURL: {url:?}"));
    //
    // O startAccessingSecurityScopedResource so devolve verdadeiro para
    // endereco que VEIO do seletor de arquivos do sistema. Para um arquivo
    // que o proprio aplicativo escreveu na pasta dele — o Exemplo.swf — ele
    // devolve FALSO, porque nao ha escopo nenhum a abrir: o arquivo ja e
    // nosso e sempre foi legivel.
    //
    // Com o panic!, portanto, o aplicativo se derrubava justamente no
    // caminho que SO acontece na PRIMEIRA abertura depois de instalado (o
    // garantir_exemplo chama aqui uma vez; depois o arquivo ja existe e ele
    // nem entra). Quem instala e abre uma vez — um revisor da App Store —
    // via o aplicativo sumir; quem abria de novo via tudo funcionar.
    //
    // Sem escopo, segue em frente. Se o arquivo realmente nao puder ser
    // lido, quem reclama e a leitura, com erro tratado.
    let _access = SecurityScopedResource::access(url);

    match Bundle::from_path(url_to_path(&url)) {
        Ok(bundle) => {
            if bundle.warnings().is_empty() {
                tracing::info!("opening bundle at {url:?}");
            } else {
                // TODO: Show warnings to user (toast?)
                tracing::warn!("opening bundle at {url:?} with warnings");
                for warning in bundle.warnings() {
                    tracing::warn!("{warning}");
                }
            }

            let s = url
                .filePathURL()
                .unwrap()
                .absoluteString()
                .unwrap()
                .to_string();
            let url = Url::parse(&s).unwrap();
            PlayingContent::Bundle(
                ContentDescriptor {
                    url,
                    root_content_path: None, // TODO
                },
                Box::new(bundle),
            )
        }
        Err(BundleError::BundleDoesntExist)
        | Err(BundleError::InvalidSource(BundleSourceError::UnknownSource)) => {
            // Open it as a swf - this likely isn't a bundle at all
            let s = url
                .filePathURL()
                .unwrap()
                .absoluteString()
                .unwrap()
                .to_string();
            let url = Url::parse(&s).unwrap();
            PlayingContent::DirectFile(ContentDescriptor {
                url,
                root_content_path: None, // TODO
            })
        }
        Err(e) => panic!("failed opening bundle {url:?}: {e}"),
    }
}

/// The returned movie should only be relied upon in `scene_delegate::play_url`.
pub fn movie_from_url(url: &NSURL) -> Option<Retained<Movie>> {
    // Mesmo motivo do add_movie: endereco por referencia pode vir vazio, e
    // isso nao pode derrubar o aplicativo. Esta funcao e a PRIMEIRA chamada
    // com o Exemplo.swf recem-criado, entao o unwrap daqui caia antes de
    // todos os outros.
    let file_url = url.fileReferenceURL();
    let url = match (url.isFileURL(), file_url.as_deref()) {
        (true, Some(referencia)) => referencia,
        _ => url,
    };

    let request: Retained<NSFetchRequest> = unsafe { msg_send![Movie::class(), fetchRequest] };
    let movies = unsafe {
        container()
            .viewContext()
            .executeFetchRequest_error(&request)
    }
    .unwrap_or_else(|err| panic!("failed loading movies: {err}"));
    for movie in movies {
        let movie = movie.downcast::<NSObject>().unwrap();
        assert!(movie.isKindOfClass(Movie::class()));
        let movie = unsafe { Retained::cast_unchecked::<Movie>(movie) };
        if &*movie.link() == url {
            return Some(movie);
        }
    }
    None
}

/// NAO DERRUBAR O APLICATIVO POR CAUSA DE UM ENDERECO.
///
/// O QUE ESTAVA AQUI
/// -----------------
///     let url = if url.isFileURL() { file_url.as_deref().unwrap() } else { url };
///
/// O fileReferenceURL devolve vazio quando o arquivo nao existe mais, quando
/// esta num lugar que nao aceita esse tipo de endereco, ou simplesmente
/// quando o sistema decide nao dar — e no iOS ele e uma forma antiga, que a
/// Apple desencoraja. Nesses casos o unwrap derruba o programa INTEIRO.
///
/// POR QUE ISSO IMPORTA MAIS DO QUE PARECE
/// --------------------------------------
/// Esta funcao e chamada quando a biblioteca ganha um arquivo. Na PRIMEIRA
/// abertura depois de instalado, o aplicativo cria o Exemplo.swf e chama
/// aqui; da segunda em diante ele ve que o arquivo ja existe e nem entra.
/// Ou seja: se cair aqui, cai so na primeira vez que a pessoa abre — que e
/// exatamente o que um revisor da App Store faz, uma vez, e desiste.
///
/// Guardar o endereco comum em vez de derrubar nao perde nada: ele continua
/// apontando pro mesmo arquivo. O endereco por referencia e preferido so
/// porque sobrevive ao arquivo ser renomeado; sem ele, o pior caso e a
/// entrada parar de achar o arquivo se alguem renomear por fora.
pub fn add_movie(url: &NSURL) {
    // The canonical URL in our DB is a file reference URL.
    let file_url = url.fileReferenceURL();
    let url = match (url.isFileURL(), file_url.as_deref()) {
        (true, Some(referencia)) => referencia,
        _ => url,
    };

    let content = get_playing_content(url);

    let movie = unsafe { msg_send![Movie::class(), alloc] };
    let movie = Movie::initWithContext(movie, unsafe { &container().viewContext() });
    movie.setLink(&url);
    let name = match content {
        PlayingContent::Bundle(_, bundle) => NSString::from_str(&bundle.information().name),
        PlayingContent::DirectFile(desc) => {
            // Try to figure out a reasonable name for the URL.
            if let Some(file_stem) = Path::new(desc.url.path()).file_stem() {
                NSString::from_str(&file_stem.to_string_lossy())
            } else {
                NSString::from_str(&desc.url.host_str().unwrap_or("unknown"))
            }
        }
    };
    movie.setCachedName(&name);
    movie.set_user_options(&PlayerOptions::default());

    // Flush changes to disk.
    unsafe { container().viewContext().save() }.unwrap_or_else(|err| {
        eprintln!("failed adding movie {url:?}: {err}");
    })
}

pub fn delete_movie(movie: &Movie) {
    unsafe { container().viewContext().deleteObject(movie) };

    // Flush changes to disk.
    unsafe { container().viewContext().save() }.unwrap_or_else(|err| {
        eprintln!("failed removing movie {:?}: {err}", movie.link());
    })
}

pub fn all_movies() -> Retained<NSFetchedResultsController<Movie>> {
    let fetch_request = Movie::fetchRequest();

    let cached_name_descriptor =
        NSSortDescriptor::sortDescriptorWithKey_ascending(Some(ns_string!("cachedName")), true);
    let link_descriptor = NSSortDescriptor::sortDescriptorWithKey_ascending(
        Some(ns_string!("link.lastPathComponent")),
        true,
    );
    let sort_descriptors = NSArray::from_retained_slice(&[cached_name_descriptor, link_descriptor]);
    unsafe { fetch_request.setSortDescriptors(Some(&sort_descriptors)) };

    unsafe {
        NSFetchedResultsController::initWithFetchRequest_managedObjectContext_sectionNameKeyPath_cacheName(
            NSFetchedResultsController::alloc(),
            &fetch_request,
            &container().viewContext(),
            None, // No sectioning
            None, // No cache
        )
    }
}
