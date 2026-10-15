package {

    import flash.display.Sprite;
    import flash.display.StageAlign;
    import flash.display.StageScaleMode;
    import flash.events.Event;
    import flash.events.MouseEvent;
    import flash.text.TextField;
    import flash.text.TextFieldAutoSize;
    import flash.text.TextFormat;

    /**
     * UM ARQUIVO .SWF DE EXEMPLO, FEITO POR NOS.
     *
     * A Apple pediu um .swf pra poder revisar o aplicativo, e isso faz todo
     * sentido: o CF Client e um tocador de Flash que, de proposito, nao traz
     * conteudo de ninguem. Sem um arquivo pra abrir, nao ha o que avaliar.
     *
     * Mandar conteudo de terceiro esta fora de questao — foi isso que causou a
     * recusa 5.2.3. Entao este arquivo e nosso, escrito do zero.
     *
     * ── A ORDEM AQUI NAO E ESTILO, E CONSERTO ──
     *
     * A primeira versao desenhava os TEXTOS primeiro e as formas depois. No
     * simulador apareceu so o fundo azul: o palco carregava, o codigo rodava,
     * e nada mais se via.
     *
     * A explicacao que sobra e que criar o texto falhou — provavelmente a
     * fonte `_sans` que eu pedia nao existe no tocador — e, falhando ali, tudo
     * o que vinha depois era pulado. O unico desenho que restava era o fundo,
     * que vem de uma etiqueta do SWF e nao do codigo.
     *
     * Entao: FORMA PRIMEIRO, TEXTO DEPOIS, e o texto dentro de um try. Se a
     * fonte faltar de novo, perde-se a legenda e nao a demonstracao.
     */
    [SWF(width="800", height="480", frameRate="30", backgroundColor="#1B2440")]
    public class Exemplo extends Sprite {

        private var quadrado:Sprite;
        private var bola:Sprite;
        private var marcador:Sprite;
        private var contador:TextField;
        private var toques:int = 0;
        private var passo:Number = 5;

        public function Exemplo() {
            if (stage) {
                comecar();
            } else {
                addEventListener(Event.ADDED_TO_STAGE, aoEntrar);
            }
        }

        private function aoEntrar(e:Event):void {
            removeEventListener(Event.ADDED_TO_STAGE, aoEntrar);
            comecar();
        }

        private function comecar():void {
            try {
                if (stage) {
                    stage.align = StageAlign.TOP_LEFT;
                    stage.scaleMode = StageScaleMode.SHOW_ALL;
                }
            } catch (erroDePalco:Error) {
            }

            // Uma faixa clara no topo: prova que o codigo desenhou, mesmo que
            // nenhuma letra apareca.
            graphics.beginFill(0xFFAD33);
            graphics.drawRect(0, 0, 800, 10);
            graphics.endFill();

            // 1. UM QUADRADO QUE GIRA
            quadrado = new Sprite();
            quadrado.graphics.beginFill(0xFFAD33);
            quadrado.graphics.drawRect(-60, -60, 120, 120);
            quadrado.graphics.endFill();
            quadrado.x = 170;
            quadrado.y = 250;
            addChild(quadrado);

            // 2. UMA BOLA QUE VAI E VOLTA
            bola = new Sprite();
            bola.graphics.beginFill(0x5EC8F2);
            bola.graphics.drawCircle(0, 0, 34);
            bola.graphics.endFill();
            bola.x = 400;
            bola.y = 250;
            addChild(bola);

            // 3. UM BOTAO QUE RESPONDE AO TOQUE
            var b:Sprite = new Sprite();
            b.graphics.beginFill(0xFFAD33);
            b.graphics.drawRoundRect(0, 0, 250, 90, 18, 18);
            b.graphics.endFill();
            b.x = 500;
            b.y = 200;
            b.buttonMode = true;
            b.addEventListener(MouseEvent.CLICK, aoTocar);
            addChild(b);

            // O marcador cresce a cada toque: a prova visual de que o toque
            // chegou, sem depender de uma letra sequer.
            marcador = new Sprite();
            marcador.x = 500;
            marcador.y = 330;
            addChild(marcador);
            desenharMarcador();

            // 4. AS LEGENDAS, POR ULTIMO E PROTEGIDAS
            try {
                addChild(texto("Sample Flash file", 30, 0xFFAD33, 40, 40));
                addChild(texto("Provided for App Store review. No third-party content.",
                               17, 0xC6CEDF, 40, 86));
                addChild(texto("Tap the orange button", 17, 0x26324F, 520, 230));
                contador = texto("Taps: 0", 20, 0xFFFFFF, 500, 390);
                addChild(contador);
            } catch (erroDeTexto:Error) {
            }

            addEventListener(Event.ENTER_FRAME, aoQuadro);
        }

        /** Um tracinho por toque, ate dez. Nao usa fonte nenhuma. */
        private function desenharMarcador():void {
            marcador.graphics.clear();
            marcador.graphics.beginFill(0xFFFFFF);
            for (var i:int = 0; i < toques && i < 10; i++) {
                marcador.graphics.drawRect(i * 24, 0, 16, 34);
            }
            marcador.graphics.endFill();
        }

        private function texto(oque:String, tamanho:Number, cor:uint,
                               x:Number, y:Number):TextField {
            var t:TextField = new TextField();
            t.autoSize = TextFieldAutoSize.LEFT;
            t.selectable = false;
            t.mouseEnabled = false;
            var f:TextFormat = new TextFormat();
            f.size = tamanho;
            f.color = cor;
            f.bold = true;
            t.defaultTextFormat = f;
            t.text = oque;
            t.x = x;
            t.y = y;
            return t;
        }

        private function aoTocar(e:MouseEvent):void {
            toques++;
            desenharMarcador();
            try {
                if (contador) {
                    contador.text = "Taps: " + toques;
                }
            } catch (erro:Error) {
            }
        }

        private function aoQuadro(e:Event):void {
            quadrado.rotation += 3;

            bola.x += passo;
            if (bola.x > 440) {
                bola.x = 440;
                passo = -passo;
            } else if (bola.x < 300) {
                bola.x = 300;
                passo = -passo;
            }
        }
    }
}
