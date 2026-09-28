//! O REGISTRO DO QUE ACONTECEU.
//!
//! POR QUE A PRIMEIRA VERSAO NAO BASTOU
//! ------------------------------------
//! A primeira versao so sabia contar UM tipo de morte: quando o Ruffle quebra
//! e avisa antes. O aplicativo fechou no iPhone do testador e nada foi
//! escrito — ou seja, ele esta morrendo por um caminho que nao passa por ali:
//! falta de memoria no meio de uma alocacao, erro no desenho pelo Metal, ou o
//! proprio sistema encerrando o processo. Nenhum desses avisa ninguem.
//!
//! COMO ESTA VERSAO PEGA TODOS
//! ---------------------------
//! Em vez de esperar um aviso, ela trabalha por AUSENCIA:
//!
//!   - ao abrir, o registro da sessao passada e guardado de lado e comeca um
//!     novo;
//!   - enquanto roda, o aplicativo vai anotando por onde passou;
//!   - quando vai pro fundo — que e como toda saida normal comeca, inclusive
//!     fechar pelo seletor de aplicativos — ele anota isso.
//!
//! Na abertura seguinte, se a ultima linha da sessao passada NAO for a ida
//! pro fundo, o aplicativo morreu no meio. Nao importa de que jeito: morreu,
//! e tudo que ele tinha anotado ate ali esta guardado.
//!
//! A MEMORIA, MEDIDA NO APARELHO
//! -----------------------------
//! O simulador nao serve pra essa pergunta: ele roda num Mac, com memoria de
//! sobra. Quem responde e a propria Apple, pela os_proc_available_memory —
//! que diz quanto AINDA RESTA pro aplicativo antes de o sistema mata-lo.
//! Anotada de dois em dois segundos durante o jogo, ela mostra no registro se
//! a morte veio precedida de queda livre, ou se chegou do nada.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// A marca de quebra do Ruffle. Continua existindo: quando ela aparece, o
/// motivo vem escrito, e isso vale mais que qualquer deducao.
pub const MARCA_DE_QUEBRA: &str = "QUEBROU";

/// A marca de saida limpa. E a ULTIMA coisa escrita numa sessao que terminou
/// bem. Se ela nao for a ultima linha, houve morte.
const MARCA_DE_FUNDO: &str = "foi pro fundo";

/// Teto do arquivo. Uma medida de memoria ocupa umas quatro dezenas de bytes,
/// entao isto da varias horas de jogo. Estourando, o registro recomeca — o
/// que interessa numa investigacao e sempre o fim dele.
const TETO: u64 = 256 * 1024;

fn pasta() -> Option<PathBuf> {
    let casa = std::env::var("HOME").ok()?;
    let alvo = PathBuf::from(casa).join("Documents");
    fs::create_dir_all(&alvo).ok()?;
    Some(alvo)
}

/// A sessao de agora.
fn caminho_atual() -> Option<PathBuf> {
    Some(pasta()?.join("registro.txt"))
}

/// A sessao passada, guardada na abertura.
fn caminho_anterior() -> Option<PathBuf> {
    Some(pasta()?.join("registro-anterior.txt"))
}

/// Escreve uma linha. Nunca falha de forma barulhenta: registro que derruba o
/// aplicativo seria pior que registro nenhum.
///
/// Escreve SEM guardar nada em memoria de passagem. Isso importa: o processo
/// pode ser morto a qualquer instante, e o que nao tiver saido ja estaria
/// perdido — justamente nas ultimas linhas, que sao as que interessam.
/// O fuso de Brasilia, em segundos.
///
/// O registro e lido por gente daqui, e hora em UTC obrigaria a fazer a conta
/// de cabeca justamente na hora em que se esta tentando entender uma morte. O
/// Brasil nao tem mais horario de verao, entao um numero fixo basta.
const FUSO: i64 = -3 * 3600;

/// Quando esta sessao abriu, em segundos desde 1970. Zero antes de abrir.
static INICIO_DA_SESSAO: AtomicU64 = AtomicU64::new(0);

/// Quebra os segundos desde 1970 em data e hora daqui.
///
/// A conta de dias pra ano/mes/dia e a do Howard Hinnant, a mesma que as
/// bibliotecas de data usam por dentro. Vale pra qualquer data; nao ha caso
/// especial de ano bissexto pra lembrar.
fn relogio(epoca: u64) -> (i64, i64, i64, u64, u64, u64) {
    let local = epoca as i64 + FUSO;
    let dias = local.div_euclid(86400);
    let hora = local.rem_euclid(86400) as u64;

    let z = dias + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + if mes <= 2 { 1 } else { 0 };

    (ano, mes, dia, hora / 3600, (hora % 3600) / 60, hora % 60)
}

// A MEMORIA DO LADO DO METAL.
//
// Tudo que medimos ate aqui conta o que o Ruffle ACHA que tem. Se a base
// continuar em 4,8 GB com a contagem de malhas baixa, isso so tira o Ruffle da
// lista de suspeitos — nao diz pra onde a memoria foi, e descobrir custaria
// outra compilacao de quem testa.
//
// O Metal responde direto. currentAllocatedSize e todo byte que a GPU alocou:
// texturas, vertices, alvos de desenho, tenha pedido quem tiver pedido. Com
// esse numero ao lado do "usado", a divisao deixa de ser deducao:
//
//   usado alto e metal alto  -> e grafico, seguimos em malha e textura
//   usado alto e metal baixo -> e do lado da CPU (bytes de SWF, objetos do
//                               ActionScript, ou triangulacao guardada la)
//
// RESSALVA HONESTA: o aparelho tem uma GPU so, e pedir o dispositivo padrao
// devolve o mesmo que o Ruffle usa. Se por algum motivo devolver outro, este
// numero vem perto de zero — e ai o proprio zero avisa que a medida nao serve,
// em vez de mentir um valor plausivel.
#[link(name = "Metal", kind = "framework")]
unsafe extern "C" {
    fn MTLCreateSystemDefaultDevice() -> *mut objc2::runtime::AnyObject;
}

/// O dispositivo grafico, pedido uma vez e guardado.
fn dispositivo_do_metal() -> Option<&'static objc2::runtime::AnyObject> {
    static DISPOSITIVO: OnceLock<usize> = OnceLock::new();
    let ponteiro = *DISPOSITIVO.get_or_init(|| unsafe { MTLCreateSystemDefaultDevice() as usize });
    if ponteiro == 0 {
        return None;
    }
    // Vive pelo resto do programa: foi criado uma vez e nunca e devolvido.
    Some(unsafe { &*(ponteiro as *const objc2::runtime::AnyObject) })
}

/// Quantos MB a GPU tem alocados agora.
pub fn memoria_do_metal_mb() -> Option<u64> {
    let dispositivo = dispositivo_do_metal()?;
    let bytes: usize = unsafe { objc2::msg_send![dispositivo, currentAllocatedSize] };
    Some((bytes / (1024 * 1024)) as u64)
}

/// Quanto tempo o aplicativo esta de pe, escrito pra ler.
fn tempo_ligado(agora: u64) -> String {
    let inicio = INICIO_DA_SESSAO.load(Ordering::Relaxed);
    if inicio == 0 || agora < inicio {
        return String::new();
    }
    let total = agora - inicio;
    format!(" | ligado {}m{:02}s", total / 60, total % 60)
}

pub fn anotar(texto: &str) {
    let Some(alvo) = caminho_atual() else { return };

    let mut recomecou = false;
    if let Ok(dados) = fs::metadata(&alvo) {
        if dados.len() > TETO {
            let _ = fs::remove_file(&alvo);
            recomecou = true;
        }
    }

    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let (_, _, _, h, m, s) = relogio(segundos);

    if let Ok(mut arquivo) = OpenOptions::new().create(true).append(true).open(&alvo) {
        if recomecou {
            let _ = writeln!(arquivo, "[{h:02}:{m:02}:{s:02}] (registro recomecado por tamanho)");
        }
        let _ = writeln!(arquivo, "[{h:02}:{m:02}:{s:02}] {texto}");
    }
}

/// Abre uma sessao nova e guarda a anterior de lado.
///
/// Chamada UMA vez, no comeco de tudo. Instala o gancho de quebra antes de
/// qualquer outra coisa acontecer, porque quebra que ocorresse antes disso
/// passaria despercebida.
pub fn iniciar_sessao() {
    if let (Some(atual), Some(anterior)) = (caminho_atual(), caminho_anterior()) {
        if atual.exists() {
            let _ = fs::remove_file(&anterior);
            let _ = fs::rename(&atual, &anterior);
        }
    }

    instalar_gancho();

    let agora = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    INICIO_DA_SESSAO.store(agora, Ordering::Relaxed);

    let (ano, mes, dia, h, m, s) = relogio(agora);
    anotar(&format!(
        "sessao aberta em {dia:02}/{mes:02}/{ano} as {h:02}:{m:02}:{s:02}"
    ));
}

/// Passa a anotar toda quebra do programa.
///
/// O gancho anterior continua valendo — e ele quem escreve no console, e nas
/// provas do simulador e por ele que a mensagem aparece. Aqui so acrescentamos
/// a copia em disco, que e a que sobrevive ao aplicativo fechar.
fn instalar_gancho() {
    let anterior = panic::take_hook();
    panic::set_hook(Box::new(move |aviso| {
        let onde = aviso
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "lugar desconhecido".to_string());

        // A mensagem vem como &str ou como String, conforme quem quebrou.
        let mensagem = aviso
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| aviso.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "sem mensagem".to_string());

        anotar(&format!("{MARCA_DE_QUEBRA} em {onde}: {mensagem}"));
        anterior(aviso);
    }));
}

pub fn marcar_fundo() {
    anotar(MARCA_DE_FUNDO);
}

pub fn marcar_frente() {
    anotar("em primeiro plano");
}

// ------------------------------------------------------------ A MEMORIA
//
// os_proc_available_memory devolve, em bytes, quanto o aplicativo ainda pode
// crescer antes de o sistema encerra-lo. Existe no iPhone desde o iOS 13 e
// mora na biblioteca do proprio sistema, que ja vem ligada.
//
// NO SIMULADOR ELA DEVOLVE ZERO. Isso nao e erro: la nao ha limite pra medir.
// Por isso o zero vira "indisponivel" em vez de "acabou a memoria" — trocar
// os dois faria o registro mentir justamente na prova mais barata.
extern "C" {
    fn os_proc_available_memory() -> usize;
}

pub fn memoria_livre_mb() -> Option<u64> {
    let bytes = unsafe { os_proc_available_memory() };
    if bytes == 0 {
        None
    } else {
        Some((bytes / (1024 * 1024)) as u64)
    }
}

// O QUE A MEDIDA ANTERIOR NAO RESPONDIA
// -------------------------------------
// Ate aqui o registro dizia so quanto SOBRAVA. Com isso nao da pra separar
// duas mortes diferentes:
//
//   - o aplicativo cresceu e bateu no proprio teto;
//   - o aparelho inteiro ficou apertado e o iOS escolheu matar o maior
//     consumidor, mesmo com o teto dele ainda longe.
//
// A ultima sessao morreu com 1723 MB de folga, o que torna a segunda hipotese
// possivel — e as duas pedem consertos opostos. Entao agora cada linha traz:
//
//   USADO    quanto o aplicativo cresceu desde que abriu (folga inicial menos
//            a de agora). Nao precisa de conta do sistema: e subtracao.
//   MINIMO   a menor folga ja vista. Morrer logo depois de um minimo baixo e
//            outra historia de morrer com folga de sobra.
//   SWFS     os contadores do Ruffle, os mesmos que a caixa mostra no
//            navegador, pra confirmar NO APARELHO que o vazamento sumiu.
//
// E ha ainda o aviso do iOS, anotado de outro lugar (player_controller).

/// A folga do primeiro instante, pra saber o quanto o aplicativo cresceu.
static FOLGA_INICIAL: AtomicU64 = AtomicU64::new(0);
/// A menor folga ja vista nesta sessao.
static MENOR_FOLGA: AtomicU64 = AtomicU64::new(u64::MAX);

/// Anota a memoria restante. Chamada de tempos em tempos pelo laco do jogo.
///
/// Os contadores do Ruffle vem de fora porque so quem tem o jogo em maos
/// consegue perguntar a ele.
pub fn anotar_memoria(contagem: Option<(usize, usize, usize, usize, usize, usize, usize)>) {
    let Some(mb) = memoria_livre_mb() else {
        anotar("memoria livre: indisponivel (simulador)");
        return;
    };

    let inicial = FOLGA_INICIAL.load(Ordering::Relaxed);
    if inicial == 0 {
        FOLGA_INICIAL.store(mb, Ordering::Relaxed);
    }
    let inicial = if inicial == 0 { mb } else { inicial };
    let usado = inicial.saturating_sub(mb);

    let menor = MENOR_FOLGA.fetch_min(mb, Ordering::Relaxed).min(mb);

    let numeros = match contagem {
        Some((swfs, figuras, carregamentos, segurando, texturas, texturas_mb, malhas)) => format!(
            " | swfs {swfs} figuras {figuras} carreg {carregamentos} segurando {segurando} | texturas {texturas} ({texturas_mb} MB) malhas {malhas}"
        ),
        None => String::new(),
    };

    let epoca = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let ligado = tempo_ligado(epoca);

    let metal = match memoria_do_metal_mb() {
        Some(mb) => format!(" | metal {mb} MB"),
        None => String::new(),
    };

    anotar(&format!(
        "memoria livre: {mb} MB | usado {usado} MB | minimo {menor} MB{metal}{ligado}{numeros}"
    ));
}

/// O iOS avisou que a memoria esta apertando.
///
/// ESTE AVISO E A RESPOSTA QUE FALTAVA. Se ele aparecer antes da morte, foi o
/// aparelho que apertou e o sistema escolheu a maior vitima. Se a sessao
/// terminar sem nenhum, o aplicativo estourou sozinho.
pub fn anotar_aviso_de_memoria() {
    match memoria_livre_mb() {
        Some(mb) => anotar(&format!("AVISO DE MEMORIA DO IOS (folga: {mb} MB)")),
        None => anotar("AVISO DE MEMORIA DO IOS"),
    }
}

// ------------------------------------------- O QUE ACONTECEU DA ULTIMA VEZ

/// O registro da sessao passada, inteiro.
pub fn anterior() -> Option<String> {
    let texto = fs::read_to_string(caminho_anterior()?).ok()?;
    if texto.trim().is_empty() {
        None
    } else {
        Some(texto)
    }
}

/// A sessao passada terminou mal?
///
/// A regra e simples e cobre todos os casos: terminou bem quem anotou a ida
/// pro fundo por ultimo. Fechar pelo seletor de aplicativos tambem passa por
/// ali — o sistema manda o aplicativo pro fundo antes de encerra-lo —, entao
/// quem so fechou o aplicativo na mao nao e acusado de nada.
pub fn anterior_morreu() -> bool {
    let Some(texto) = anterior() else {
        return false;
    };

    if texto.contains(MARCA_DE_QUEBRA) {
        return true;
    }

    let ultima = texto
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .next_back()
        .unwrap_or("");

    !ultima.contains(MARCA_DE_FUNDO)
}

pub fn limpar_anterior() {
    if let Some(alvo) = caminho_anterior() {
        let _ = fs::remove_file(alvo);
    }
}
