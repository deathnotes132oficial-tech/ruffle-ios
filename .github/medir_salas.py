"""
O CONTROLE REMOTO DO ROTEIRO, DO LADO DE FORA DO SIMULADOR.

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

ESPERA = 9

os.makedirs(SAIDA, exist_ok=True)
t0 = time.time()


def sh(*args):
    return subprocess.run(args, capture_output=True, text=True).stdout.strip()


DOCS = os.path.join(sh("xcrun", "simctl", "get_app_container", ID, APP, "data"), "Documents")
print("pasta do aplicativo:", DOCS, flush=True)


def memoria():
    """MB ocupados pelo processo do aplicativo (RSS). None = nao esta rodando."""
    total, achou = 0, False
    for linha in sh("ps", "-Ao", "rss=,comm=").splitlines():
        if "ruffle-ios" in linha.lower():
            achou = True
            total += int(linha.split()[0])
    return round(total / 1024) if achou else None


def anotar(ciclo, fase):
    linha = {"s": round(time.time() - t0), "ciclo": ciclo, "fase": fase, "mb": memoria()}
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


def na_lista():
    img = foto()
    x0, y0 = no_palco(img, X_DA_LISTA[0], X_DA_LISTA[1])
    x1, y1 = no_palco(img, X_DA_LISTA[2], X_DA_LISTA[3])
    px = list(img.crop((int(x0), int(y0), int(x1), int(y1))).getdata())
    vermelho = sum(1 for r, g, b in px if r > 150 and g < 70 and b < 70)
    return 100 * vermelho / max(1, len(px)) >= 5


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
    foto("parou.png")
    print("PAROU no ciclo %d: %s (foto: parou.png)" % (ciclo, motivo), flush=True)
    resumir()
    sys.exit(2)


def resumir():
    """Mesma conta do navegador: crescimento depois do aquecimento."""
    try:
        linhas = [json.loads(l) for l in open(os.path.join(SAIDA, "medicao.jsonl"))]
    except FileNotFoundError:
        return
    com = [l for l in linhas if l["mb"] is not None and l["ciclo"] >= 1]
    if len(com) < 2:
        return
    ini = next((l for l in com if l["ciclo"] >= 4), com[0])
    fim = com[-1]
    minutos = max((fim["s"] - ini["s"]) / 60, 0.01)
    pisos = {}
    for l in com:
        pisos.setdefault(l["ciclo"], []).append(l["mb"])
    texto = (
        "ciclos medidos: %d\n" % len([c for c in pisos if c <= CICLOS])
        + "memoria: %d MB -> %d MB do ciclo %d ao fim = %+.1f MB/min\n"
        % (ini["mb"], fim["mb"], ini["ciclo"], (fim["mb"] - ini["mb"]) / minutos)
        + "menor valor de cada ciclo: "
        + " ".join(str(min(v)) for c, v in sorted(pisos.items()) if c <= CICLOS)
        + "\n"
    )
    print(texto, flush=True)
    with open(os.path.join(SAIDA, "resumo.txt"), "w") as f:
        f.write(texto)


# ---------------------------------------------------------------- chegar
print("esperando o jogo subir...", flush=True)
esperar(120, 0, "carregando")
foto("0-depois-de-carregar.png")

chegou = False
for tentativa in range(1, 7):
    if na_lista():
        chegou = True
        break
    print("tentativa %d de chegar na lista de salas" % tentativa, flush=True)
    clicar(FECHA_ANUNCIO)
    esperar(3, 0, "chegando")
    clicar(FECHA_DICAS)
    esperar(3, 0, "chegando")
    clicar(ARENA)
    for _ in range(6):
        esperar(10, 0, "chegando")
        if na_lista():
            chegou = True
            break
    foto("chegando-%d.png" % tentativa)
    if chegou:
        break
if not chegou:
    parar("nao consegui chegar na lista de salas", 0)
foto("1-lista-de-salas.png")
esperar(20, 0, "base")

# ---------------------------------------------------------------- o gesto
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
        esperar(4, c, "fora")
        if not na_lista():
            parar("a porta nao devolveu a lista de salas", c)
    esperar(ESPERA - 4, c, "fora")
    print("ciclo %d/%d  memoria %s MB" % (c, CICLOS, memoria()), flush=True)

esperar(60, CICLOS + 1, "repouso")
foto("3-fim.png")
resumir()
