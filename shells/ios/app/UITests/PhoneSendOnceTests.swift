// Send exactly one message: clear the draft, type, keyboard down, double-tap the chat button; if nothing was sent
// (the draft card is still there), reopen and tap the composer's run button (→).
import XCTest
final class PhoneSendOnceTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    var app: XCUIApplication!
    func shot(_ name: String) { let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a) }
    func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)) }
    func testSendOnce() throws {
        guard let task = env["OPEN_TASK"], let msg = env["MESSAGE"] else { throw XCTSkip("OPEN_TASK/MESSAGE") }
        app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = task
        app.launch(); sleep(10)
        let f = app.windows.firstMatch.frame
        let chat = at(f.width / 2, f.height - 77)
        chat.tap(); sleep(2)
        app.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 200))
        app.typeText(msg); sleep(1)
        at(f.width / 2, f.height * 0.25).tap(); sleep(2); shot("15-draft-card")
        chat.doubleTap(); sleep(3); shot("16-after-double-tap")
        if env["FALLBACK"] == "1" {
            chat.tap(); sleep(2)
            at(f.width * 0.86, f.height * 0.445).tap(); sleep(2); shot("17-after-run-button")
        }
        sleep(20); shot("18-reply")
    }
}
