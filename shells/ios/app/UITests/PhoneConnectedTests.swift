// The phone layout connected to a workspace, on a device: gestures in an order where a microphone permission
// prompt (the user's to answer) can't block the rest; the one message is sent by double-tap.
// Env (TEST_RUNNER_*): OPEN_TASK (opened by the app's debug hook), MESSAGE (sent by double-tap; empty = skip).
import XCTest
final class PhoneConnectedTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    var app: XCUIApplication!
    func shot(_ name: String) { let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a) }
    func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)) }
    func testConnectedGestures() throws {
        XCUIDevice.shared.orientation = .portrait
        app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        if let t = env["OPEN_TASK"] { app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = t }
        app.launch()
        sleep(10)
        let f = app.windows.firstMatch.frame
        let chat = at(f.width / 2, f.height - 77)
        let away = at(f.width / 2, f.height * 0.3)
        shot("01-opened")
        if env["SKIP_GESTURES"] != "1" {
        chat.tap(); sleep(2); shot("02-tap-chat")
        away.tap(); sleep(1)
        at(f.width / 2 + 70, f.height * 0.09).press(forDuration: 0.02, thenDragTo: at(30, f.height * 0.09), withVelocity: .fast, thenHoldForDuration: 0); sleep(3); shot("03-title-swipe")
        at(f.width - 30, f.height * 0.09).press(forDuration: 0.02, thenDragTo: at(f.width - 100, f.height * 0.09), withVelocity: .fast, thenHoldForDuration: 0) // swipe back toward the first task
        sleep(2)
        at(3, f.height * 0.5).press(forDuration: 0.02, thenDragTo: at(f.width * 0.7, f.height * 0.5), withVelocity: .fast, thenHoldForDuration: 0); sleep(2); shot("04-edge-left")
        at(f.width - 3, f.height * 0.5).press(forDuration: 0.02, thenDragTo: at(f.width * 0.3, f.height * 0.5), withVelocity: .fast, thenHoldForDuration: 0); sleep(2); shot("05-edge-right")
        }
        if let t = env["OPEN_TASK"], env["SKIP_GESTURES"] != "1" { app.terminate(); app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = t; app.launch(); sleep(10) } // back on the task
        if let msg = env["MESSAGE"], !msg.isEmpty {
            chat.tap(); sleep(2)
            // the card focuses the composer but doesn't raise the keyboard (crate fix requested): touch the field
            at(f.width / 2, f.height * 0.787).tap(); sleep(2)
            shot("06a-after-field-tap"); print("PROBE keyboards: \(app.keyboards.count)")
            app.typeText(msg); sleep(1); shot("06-draft")
            chat.doubleTap(); sleep(25); shot("07-double-tap-sent-reply")
        }
        chat.press(forDuration: 0.7); sleep(2); shot("08-long-press")
        away.tap(); sleep(1)
        chat.press(forDuration: 1.5); sleep(2); shot("09-hold-to-talk")
    }
}
