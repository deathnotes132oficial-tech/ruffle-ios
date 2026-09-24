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

    if let Ok(mut arquivo) = OpenOptions::new().create(true).append(true).open(&alvo) {
        if recomecou {
            let _ = writeln!(arquivo, "[{segundos}] (registro recomecado por tamanho)");
        }
        let _ = writeln!(arquivo, "[{segundos}] {texto}");
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
    anotar("sessao aberta");
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

/// Anota a memoria restante. Chamada de tempos em tempos pelo laco do jogo.
pub fn anotar_memoria() {
    match memoria_livre_mb() {
        Some(mb) => anotar(&format!("memoria livre: {mb} MB")),
        None => anotar("memoria livre: indisponivel (simulador)"),
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
