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
     * Mandar conteudo de terceiro esta fora de questao — foi justamente isso
     * que causou a recusa 5.2.3 la atras. Entao este arquivo e nosso, escrito
     * do zero, e mostra as tres coisas que um tocador precisa provar:
     *
     *   1. DESENHA        o quadrado gira e a bola anda
     *   2. ANIMA          tudo isso quadro a quadro, pelo ENTER_FRAME
     *   3. RECEBE TOQUE   o botao conta os toques e muda o texto
     *
     * Em ingles porque quem vai ler e o revisor.
     */
    [SWF(width="800", height="480", frameRate="30", backgroundColor="#1B2440")]
    public class Exemplo extends Sprite {

        private var quadrado:Sprite;
        private var bola:Sprite;
        private var contador:TextField;
        private var toques:int = 0;
        private var passo:Number = 4;

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
            if (stage) {
                stage.align = StageAlign.TOP_LEFT;
                stage.scaleMode = StageScaleMode.SHOW_ALL;
            }

            graphics.beginFill(0x1B2440);
            graphics.drawRect(0, 0, 800, 480);
            graphics.endFill();

            addChild(texto("Sample Flash file", 28, 0xFFAD33, 40, 40));
            addChild(texto("Provided for App Store review. Nothing here is third-party content.",
                           16, 0xC6CEDF, 40, 84));

            quadrado = new Sprite();
            quadrado.graphics.beginFill(0xFFAD33);
            quadrado.graphics.drawRect(-45, -45, 90, 90);
            quadrado.graphics.endFill();
            quadrado.x = 200;
            quadrado.y = 260;
            addChild(quadrado);

            bola = new Sprite();
            bola.graphics.beginFill(0x5EC8F2);
            bola.graphics.drawCircle(0, 0, 26);
            bola.graphics.endFill();
            bola.x = 420;
            bola.y = 260;
            addChild(bola);

            addChild(botao());

            contador = texto("Taps: 0", 20, 0xFFFFFF, 560, 330);
            addChild(contador);

            addEventListener(Event.ENTER_FRAME, aoQuadro);
        }

        private function texto(oque:String, tamanho:Number, cor:uint,
                               x:Number, y:Number):TextField {
            var t:TextField = new TextField();
            t.autoSize = TextFieldAutoSize.LEFT;
            t.selectable = false;
            t.mouseEnabled = false;
            t.defaultTextFormat = new TextFormat("_sans", tamanho, cor, true);
            t.text = oque;
            t.x = x;
            t.y = y;
            return t;
        }

        /** O botao: e ele que prova que o toque chega ao conteudo. */
        private function botao():Sprite {
            var b:Sprite = new Sprite();
            b.graphics.beginFill(0xFFAD33);
            b.graphics.drawRoundRect(0, 0, 240, 64, 16, 16);
            b.graphics.endFill();
            b.x = 560;
            b.y = 240;
            b.buttonMode = true;
            b.useHandCursor = true;

            var r:TextField = texto("Tap to test input", 18, 0x26324F, 0, 0);
            r.x = (240 - r.width) / 2;
            r.y = (64 - r.height) / 2;
            b.addChild(r);

            b.addEventListener(MouseEvent.CLICK, aoTocar);
            return b;
        }

        private function aoTocar(e:MouseEvent):void {
            toques++;
            contador.text = "Taps: " + toques;
        }

        private function aoQuadro(e:Event):void {
            quadrado.rotation += 2;

            bola.x += passo;
            if (bola.x > 500) {
                bola.x = 500;
                passo = -passo;
            } else if (bola.x < 360) {
                bola.x = 360;
                passo = -passo;
            }
        }
    }
}
