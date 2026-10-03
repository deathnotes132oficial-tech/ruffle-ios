//! O CONTADOR DO ALOCADOR: QUEM REALMENTE SABE ONDE ESTA A MEMORIA.
//!
//! POR QUE ISTO EXISTE
//! -------------------
//! Os contadores que ja havia medem o que a biblioteca do Ruffle GUARDA:
//! texturas, malhas, bytes de SWF, bytes de imagem. Numa sessao que usava
//! 1807 MB, tudo isso somado dava 75 MB — quatro por cento. Os outros noventa
//! e seis por cento estavam fora do alcance de qualquer contador nosso.
//!
//! O coletor do ActionScript tambem nao ajuda: a versao de gc-arena presa
//! aqui conta OBJETOS e nao bytes, e quatro milhoes de objetos podem ser
//! duzentos megabytes ou dois gigabytes.
//!
//! Entao a medicao desce um andar. Todo pedaco de memoria que o Rust pede ao
//! sistema passa por aqui: o monte do coletor, os bytes dos SWFs baixados,
//! as imagens decodificadas, o som, os registros de carregamento. Nada
//! escapa, porque nao ha outro caminho.
//!
//! O QUE A FAIXA DE TAMANHO RESPONDE
//! ---------------------------------
//! Saber que ha 1,4 GB vivos ainda nao diz onde mexer. Saber COMO ele esta
//! repartido, diz:
//!
//!   - milhoes de pedacos pequenos  -> sao objetos do ActionScript, e o
//!     caminho e fazer o jogo criar menos ou o coletor recolher mais;
//!   - poucos pedacos enormes       -> sao arquivos e imagens inteiras
//!     seguradas por alguem, e o caminho e descobrir quem segura.
//!
//! Sao dois consertos completamente diferentes. Sem esta medida, a escolha
//! entre eles e chute.
//!
//! O CUSTO
//! -------
//! Tres somas atomicas por alocacao, sem trava e sem tabela ao lado: o
//! tamanho de volta vem no proprio `Layout` que o Rust entrega na devolucao.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Os limites das faixas, em bytes. A ultima e "tudo acima".
const LIMITES: [usize; 7] = [
    64,
    256,
    1024,
    16 * 1024,
    256 * 1024,
    4 * 1024 * 1024,
    usize::MAX,
];

const FAIXAS: usize = LIMITES.len();

static VIVO: AtomicUsize = AtomicUsize::new(0);
static PICO: AtomicUsize = AtomicUsize::new(0);
static BLOCOS: AtomicUsize = AtomicUsize::new(0);

/// Quantos pedacos vivos ha em cada faixa, e quantos bytes eles somam.
static POR_FAIXA: [AtomicUsize; FAIXAS] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0),
];
static BYTES_POR_FAIXA: [AtomicUsize; FAIXAS] = [
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0), AtomicUsize::new(0), AtomicUsize::new(0),
    AtomicUsize::new(0),
];

fn faixa(tamanho: usize) -> usize {
    let mut i = 0;
    while i < FAIXAS - 1 && tamanho > LIMITES[i] {
        i += 1;
    }
    i
}

fn somar(tamanho: usize) {
    let antes = VIVO.fetch_add(tamanho, Ordering::Relaxed);
    PICO.fetch_max(antes + tamanho, Ordering::Relaxed);
    BLOCOS.fetch_add(1, Ordering::Relaxed);
    let f = faixa(tamanho);
    POR_FAIXA[f].fetch_add(1, Ordering::Relaxed);
    BYTES_POR_FAIXA[f].fetch_add(tamanho, Ordering::Relaxed);
}

fn tirar(tamanho: usize) {
    VIVO.fetch_sub(tamanho, Ordering::Relaxed);
    BLOCOS.fetch_sub(1, Ordering::Relaxed);
    let f = faixa(tamanho);
    POR_FAIXA[f].fetch_sub(1, Ordering::Relaxed);
    BYTES_POR_FAIXA[f].fetch_sub(tamanho, Ordering::Relaxed);
}

pub struct Contado;

unsafe impl GlobalAlloc for Contado {
    unsafe fn alloc(&self, esquema: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(esquema) };
        if !p.is_null() {
            somar(esquema.size());
        }
        p
    }

    unsafe fn dealloc(&self, ponteiro: *mut u8, esquema: Layout) {
        tirar(esquema.size());
        unsafe { System.dealloc(ponteiro, esquema) };
    }

    unsafe fn alloc_zeroed(&self, esquema: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(esquema) };
        if !p.is_null() {
            somar(esquema.size());
        }
        p
    }

    unsafe fn realloc(&self, ponteiro: *mut u8, esquema: Layout, novo: usize) -> *mut u8 {
        let p = unsafe { System.realloc(ponteiro, esquema, novo) };
        if !p.is_null() {
            // So se contabiliza quando deu certo: fracassando, o pedaco antigo
            // continua vivo e inteiro.
            tirar(esquema.size());
            somar(novo);
        }
        p
    }
}

fn mb(bytes: usize) -> usize {
    bytes / (1024 * 1024)
}

/// Quantos bytes o Rust tem vivos agora. Serve pra separar o que e nosso do
/// que e do sistema: a conta do iOS inclui tudo, inclusive o que o Objective-C
/// e a placa de video seguram.
#[allow(dead_code)]
pub fn vivo_mb() -> usize {
    mb(VIVO.load(Ordering::Relaxed))
}

/// A linha inteira, pro registro.
pub fn resumo() -> String {
    let vivo = VIVO.load(Ordering::Relaxed);
    let pico = PICO.load(Ordering::Relaxed);
    let blocos = BLOCOS.load(Ordering::Relaxed);

    let nomes = ["64B", "256B", "1K", "16K", "256K", "4M", "+4M"];
    let mut faixas = String::new();
    for i in 0..FAIXAS {
        let quantos = POR_FAIXA[i].load(Ordering::Relaxed);
        if quantos == 0 {
            continue;
        }
        let bytes = BYTES_POR_FAIXA[i].load(Ordering::Relaxed);
        // Pedacos sao contados em milhares: quatro milhoes escritos por
        // extenso tornam a linha ilegivel.
        faixas.push_str(&format!(" {}:{}k/{}MB", nomes[i], quantos / 1000, mb(bytes)));
    }

    format!(
        "rust {} MB pico {} MB blocos {}k |{}",
        mb(vivo),
        mb(pico),
        blocos / 1000,
        faixas
    )
}
