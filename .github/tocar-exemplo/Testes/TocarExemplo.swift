import XCTest

/// O QUE O REVISOR DA APPLE FEZ (16/11, build 53, iPad Air 11" M3).
///
/// Instalou, abriu, tocou em "Exemplo" — e o app fechou. Abriu de novo,
/// tocou de novo, fechou de novo. O "abrir sem link" e o "ponta a ponta"
/// nunca tocam no Exemplo, por isso nao pegaram.
///
/// Este teste repete as duas aberturas. Em cada uma: toca em "Exemplo" e o
/// CF Client tem que continuar na frente por 30 segundos.
final class TocarExemplo: XCTestCase {
    func testTocarNoExemplo() throws {
        continueAfterFailure = false
        let appId = try XCTUnwrap(ProcessInfo.processInfo.environment["APP"], "falta APP")
        let app = XCUIApplication(bundleIdentifier: appId)

        for vez in 1...2 {
            app.terminate()
            app.activate()
            XCTAssert(app.wait(for: .runningForeground, timeout: 30), "abertura \(vez): o CF Client nao abriu")

            let exemplo = app.staticTexts["Exemplo"].firstMatch
            XCTAssert(exemplo.waitForExistence(timeout: 30), "abertura \(vez): o Exemplo nao apareceu na lista")
            passo("abertura \(vez): lista com o Exemplo")
            exemplo.tap()
            passo("abertura \(vez): tocou no Exemplo")

            for segundos in stride(from: 5, through: 30, by: 5) {
                sleep(5)
                XCTAssert(app.state == .runningForeground,
                          "abertura \(vez): o CF Client fechou \(segundos)s depois do toque no Exemplo")
            }
            passo("abertura \(vez): 30s tocando o Exemplo")

            // Girar nao pode derrubar, e o conteudo tem que ocupar a tela nas
            // duas posicoes (a build 54 abria numa janelinha em pe no iPad).
            for (nome, posicao) in [("deitado", UIDeviceOrientation.landscapeLeft),
                                    ("em pe", UIDeviceOrientation.portrait)] {
                XCUIDevice.shared.orientation = posicao
                sleep(5)
                XCTAssert(app.state == .runningForeground,
                          "abertura \(vez): o CF Client fechou ao girar \(nome)")
                passo("abertura \(vez): \(nome)")
            }
        }
    }

    private func passo(_ texto: String) {
        print("PASSO: \(texto)")
        let foto = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        foto.name = texto
        foto.lifetime = .keepAlways
        add(foto)
    }
}
