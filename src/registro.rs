//! O REGISTRO DO QUE ACONTECEU.
//!
//! POR QUE ISTO EXISTE
//! -------------------
//! O aplicativo fechou no iPhone do testador ao abrir a sociedade do jogo, e
//! nao sobrou nenhum rastro: os Dados de Analise do aparelho nao tinham nada
//! com o nosso nome, e o painel da Apple so mostra o que o testador escolhe
//! enviar. Sem Mac nao da pra espiar o console do aparelho.
//!
//! Quando o Ruffle quebra, ele diz o arquivo e a linha antes de morrer. Essa
//! mensagem existe — o que faltava era alguem guardando ela. E o que este
//! arquivo faz: guarda em disco, e a tela de entrada mostra na abertura
//! seguinte.
//!
//! O CICLO QUE ISSO CRIA
//! ---------------------
//! O jogador abre a sociedade, o aplicativo fecha, ele abre de novo e ve o
//! que houve — e manda um print. Sem Ajustes, sem cacar arquivo no celular,
//! sem depender de ninguem com um Mac na mao.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// A marca que separa "quebrou" de simples anotacao de passagem. So a
/// presenca dela faz a tela do registro aparecer.
pub const MARCA_DE_QUEBRA: &str = "QUEBROU";

/// Teto do arquivo. Passando disso ele recomeca do zero — um registro que
/// cresce pra sempre acaba ocupando o telefone do jogador, e o que interessa
/// e sempre o fim dele.
const TETO: u64 = 64 * 1024;

/// Onde o registro mora: dentro da propria pasta do aplicativo.
///
/// No iPhone o HOME de um aplicativo e a pasta dele, e so ele enxerga ali.
/// Nada disso vai parar em servidor nenhum.
fn caminho() -> Option<PathBuf> {
    let casa = std::env::var("HOME").ok()?;
    Some(PathBuf::from(casa).join("Documents").join("registro.txt"))
}

/// Escreve uma linha no registro. Nunca falha de forma barulhenta: se nao der
/// pra escrever, o aplicativo segue normalmente — registro que derruba o
/// programa seria pior que registro nenhum.
pub fn anotar(texto: &str) {
    let Some(alvo) = caminho() else { return };

    if let Some(pasta) = alvo.parent() {
        let _ = fs::create_dir_all(pasta);
    }
    if let Ok(dados) = fs::metadata(&alvo) {
        if dados.len() > TETO {
            let _ = fs::remove_file(&alvo);
        }
    }

    let segundos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if let Ok(mut arquivo) = OpenOptions::new().create(true).append(true).open(&alvo) {
        let _ = writeln!(arquivo, "[{segundos}] {texto}");
    }
}

/// Tudo que esta guardado, ou None se nao houver nada.
pub fn ler() -> Option<String> {
    let texto = fs::read_to_string(caminho()?).ok()?;
    if texto.trim().is_empty() {
        None
    } else {
        Some(texto)
    }
}

pub fn limpar() {
    if let Some(alvo) = caminho() {
        let _ = fs::remove_file(alvo);
    }
}

/// Passa a anotar toda quebra do programa, daqui pra frente.
///
/// O gancho anterior continua valendo: ele e quem escreve no console, e nas
/// provas do simulador e por ele que a mensagem aparece. Aqui so acrescentamos
/// a copia em disco, que e a que sobrevive ao aplicativo fechar.
pub fn instalar() {
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
