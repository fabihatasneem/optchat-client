// Keyboard-up gestures on a device (optchat-ui ea6df6f): MODE=measure (type + screenshot, then clear, nothing sent),
// MODE=run: double-tap send (MESSAGE) with the keyboard up; long-press with the keyboard up (cancelled quickly);
// hold-to-talk with the keyboard up (short; the dictated text is cleared); → run (MESSAGE2) at RUN_X/RUN_Y (fractions).
import XCTest
final class PhoneKeyboardUpTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    var app: XCUIApplication!
    func shot(_ name: String) { let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a) }
    func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)) }
    func clearDraft() { app.typeText(String(repeating: XCUIKeyboardKey.delete.rawValue, count: 200)) }
    func testKeyboardUp() throws {
        guard let task = env["OPEN_TASK"] else { throw XCTSkip("OPEN_TASK") }
        app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = task
        app.launch(); sleep(10)
        let f = app.windows.firstMatch.frame
        let chat = at(f.width / 2, f.height - 77)
        let feed = at(f.width / 2, f.height * 0.25)
        chat.tap(); sleep(2)
        if env["MODE"] == "measure" {
            clearDraft(); app.typeText("measure"); sleep(1); shot("20-measure"); clearDraft(); return
        }
        // 0. Paste (the device's own clipboard), long-press in the field → the
        //    Paste pill above the touch → tap it; then clear (nothing is sent)
        if env["PASTE"] == "1" {
            clearDraft(); sleep(1)
            let fieldY = f.height * 0.393
            at(f.width * 0.4, fieldY).press(forDuration: 0.8); sleep(1); shot("27-paste-pill")
            at(f.width * 0.4, fieldY - 44).tap(); sleep(2); shot("28-after-paste-tap")
            // the iOS paste prompt ("would like to paste from …") is the user's to answer: stop here if it shows
            let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            if app.alerts.firstMatch.waitForExistence(timeout: 2) || sb.alerts.firstMatch.waitForExistence(timeout: 1) {
                print("PROBE paste prompt shown: stopping, nothing tapped"); shot("29-paste-prompt"); return
            }
            print("PROBE no paste prompt")
            if app.keyboards.count == 0 { chat.tap(); sleep(2) }
            clearDraft(); sleep(1)
        }
        // 1. double-tap send with the keyboard up
        clearDraft(); app.typeText(env["MESSAGE"] ?? ""); sleep(1); shot("21-typed")
        let chatUp = at(f.width / 2, f.height * 0.537) // the bar on the keyboard (measured; the keyboard frame excludes its suggestion bar)
        chatUp.doubleTap(); sleep(20); shot("22-double-tap-sent")
        // 2. long-press with the keyboard up: dismisses it and dictates into the floating card; cancel at once
        if app.keyboards.count == 0 { chat.tap(); sleep(2) }
        let chatUp2 = app.keyboards.count > 0 ? at(f.width / 2, f.height * 0.537) : chat
        chatUp2.press(forDuration: 0.6); sleep(1); shot("23-long-press-up")
        chat.tap(); sleep(1); shot("24-after-cancel") // ✕ in the chat button's place
        chat.tap(); sleep(3); shot("24b-after-second-x") // a second ✕ always ends it (Finishing… meanwhile)
        if env["SKIP_HOLD"] == "1" { feed.tap(); sleep(1); return }
        // 3. hold-to-talk with the keyboard up (short), then clear whatever it heard
        chat.tap(); sleep(2)
        let chatUp3 = app.keyboards.count > 0 ? at(f.width / 2, f.height * 0.537) : chat
        chatUp3.press(forDuration: 1.3); sleep(2); shot("25-hold-released")
        chat.tap(); sleep(2); clearDraft(); sleep(1)
        // 4. → run with a typed message
        if let m2 = env["MESSAGE2"], let rx = Double(env["RUN_X"] ?? ""), let ry = Double(env["RUN_Y"] ?? "") {
            app.typeText(m2); sleep(1)
            at(f.width * rx, f.height * ry).tap(); sleep(20); shot("26-run-button-sent")
        }
        feed.tap(); sleep(1)
    }
}
