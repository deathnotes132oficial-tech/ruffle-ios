//! MODO DE TESTE: O APLICATIVO OBEDECE A UM ARQUIVO DE COMANDOS.
//!
//! POR QUE EXISTE
//! --------------
//! O vazamento de memoria so aparece abrindo e fechando coisa no jogo — criar
//! uma sala, sair, criar outra. No navegador do PC isso foi medido com um
//! controle remoto clicando sozinho: a base antiga do Ruffle crescia ~40 MB por
//! minuto, a nova parou de crescer depois da sexta sala. Faltava medir o MESMO
//! gesto no iOS, e o simulador nao tem como tocar na tela.
//!
//! COMO FUNCIONA
//! -------------
//! Com o argumento de abertura `--roteiro`, de 250 em 250 ms o aplicativo olha
//! `Documents/comando.txt`. Se encontrar `clique X Y`, em coordenada do PALCO
//! do jogo (1000x600), ele:
//!
//!   1. converte pra posicao na tela, do mesmo jeito que o Ruffle desenha o
//!      palco (cabendo inteiro, centralizado);
//!   2. move o mouse ate la, espera 400 ms, aperta, espera 250 ms, solta;
//!   3. apaga o comando e escreve o numero do clique em `comando-feito.txt`.
//!
//! Os cliques vao pelo `PlayerEvent`, o MESMO canal do toque de verdade. Pro
//! Ruffle e mouse real — inclusive pra quem le `stage.mouseX`.
//!
//! O ritmo lento e herdado do teste no navegador: clique de 80 ms se perdia,
//! porque os botoes do jogo precisam ver o mouse por cima antes do clique.
//!
//! Quem decide QUANDO e ONDE clicar e o workflow, do lado de fora: ele enxerga
//! esta pasta no simulador, tira foto da tela, confere o estado e escreve o
//! proximo comando. Assim o aplicativo nao precisa saber nada do jogo.
//!
//! JOGADOR NENHUM ATIVA ISTO. Sem o argumento `--roteiro` na abertura — que
//! so a linha de comando do simulador consegue passar — nada aqui roda.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::Instant;

use objc2_foundation::NSProcessInfo;
use ruffle_core::events::MouseButton;
use ruffle_core::{Player, PlayerEvent};

#[derive(Clone, Copy)]
enum Fase {
    Parado,
    Movido { x: f64, y: f64, desde: Instant },
    Apertado { x: f64, y: f64, desde: Instant },
}

thread_local! {
    static LIGADO: Cell<Option<bool>> = const { Cell::new(None) };
    static FASE: Cell<Fase> = const { Cell::new(Fase::Parado) };
    static ULTIMA_OLHADA: Cell<Option<Instant>> = const { Cell::new(None) };
    static FEITOS: Cell<u64> = const { Cell::new(0) };
}

/// O modo de teste esta ligado nesta abertura?
pub fn ligado() -> bool {
    LIGADO.with(|l| {
        if let Some(v) = l.get() {
            return v;
        }
        let v = NSProcessInfo::processInfo()
            .arguments()
            .iter()
            .any(|a| a.to_string() == "--roteiro");
        if v {
            tracing::warn!("ROTEIRO: modo de teste ligado");
        }
        l.set(Some(v));
        v
    })
}

fn pasta() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var("HOME").ok()?).join("Documents"))
}

/// Le e consome o proximo comando, se houver: `clique X Y` no palco.
fn proximo_clique() -> Option<(f64, f64)> {
    let arquivo = pasta()?.join("comando.txt");
    let texto = std::fs::read_to_string(&arquivo).ok()?;
    let _ = std::fs::remove_file(&arquivo);
    let mut partes = texto.split_whitespace();
    if partes.next()? != "clique" {
        tracing::warn!("ROTEIRO: comando desconhecido: {texto:?}");
        return None;
    }
    let x: f64 = partes.next()?.parse().ok()?;
    let y: f64 = partes.next()?.parse().ok()?;
    Some((x, y))
}

fn avisar_feito() {
    let n = FEITOS.with(|f| {
        f.set(f.get() + 1);
        f.get()
    });
    if let Some(p) = pasta() {
        let _ = std::fs::write(p.join("comando-feito.txt"), n.to_string());
    }
}

/// Um passo do roteiro. Chamado a cada quadro pela tela do jogo.
///
/// `largura` e `altura` sao os pixels fisicos da tela do jogo — os mesmos em
/// que o toque comum e entregue ao Ruffle.
pub fn passo(player: &mut Player, largura: f64, altura: f64) {
    let agora = Instant::now();
    match FASE.with(|f| f.get()) {
        Fase::Parado => {
            let olhar = ULTIMA_OLHADA.with(|u| match u.get() {
                Some(antes) => agora.duration_since(antes).as_millis() >= 250,
                None => true,
            });
            if !olhar {
                return;
            }
            ULTIMA_OLHADA.with(|u| u.set(Some(agora)));

            let Some((px, py)) = proximo_clique() else {
                return;
            };
            // O palco cabe inteiro na tela e fica centralizado — a mesma
            // conta que o Ruffle faz pra desenhar.
            let s = (largura / 1000.0).min(altura / 600.0);
            let x = (largura - 1000.0 * s) / 2.0 + px * s;
            let y = (altura - 600.0 * s) / 2.0 + py * s;
            tracing::warn!("ROTEIRO: clique no palco {px},{py} -> tela {x:.0},{y:.0}");

            player.set_mouse_in_stage(true);
            player.handle_event(PlayerEvent::MouseMove { x, y });
            FASE.with(|f| f.set(Fase::Movido { x, y, desde: agora }));
        }
        Fase::Movido { x, y, desde } => {
            if agora.duration_since(desde).as_millis() < 400 {
                return;
            }
            player.handle_event(PlayerEvent::MouseDown {
                x,
                y,
                button: MouseButton::Left,
                index: Some(1),
            });
            FASE.with(|f| f.set(Fase::Apertado { x, y, desde: agora }));
        }
        Fase::Apertado { x, y, desde } => {
            if agora.duration_since(desde).as_millis() < 250 {
                return;
            }
            player.handle_event(PlayerEvent::MouseUp {
                x,
                y,
                button: MouseButton::Left,
            });
            FASE.with(|f| f.set(Fase::Parado));
            avisar_feito();
        }
    }
}
