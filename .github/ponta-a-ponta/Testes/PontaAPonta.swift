import XCTest

/// O CAMINHO DO JOGADOR, DE PONTA A PONTA.
///
/// Com o CF Client fechado: Safari no site, login, servidor, "Entrar no jogo",
/// "Abrir" na pergunta do iPhone — e o jogo tem que comecar a abrir. Quem
/// confere o fim e o workflow, pelo registro do aplicativo e pelas fotos.
///
/// Conta, senha e o id do aplicativo chegam por ambiente (TEST_RUNNER_*). A
/// senha nunca e escrita em lugar nenhum.
final class PontaAPonta: XCTestCase {
    let servidor = "Lugares escuros"

    func testEntrarPeloSite() throws {
        continueAfterFailure = false
        let env = ProcessInfo.processInfo.environment
        let conta = try XCTUnwrap(env["CONTA"], "falta CONTA")
        let senha = try XCTUnwrap(env["SENHA"], "falta SENHA")
        let appId = try XCTUnwrap(env["APP"], "falta APP")

        let app = XCUIApplication(bundleIdentifier: appId)
        app.terminate()
        let safari = XCUIApplication(bundleIdentifier: "com.apple.mobilesafari")

        passo("abrindo o site")
        safari.open(URL(string: "https://deathnotestore.com.br/jogar/")!)
        fecharSeHouver(safari, ["Continue", "Continuar", "Not Now", "Agora Não"])

        let campoConta = safari.webViews.textFields.firstMatch
        XCTAssert(campoConta.waitForExistence(timeout: 60), "o formulario do site nao apareceu")
        focar(campoConta, safari)
        // O site lembra a ultima conta; apaga antes de digitar.
        if let atual = campoConta.value as? String, !atual.isEmpty {
            campoConta.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: atual.count))
        }
        campoConta.typeText(conta)

        let campoSenha = safari.webViews.secureTextFields.firstMatch
        XCTAssert(campoSenha.waitForExistence(timeout: 10), "o campo de senha nao apareceu")
        focar(campoSenha, safari)
        campoSenha.typeText(senha)
        passo("conta e senha digitadas")

        escolherServidor(safari)
        passo("servidor escolhido")

        fecharDicas(safari)
        safari.webViews.buttons["ENTRAR NO JOGO"].firstMatch.tap()
        passo("formulario enviado")
        fecharSeHouver(safari, ["Not Now", "Agora Não", "Never for This Website"])

        let botaoDoApp = safari.webViews.links["ENTRAR NO JOGO"].firstMatch
        XCTAssert(botaoDoApp.waitForExistence(timeout: 60), "o site nao mostrou o botao do aplicativo")
        passo("botao do aplicativo na tela")
        botaoDoApp.tap()

        // "Abrir no CF Client?" — a pergunta que o jogador responde.
        let abrir = primeiroQueExiste([safari, XCUIApplication(bundleIdentifier: "com.apple.springboard")],
                                      ["Open", "Abrir"], espera: 20)
        XCTAssertNotNil(abrir, "a pergunta de abrir o aplicativo nao apareceu")
        passo("pergunta de abrir na tela")
        abrir?.tap()

        XCTAssert(app.wait(for: .runningForeground, timeout: 30), "o CF Client nao abriu")
        passo("CF Client na frente")

        // Da tempo do jogo carregar; o workflow fotografa e confere o registro.
        for segundos in stride(from: 15, through: 90, by: 15) {
            sleep(15)
            XCTAssert(app.state == .runningForeground, "o CF Client saiu da frente aos \(segundos)s")
            passo("\(segundos)s depois de abrir")
        }
    }

    private func escolherServidor(_ safari: XCUIApplication) {
        let caixa = safari.webViews.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS 'S(' OR value CONTAINS 'S('")).firstMatch
        guard caixa.waitForExistence(timeout: 10) else {
            XCTFail("a caixa de servidor nao apareceu")
            return
        }
        if ((caixa.value as? String) ?? caixa.label).contains(servidor) { return }
        fecharDicas(safari)
        caixa.tap()

        // iPhone mostra a roleta; versoes novas mostram um menu.
        let roleta = safari.pickerWheels.firstMatch
        if roleta.waitForExistence(timeout: 3) {
            roleta.adjust(toPickerWheelValue: "S(132-394):\(servidor)")
            fecharSeHouver(safari, ["Done", "OK", "Concluído"])
            return
        }
        let item = safari.descendants(matching: .any)
            .matching(NSPredicate(format: "label CONTAINS %@", servidor)).firstMatch
        XCTAssert(item.waitForExistence(timeout: 5), "o servidor nao apareceu na lista")
        item.tap()
    }

    /// TOCA NO CAMPO ATE ELE PEGAR O TECLADO.
    ///
    /// Na 1a rodada (28/10) uma dica do Safari ("View Bookmarks, Share
    /// Menu...") abriu em cima do campo de senha e o toque caiu nela.
    /// Fecha as dicas e confere o foco antes de digitar.
    private func focar(_ campo: XCUIElement, _ safari: XCUIApplication) {
        for _ in 0..<5 {
            fecharDicas(safari)
            campo.tap()
            usleep(500_000)
            if (campo.value(forKey: "hasKeyboardFocus") as? Bool) == true { return }
        }
        passo("campo sem foco")
        print("ARVORE:\n\(safari.debugDescription)")
        XCTFail("o campo nao pegou o teclado")
    }

    private func fecharDicas(_ safari: XCUIApplication) {
        for nome in ["Close", "Fechar", "Dismiss"] {
            let b = safari.buttons.matching(identifier: nome).allElementsBoundByIndex
                .first { $0.exists && $0.isHittable }
            b?.tap()
        }
    }

    private func fecharSeHouver(_ app: XCUIApplication, _ nomes: [String]) {
        if let b = primeiroQueExiste([app], nomes, espera: 2) { b.tap() }
    }

    private func primeiroQueExiste(_ apps: [XCUIApplication], _ nomes: [String], espera: TimeInterval) -> XCUIElement? {
        let fim = Date().addingTimeInterval(espera)
        repeat {
            for a in apps {
                for n in nomes where a.buttons[n].exists { return a.buttons[n] }
            }
            usleep(300_000)
        } while Date() < fim
        return nil
    }

    private func passo(_ texto: String) {
        print("PASSO: \(texto)")
        let foto = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        foto.name = texto
        foto.lifetime = .keepAlways
        add(foto)
    }
}
