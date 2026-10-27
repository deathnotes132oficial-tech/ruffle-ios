"""
O CONTROLE REMOTO DO ROTEIRO, DO LADO DE FORA DO SIMULADOR.

DOIS GESTOS (variavel GESTO):
  sociedade (padrao) — do salao, abre o Clube da Sociedade e volta pela porta,
      N vezes. E o que o testador contou: "abre a sociedade, o aplicativo
      fecha". A tela da Sociedade e inteira, com muita figura.
  salas — o gesto antigo, descrito abaixo.

A REGUA E O "PHYSICAL FOOTPRINT", NAO O RSS. E o numero que o iOS usa pra
matar o aplicativo (e o que a tela do iPhone mostrava chegando a 4 GB). O RSS
do simulador fica menor que ele, porque o macOS comprime memoria parada — as
primeiras rodadas mediram RSS e nao viam o que o iPhone via. Agora cada linha
guarda os dois, mais o PICO do footprint desde que o app abriu.

E o mesmo gesto do teste no navegador (scratchpad/navegador/medir.js), so que
no iOS: na lista de salas do Salao de Jogos, clica "Inicio" (cria uma sala),
espera, clica a porta (sai), espera — N vezes — medindo a memoria do processo
de 2 em 2 segundos.

O aplicativo nao sabe nada do jogo. Ele so obedece a `Documents/comando.txt`
(ver src/roteiro.rs). Quem decide quando e onde clicar e este script: ele
enxerga a pasta do aplicativo no simulador, tira foto da tela e confere o
estado antes de cada clique.

A LISTA DE SALAS E RECONHECIDA PELO X VERMELHO DE FECHAR no canto (11% de
pixels vermelhos na lista; 0% na sala, no salao e na janela de sair). Inicio
que nao tira da lista e repetido uma vez; porta que nao devolve a lista PARA
a medicao com foto. No navegador, uma rodada que nao conferia desandou no
ciclo 43 e mediu 15 ciclos de jogo parado como se fossem salas.
"""

import json
import os
import re
import subprocess
import sys
import time

from PIL import Image

ID = os.environ["ID"]
APP = os.environ["APP"]
CICLOS = int(os.environ.get("CICLOS", "20"))
GIRAR = os.environ.get("GIRAR", "1") == "1"   # iPhone: a foto sai em pe com o jogo deitado
SAIDA = "fotos"

# Em coordenada do PALCO do jogo (1000x600) — as mesmas do teste no navegador.
INICIO = (647, 516)          # "Inicio" na lista de salas
PORTA = (964, 564)           # a porta, dentro da sala
FECHA_ANUNCIO = (808, 160)   # X do "Anuncio do jogo", ao entrar
FECHA_DICAS = (945, 362)     # X do aviso de check-in
ARENA = (518, 251)           # o predio do Salao Arena, no salao
X_DA_LISTA = (895, 28, 930, 58)
GESTO = os.environ.get("GESTO", "sociedade")

# Clube da Sociedade. Conferido no navegador: depois da porta o salao volta
# sempre centralizado, entao o velhinho da Sociedade fica no mesmo lugar. O
# painel de missoes do lado direito volta aberto e cobre ele — por isso e
# recolhido antes (so se estiver na tela: clicar ali com ele fechado acerta
# o chao perto da Capela).
SOCIEDADE = (918, 353)       # o NPC "Clube da Sociedade", no salao
RECOLHE_MISSOES = (791, 270) # a setinha do painel de missoes
ABAS_DA_SOCIEDADE = (771, 138, 827, 163)  # abas verdes "Missao Guilda"/"Aviso": 42% na Sociedade, <1% fora
PAINEL_DE_MISSOES = (810, 300, 990, 460)  # fundo escuro do painel: 25% aberto, <3% recolhido

# O QUE ESTA ABERTO NA TELA, PELA COR DO BOTAO DE FECHAR.
#
# A primeira rodada com o jogo de verdade clicava "fechar o anuncio" AS CEGAS.
# Naquele dia a conta ja tinha visto o anuncio (no teste do navegador), entao
# ele nao apareceu — e o clique caiu no predio da Capela, que abriu o "Sistema
# de Auditorio". Logo depois o jogo desconectou. Pode ter sido coincidencia;
# clicar no escuro nao e. Agora so se fecha o que se ve.
#
# Conferido em fotos reais de cada situacao (lista, salao com check-in,
# desconectado, anuncio, salao limpo, sala): cada detector so acende na sua.
X_DO_CHECKIN = (930, 348, 962, 378)
X_DO_ANUNCIO = (792, 144, 826, 178)
X_DO_DESCONECTADO = (727, 213, 758, 244)

ESPERA = 9

os.makedirs(SAIDA, exist_ok=True)
t0 = time.time()


def sh(*args):
    return subprocess.run(args, capture_output=True, text=True).stdout.strip()


def tudo(*args):
    """Saida e erro juntos: ferramenta que falha calada nao vira linha vazia."""
    try:
        r = subprocess.run(args, capture_output=True, text=True, timeout=600)
        return r.stdout + r.stderr
    except subprocess.TimeoutExpired:
        return "(passou de 10 min)"


DOCS = os.path.join(sh("xcrun", "simctl", "get_app_container", ID, APP, "data"), "Documents")
print("pasta do aplicativo:", DOCS, flush=True)


def processo():
    """(pid, rss em MB) do aplicativo, ou (None, None) se nao esta rodando."""
    for linha in sh("ps", "-Ao", "pid=,rss=,comm=").splitlines():
        if "ruffle-ios" in linha.lower():
            pid, rss = linha.split()[:2]
            return pid, round(int(rss) / 1024)
    return None, None


def em_mb(texto):
    n = re.match(r"([\d.]+)\s*([KMGT]?)", texto.strip())
    if not n:
        return None
    return round(float(n.group(1)) * {"": 1 / 2**20, "K": 1 / 1024, "M": 1, "G": 1024, "T": 2**20}[n.group(2)])


def footprint(pid):
    """(atual, pico) do physical footprint em MB. vmmap primeiro; footprint de reserva."""
    saida = sh("sudo", "-n", "vmmap", "--summary", pid)
    atual = re.search(r"^Physical footprint:\s+(\S+)", saida, re.M)
    pico = re.search(r"^Physical footprint \(peak\):\s+(\S+)", saida, re.M)
    if atual:
        return em_mb(atual.group(1)), em_mb(pico.group(1)) if pico else None
    saida = sh("sudo", "-n", "footprint", "-p", pid)
    atual = re.search(r"phys_footprint:\s+([\d.]+\s*[KMGT]?)", saida)
    pico = re.search(r"phys_footprint_peak:\s+([\d.]+\s*[KMGT]?)", saida)
    return (em_mb(atual.group(1)) if atual else None), (em_mb(pico.group(1)) if pico else None)


def memoria():
    """MB pela regua do iOS (physical footprint). None = nao esta rodando."""
    pid, _ = processo()
    return footprint(pid)[0] if pid else None


def anotar(ciclo, fase):
    pid, rss = processo()
    atual, pico = footprint(pid) if pid else (None, None)
    linha = {"s": round(time.time() - t0), "ciclo": ciclo, "fase": fase,
             "mb": atual, "pico": pico, "rss": rss}
    with open(os.path.join(SAIDA, "medicao.jsonl"), "a") as f:
        f.write(json.dumps(linha) + "\n")
    return linha


def esperar(segundos, ciclo, fase):
    fim = time.time() + segundos
    while time.time() < fim:
        anotar(ciclo, fase)
        time.sleep(2)


def foto(nome=None):
    caminho = "/tmp/agora.png"
    subprocess.run(["xcrun", "simctl", "io", ID, "screenshot", caminho], capture_output=True)
    img = Image.open(caminho).convert("RGB")
    if GIRAR and img.height > img.width:
        img = img.rotate(90, expand=True)
    if nome:
        img.save(os.path.join(SAIDA, nome))
    return img


def no_palco(img, x, y):
    w, h = img.size
    s = min(w / 1000, h / 600)
    return (w - 1000 * s) / 2 + x * s, (h - 600 * s) / 2 + y * s


def vermelho(r, g, b):
    return r > 150 and g < 70 and b < 70


def amarelo(r, g, b):
    return r > 200 and g > 160 and b < 90


def tem(img, caixa, cor, minimo):
    x0, y0 = no_palco(img, caixa[0], caixa[1])
    x1, y1 = no_palco(img, caixa[2], caixa[3])
    px = list(img.crop((int(x0), int(y0), int(x1), int(y1))).getdata())
    return 100 * sum(1 for r, g, b in px if cor(r, g, b)) / max(1, len(px)) >= minimo


def na_lista(img=None):
    return tem(img or foto(), X_DA_LISTA, vermelho, 5)


def desconectado(img=None):
    return tem(img or foto(), X_DO_DESCONECTADO, vermelho, 5)


def clicar(ponto, prazo=15):
    """Escreve o comando e espera o aplicativo confirmar que clicou."""
    feito = os.path.join(DOCS, "comando-feito.txt")
    antes = open(feito).read().strip() if os.path.exists(feito) else ""
    with open(os.path.join(DOCS, "comando.txt"), "w") as f:
        f.write("clique %d %d" % ponto)
    fim = time.time() + prazo
    while time.time() < fim:
        if os.path.exists(feito) and open(feito).read().strip() != antes:
            return True
        time.sleep(0.25)
    return False


def parar(motivo, ciclo):
    img = foto("parou.png")
    if desconectado(img) and "DESCONECT" not in motivo:
        motivo += " — e o jogo esta DESCONECTADO"
    print("PAROU no ciclo %d: %s (foto: parou.png)" % (ciclo, motivo), flush=True)
    resumir()
    sys.exit(2)


def resumir():
    """O pico e o que importa no iPhone: o iOS mata no pico, nao na media."""
    try:
        linhas = [json.loads(l) for l in open(os.path.join(SAIDA, "medicao.jsonl"))]
    except FileNotFoundError:
        return
    vivas = [l for l in linhas if l["mb"] is not None]
    if not vivas:
        return
    carga = [l["mb"] for l in vivas if l["ciclo"] == 0]
    texto = "regua: physical footprint (o numero que o iOS usa pra fechar o app)\n"
    texto += "maior footprint visto carregando o jogo: %s MB\n" % (max(carga) if carga else "-")
    texto += "maior footprint visto no teste todo: %d MB\n" % max(l["mb"] for l in vivas)
    picos = [l["pico"] for l in vivas if l.get("pico")]
    if picos:
        texto += "pico guardado pelo sistema: %d MB\n" % max(picos)
    if linhas[-1]["mb"] is None:
        texto += "O APLICATIVO MORREU no ciclo %d (fase %s)\n" % (linhas[-1]["ciclo"], linhas[-1]["fase"])
    por_ciclo = {}
    for l in vivas:
        if 1 <= l["ciclo"] <= CICLOS:
            por_ciclo.setdefault(l["ciclo"], []).append(l["mb"])
    if por_ciclo:
        texto += "ciclos medidos: %d\n" % len(por_ciclo)
        texto += "maior de cada ciclo: " + " ".join(str(max(v)) for _, v in sorted(por_ciclo.items())) + "\n"
        texto += "menor de cada ciclo: " + " ".join(str(min(v)) for _, v in sorted(por_ciclo.items())) + "\n"
    print(texto, flush=True)
    with open(os.path.join(SAIDA, "resumo.txt"), "w") as f:
        f.write(texto)


def fechar_avisos(img, tentativa):
    """Fecha o anuncio ou o check-in, se estiver na tela. True = fechou algo."""
    if tem(img, X_DO_ANUNCIO, amarelo, 10):
        print("tentativa %d: fechando o anuncio" % tentativa, flush=True)
        clicar(FECHA_ANUNCIO)
        esperar(4, 0, "chegando")
        return True
    if tem(img, X_DO_CHECKIN, vermelho, 5):
        print("tentativa %d: fechando o aviso de check-in" % tentativa, flush=True)
        clicar(FECHA_DICAS)
        esperar(4, 0, "chegando")
        return True
    return False


def na_sociedade(img=None):
    return tem(img or foto(), ABAS_DA_SOCIEDADE, lambda r, g, b: r < 90 and 140 < g and 120 < b < 220 and g > r + 60, 20)


def missoes_abertas(img):
    return tem(img, PAINEL_DE_MISSOES, lambda r, g, b: r < 70 and g < 60 and b < 50, 15)


def esperar_ate(condicao, segundos, ciclo, fase):
    """Mede enquanto espera; olha a tela a cada ~6 s. True = a condicao chegou."""
    fim = time.time() + segundos
    while time.time() < fim:
        esperar(4, ciclo, fase)
        if processo()[0] is None:
            parar("o APLICATIVO MORREU", ciclo)
        if condicao():
            return True
    return False


# ---------------------------------------------------------------- chegar
print("esperando o jogo subir...", flush=True)
esperar(120, 0, "carregando")
foto("0-depois-de-carregar.png")
if processo()[0] is None:
    parar("o APLICATIVO MORREU carregando o jogo", 0)

if GESTO == "sociedade":
    # Chegar = salao limpo: sem anuncio, sem check-in.
    for tentativa in range(1, 9):
        img = foto("chegando-%d.png" % tentativa)
        if desconectado(img):
            parar("o jogo DESCONECTOU antes de chegar no salao", 0)
        if not fechar_avisos(img, tentativa):
            break
    else:
        parar("os avisos nao fecharam", 0)
    foto("1-salao.png")
    esperar(20, 0, "base")

    for c in range(1, CICLOS + 1):
        for chance in (1, 2):
            img = foto()
            if missoes_abertas(img):
                clicar(RECOLHE_MISSOES)
                esperar(2, c, "salao")
            clicar(SOCIEDADE)
            # O personagem anda ate o velhinho antes de abrir: leva uns segundos.
            if esperar_ate(na_sociedade, 24, c, "abrindo"):
                break
        else:
            parar("a Sociedade nao abriu duas vezes", c)
        if c == 1:
            foto("2-sociedade.png")
        esperar(ESPERA, c, "na-sociedade")

        for chance in (1, 2):
            clicar(PORTA)
            if esperar_ate(lambda: not na_sociedade(), 16, c, "saindo"):
                break
        else:
            parar("a porta nao fechou a Sociedade duas vezes", c)
        esperar(ESPERA, c, "salao")
        print("ciclo %d/%d  footprint %s MB" % (c, CICLOS, memoria()), flush=True)
        # ONDE ESTA A MEMORIA. O footprint separa grafico (IOAccelerator,
        # IOSurface) de CPU (MALLOC); o heap conta os blocos vivos por tipo.
        # (Rastro de alocacao com MallocStackLogging no lancamento impediu o
        # app de abrir no simulador, 27/10.)
        pid, _ = processo()
        if pid:
            with open(os.path.join(SAIDA, "regioes.txt"), "a") as f:
                f.write("==== ciclo " + str(c) + "\n" + tudo("sudo", "-n", "footprint", "-p", str(pid)) + "\n")
            with open(os.path.join(SAIDA, "heap-%d.txt" % c), "w") as f:
                f.write(tudo("sudo", "-n", "heap", "-s", str(pid))[:400000] + "\n")

else:
    chegou = False
    for tentativa in range(1, 9):
        img = foto("chegando-%d.png" % tentativa)
        if na_lista(img):
            chegou = True
            break
        if desconectado(img):
            parar("o jogo DESCONECTOU antes de chegar na lista", 0)
        if fechar_avisos(img, tentativa):
            continue
        # Nada aberto: o salao esta limpo, e so agora o predio e clicado.
        print("tentativa %d: entrando no Salao Arena" % tentativa, flush=True)
        clicar(ARENA)
        for _ in range(6):
            esperar(10, 0, "chegando")
            if na_lista():
                chegou = True
                break
        if chegou:
            break
    if not chegou:
        parar("nao consegui chegar na lista de salas", 0)
    foto("1-lista-de-salas.png")
    esperar(20, 0, "base")

    for c in range(1, CICLOS + 1):
        clicar(INICIO)
        esperar(4, c, "na-sala")
        if na_lista():
            clicar(INICIO)  # o clique se perdeu: uma segunda chance, nao mais
            esperar(5, c, "na-sala")
            if na_lista():
                parar("Inicio nao abriu sala duas vezes", c)
        if c == 1:
            foto("2-dentro-da-sala.png")
        esperar(ESPERA - 4, c, "na-sala")

        clicar(PORTA)
        esperar(4, c, "fora")
        if not na_lista():
            # Mesma segunda chance do Inicio: um clique as vezes se perde. Na
            # primeira rodada completa, a porta pegou nos ciclos 1 e 2 e nao no 3,
            # com o jogo parado dentro da sala esperando.
            clicar(PORTA)
            esperar(5, c, "fora")
            if not na_lista():
                parar("a porta nao devolveu a lista de salas duas vezes", c)
        esperar(ESPERA - 4, c, "fora")
        print("ciclo %d/%d  footprint %s MB" % (c, CICLOS, memoria()), flush=True)

esperar(60, CICLOS + 1, "repouso")
foto("3-fim.png")
resumir()
