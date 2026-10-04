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

use std::ffi::{c_char, c_void};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::panic;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// O NOME DE FABRICA DO APARELHO, do tipo "iPhone14,5".
///
/// Sem ele nao da pra montar tabela de compatibilidade nenhuma: dois
/// registros podem mostrar tetos de memoria completamente diferentes e nao
/// havia como saber de que aparelhos eram. Com ele, cada relato que chega ja
/// vem dizendo em que modelo aconteceu.
fn modelo_do_aparelho() -> Option<String> {
    extern "C" {
        fn sysctlbyname(
            nome: *const c_char,
            saida: *mut c_void,
            tamanho: *mut usize,
            entrada: *mut c_void,
            tamanho_entrada: usize,
        ) -> i32;
    }

    let chave = b"hw.machine\0";
    let mut tamanho: usize = 0;

    unsafe {
        // A primeira chamada so pergunta o tamanho; a segunda traz o texto.
        if sysctlbyname(
            chave.as_ptr() as *const c_char,
            std::ptr::null_mut(),
            &mut tamanho,
            std::ptr::null_mut(),
            0,
        ) != 0
            || tamanho == 0
            || tamanho > 256
        {
            return None;
        }

        let mut buraco = vec![0u8; tamanho];
        if sysctlbyname(
            chave.as_ptr() as *const c_char,
            buraco.as_mut_ptr() as *mut c_void,
            &mut tamanho,
            std::ptr::null_mut(),
            0,
        ) != 0
        {
            return None;
        }

        // Vem terminado em zero, que nao faz parte do nome.
        while buraco.last() == Some(&0) {
            buraco.pop();
        }
        String::from_utf8(buraco).ok()
    }
}

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
extern "C" {
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

    // QUEM E O APARELHO, LOGO NA PRIMEIRA LINHA.
    //
    // O teto de memoria muda de modelo pra modelo — ja se mediu 6111 MB num
    // e 2322 MB noutro. Sem saber qual e qual, dois registros nao se
    // comparam, e nenhuma recomendacao de aparelho passa de chute.
    let aparelho = modelo_do_aparelho().unwrap_or_else(|| "desconhecido".to_string());
    let teto = match memoria_livre_mb() {
        Some(mb) => format!("{mb} MB"),
        None => "indisponivel (simulador)".to_string(),
    };
    anotar(&format!("aparelho: {aparelho} | memoria disponivel ao abrir: {teto}"));
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


// ============ O APARELHO INTEIRO, E O QUE O APLICATIVO OCUPA NELE ============
//
// POR QUE AS DUAS MEDIDAS DE ANTES NAO SERVIAM
// --------------------------------------------
// `os_proc_available_memory` devolve a folga do APLICATIVO dentro do limite
// DELE. Quem decide matar olha o aparelho inteiro. Sao grandezas diferentes, e
// tratar uma como a outra produziu duas leituras erradas:
//
//   - uma sessao terminou com "1 GB livre" e parecia nao fazer sentido. Fazia:
//     o telefone e que estava cheio, nao a nossa cota;
//   - o numero da tela era `teto - folga`, uma subtracao. Quando o iOS mexia no
//     teto, ele saltava centenas de MB sem o aplicativo ter alocado nada — e
//     foram esses saltos fantasmas que eu passei dias caçando.
//
// Agora sao dois numeros absolutos, lidos direto do sistema: quanto o APARELHO
// tem livre, e quanto o APLICATIVO ocupa.

type KernReturn = i32;

extern "C" {
    fn mach_host_self() -> u32;
    fn mach_task_self() -> u32;
    fn host_page_size(maquina: u32, saida: *mut usize) -> KernReturn;
    fn host_statistics64(
        maquina: u32,
        sabor: i32,
        saida: *mut u32,
        quantos: *mut u32,
    ) -> KernReturn;
    fn task_info(tarefa: u32, sabor: u32, saida: *mut u32, quantos: *mut u32) -> KernReturn;
}

/// HOST_VM_INFO64. A contagem e o tamanho da estrutura em palavras de 32 bits:
/// quatro contadores de 32, nove de 64, dois de 32, quatro de 64, quatro de 32
/// e um de 64 dao 152 bytes — 38 palavras.
const HOST_VM_INFO64: i32 = 4;
const HOST_VM_INFO64_PALAVRAS: u32 = 38;

/// MACH_TASK_BASIC_INFO. Tres tamanhos de 64, dois tempos de 64, e dois de 32:
/// 48 bytes, 12 palavras.
const MACH_TASK_BASIC_INFO: u32 = 20;
const MACH_TASK_BASIC_INFO_PALAVRAS: u32 = 12;

/// Quanto o APARELHO tem livre, em MB. Conta as paginas livres mais as
/// inativas: inativa e memoria de outro app que o sistema pode tomar na hora,
/// entao ela conta como disponivel na decisao de quem matar.
pub fn aparelho_livre_mb() -> Option<u64> {
    let mut pagina: usize = 0;
    let mut dados = [0u32; HOST_VM_INFO64_PALAVRAS as usize];
    let mut quantos = HOST_VM_INFO64_PALAVRAS;

    unsafe {
        let maquina = mach_host_self();
        if host_page_size(maquina, &mut pagina) != 0 || pagina == 0 {
            return None;
        }
        if host_statistics64(maquina, HOST_VM_INFO64, dados.as_mut_ptr(), &mut quantos) != 0 {
            return None;
        }
    }

    // As quatro primeiras palavras sao livre, ativa, inativa e presa — nessa
    // ordem, e sao as unicas que se le aqui. O resto da estrutura fica onde
    // esta: nao depender do formato inteiro e o que torna isto seguro.
    let livres = dados[0] as u64;
    let inativas = dados[2] as u64;
    Some((livres + inativas) * pagina as u64 / (1024 * 1024))
}

/// Quanto o APLICATIVO ocupa de verdade, em MB.
///
/// Numero absoluto, do proprio sistema — nao a subtracao de antes. E o que o
/// iOS enxerga quando precisa escolher um processo pra encerrar.
pub fn app_residente_mb() -> Option<u64> {
    let mut dados = [0u32; MACH_TASK_BASIC_INFO_PALAVRAS as usize];
    let mut quantos = MACH_TASK_BASIC_INFO_PALAVRAS;

    unsafe {
        if task_info(
            mach_task_self(),
            MACH_TASK_BASIC_INFO,
            dados.as_mut_ptr(),
            &mut quantos,
        ) != 0
        {
            return None;
        }
    }

    // Primeiro campo e o tamanho virtual, segundo o residente — os dois de 64
    // bits, entao o residente sao as palavras 2 e 3.
    let residente = (dados[2] as u64) | ((dados[3] as u64) << 32);
    if residente == 0 {
        None
    } else {
        Some(residente / (1024 * 1024))
    }
}

// NAO ADIANTA PEDIR MEMORIA DE VOLTA AO malloc.
//
// Havia aqui uma chamada a `malloc_zone_pressure_relief`, apostando que as
// listas livres do alocador estivessem segurando centenas de MB em nome do
// aplicativo. Medido em oito avisos seguidos de memoria, ela devolveu
// ZERO MB todas as vezes.
//
// A aposta nasceu de uma conta errada: eu comparava o que o Rust tinha vivo
// com o medidor antigo, que inflava. Com os numeros certos a conta fecha
// sozinha — `rust` mais `metal` dao quase exatamente o que o sistema cobra,
// e nao sobra pilha escondida pra devolver.
//
// Fica o registro pra ninguem tentar de novo.


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
pub fn anotar_memoria(contagem: Option<String>) {
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

    // A LINHA VEM PRONTA DO RUFFLE, DE PROPOSITO.
    //
    // Quem sabe o que existe pra contar e ele. Recebendo os numeros soltos,
    // toda medida nova obrigava a mexer nos dois repositorios; recebendo o
    // texto, ela mexe so no dele.
    let numeros = match contagem {
        Some(texto) => format!(" | {texto}"),
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

    // O QUE O ALOCADOR SABE, EM TODA LINHA.
    //
    // "usado" e a conta do iOS: inclui a placa de video, o Objective-C e a
    // fragmentacao. "rust" e so o que o nosso lado pediu e ainda nao
    // devolveu. A diferenca entre os dois e o que diz se o buraco esta do
    // nosso lado ou nao — e era justamente isso que faltava saber.
    let alocador = crate::alocador::resumo();

    // OS DOIS LADOS DA MESMA MORTE.
    //
    // "aparelho" diz se o telefone inteiro apertou; "app" diz se fomos nos que
    // crescemos. Sem os dois juntos nao da pra saber qual das duas coisas
    // matou — e elas pedem consertos opostos.
    let aparelho = match aparelho_livre_mb() {
        Some(livre) => format!("{livre} MB livres"),
        None => "indisponivel".to_string(),
    };
    let app = app_residente_mb().unwrap_or(usado);

    anotar(&format!(
        "aparelho {aparelho} | app {app} MB | cota livre {mb} MB | minimo {menor} MB{metal}{ligado} | {alocador}{numeros}"
    ));
}

/// Quanto o aplicativo cresceu desde que abriu, pra mostrar na tela.
///
/// A mesma conta da linha do registro, sem escrever nada: quem desenha na
/// tela nao pode gravar em disco a cada quadro.
pub fn usado_mb() -> Option<u64> {
    // O QUE O APLICATIVO OCUPA, E NAO O QUANTO ELE CRESCEU.
    //
    // Antes isto era `folga inicial - folga de agora`. A conta tinha um
    // defeito grave: quando o iOS mexia no teto, o numero saltava centenas de
    // MB sem o aplicativo ter alocado coisa alguma. Era mentira na tela, e
    // mentira no registro.
    //
    // Agora e o tamanho residente do processo, lido do sistema. Se o sistema
    // nao responder, cai no jeito antigo em vez de sumir com o numero.
    if let Some(mb) = app_residente_mb() {
        return Some(mb);
    }

    let mb = memoria_livre_mb()?;
    let inicial = FOLGA_INICIAL.load(Ordering::Relaxed);
    if inicial == 0 {
        return Some(0);
    }
    Some(inicial.saturating_sub(mb))
}

/// O iOS avisou que a memoria esta apertando.
///
/// ESTE AVISO E A RESPOSTA QUE FALTAVA. Se ele aparecer antes da morte, foi o
/// aparelho que apertou e o sistema escolheu a maior vitima. Se a sessao
/// terminar sem nenhum, o aplicativo estourou sozinho.
/// O MAIOR USO JA VISTO, e a linha que o registra.
static MAIOR_USO: AtomicU64 = AtomicU64::new(0);

/// Marca quando o uso bate um recorde, e so entao.
///
/// O que derruba o aplicativo nao e o patamar: e o salto. Duas medidas
/// seguidas mostraram 1160 MB e, menos de um segundo depois, 1959 MB — e a
/// morte coube entre duas dessas. Uma linha por recorde deixa os saltos
/// achaveis sem ter que ler o registro inteiro.
///
/// So vale subida de 50 MB pra cima: o numero do iOS oscila dezenas de MB
/// sozinho, e recorde a cada oscilacao encheria o registro de ruido.
pub fn marcar_pico() {
    let Some(usado) = usado_mb() else {
        return;
    };

    let maior = MAIOR_USO.load(Ordering::Relaxed);
    if usado < maior + 50 {
        return;
    }
    MAIOR_USO.store(usado, Ordering::Relaxed);

    // O primeiro "recorde" e so a abertura, e nao diz nada.
    if maior == 0 {
        return;
    }
    retrato(&format!("PICO {usado} MB (antes {maior} MB)"));
}

/// DEVOLVER O QUE DA, AGORA.
///
/// O iOS avisa antes de encerrar um aplicativo — e esse aviso e a unica chance
/// de evitar ser o escolhido. Ate aqui o aplicativo so ANOTAVA o aviso e nao
/// devolvia nada, o que esta escrito no proprio codigo desde o dia em que a
/// anotacao foi posta.
///
/// Anota quanto saiu em cada passo, de proposito: se um deles nao render nada,
/// isso aparece no registro e a gente para de insistir nele.
pub fn aliviar(soltou_do_jogo: Option<String>) {
    let antes_app = app_residente_mb();
    let antes_rust = crate::alocador::vivo_mb();

    let depois_app = app_residente_mb();
    let caiu = match (antes_app, depois_app) {
        (Some(a), Some(d)) => format!("{} MB", a.saturating_sub(d)),
        _ => "?".to_string(),
    };

    anotar(&format!(
        "ALIVIO: app caiu {caiu} | rust antes {antes_rust} MB depois {} MB{}",
        crate::alocador::vivo_mb(),
        match soltou_do_jogo {
            Some(texto) => format!(" | {texto}"),
            None => String::new(),
        }
    ));
}

/// O retrato completo, pra quando algo importante acontece.
///
/// E a mesma informacao da linha de sempre, mas pedida de proposito nos dois
/// momentos que contam: quando o iOS avisa que esta apertando, e quando se
/// bate um pico novo. Nesses instantes a linha periodica pode estar a um
/// segundo de distancia — e um segundo, aqui, ja escondeu 900 MB.
pub fn retrato(motivo: &str) {
    anotar(&format!("--- {motivo} --- {}", crate::alocador::resumo()));
}

pub fn anotar_aviso_de_memoria() {
    match memoria_livre_mb() {
        Some(mb) => anotar(&format!("AVISO DE MEMORIA DO IOS (folga: {mb} MB)")),
        None => anotar("AVISO DE MEMORIA DO IOS"),
    }
}

/// O registro da sessao de agora, inteiro.
///
/// Existe pra que haja ALGUM jeito de tirar isto do aparelho. O arquivo mora
/// na pasta do aplicativo, onde ninguem alcanca sem cabo e sem Xcode — e um
/// registro que so o aparelho le nao serve pra investigar nada.
pub fn atual() -> Option<String> {
    let texto = fs::read_to_string(caminho_atual()?).ok()?;
    if texto.trim().is_empty() {
        None
    } else {
        Some(texto)
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
