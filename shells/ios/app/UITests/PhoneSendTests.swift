// One message from the phone layout on a device, with the composer's run button (→): open the task (debug hook),
// open the draft card, clear any leftover draft, type, tap →, wait for the reply. Then, keyboard down, long-press
// (dictation) and hold-to-talk. Env: OPEN_TASK, MESSAGE.
import XCTest
final class PhoneSendTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    var app: XCUIApplication!
    func shot(_ name: String) { let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a) }
    func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)) }
    func testSendAndDictation() throws {
        guard let task = env["OPEN_TASK"], let msg = env["MESSAGE"] else { throw XCTSkip("OPEN_TASK/MESSAGE") }
        XCUIDevice.shared.orientation = .portrait
        app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = task
        app.launch(); sleep(10)
        let f = app.windows.firstMatch.frame
        let chat = at(f.width / 2, f.height - 77)
        chat.tap(); sleep(2)
        XCTAssertTrue(app.keyboards.count > 0, "keyboard")
        app.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 80)) // a leftover draft
        app.typeText(msg); sleep(1); shot("10-typed")
        // the run button (→) at the composer's right edge, just above the keyboard
        let kbTop = app.keyboards.firstMatch.frame.minY
        at(f.width - 34, kbTop - 130).tap()
        sleep(25); shot("11-sent-reply")
        at(f.width / 2, f.height * 0.25).tap(); sleep(2) // the feed: dismisses the keyboard
        shot("12-keyboard-down")
        chat.press(forDuration: 0.8); sleep(2); shot("13-long-press")
        at(f.width / 2, f.height * 0.25).tap(); sleep(2)
        chat.press(forDuration: 1.6); sleep(2); shot("14-hold-to-talk-released")
    }
}
