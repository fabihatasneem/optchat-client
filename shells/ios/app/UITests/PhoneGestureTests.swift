// The phone layout's gestures with real touches (optchat-ui phone.rs): tap, long-press (450 ms: dictation), hold
// (1.2 s: push-to-talk), double-tap, edge swipes, title swipe, tab taps. Points from the window's corners (GPUI has
// no accessibility tree). SHOTS (host dir, simulator only). Nothing is sent: the draft stays empty.
import XCTest
final class PhoneGestureTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    var app: XCUIApplication!
    func shot(_ name: String) {
        let s = XCUIScreen.main.screenshot()
        let a = XCTAttachment(screenshot: s); a.name = name; a.lifetime = .keepAlways; add(a)
        if let d = env["SHOTS"], !d.isEmpty { try? s.pngRepresentation.write(to: URL(fileURLWithPath: d).appendingPathComponent("phone-\(name).png")) }
    }
    func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)) }
    func testPhoneGestures() throws {
        XCUIDevice.shared.orientation = .portrait
        app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        app.launch()
        sleep(8)
        let f = app.windows.firstMatch.frame
        let chat = at(f.width / 2, f.height - 77)   // the bottom bar's chat button
        shot("01-start")
        chat.tap(); sleep(2); shot("02-tap-chat")
        at(f.width / 2, f.height * 0.3).tap(); sleep(1) // outside: closes the draft card
        chat.press(forDuration: 0.6); sleep(2); shot("03-long-press-dictation")
        at(f.width / 2, f.height * 0.3).tap(); sleep(1); shot("04-after-outside-tap")
        chat.press(forDuration: 1.4); sleep(1); shot("05-hold-to-talk-released")
        chat.doubleTap(); sleep(2); shot("06-double-tap")
        at(f.width / 2, f.height * 0.3).tap(); sleep(1)
        at(3, f.height * 0.5).press(forDuration: 0.02, thenDragTo: at(f.width * 0.7, f.height * 0.5), withVelocity: .fast, thenHoldForDuration: 0); sleep(2); shot("07-edge-swipe-left")
        at(f.width - 3, f.height * 0.5).press(forDuration: 0.02, thenDragTo: at(f.width * 0.3, f.height * 0.5), withVelocity: .fast, thenHoldForDuration: 0); sleep(2); shot("08-edge-swipe-right")
        at(f.width / 2 + 70, f.height * 0.09).press(forDuration: 0.02, thenDragTo: at(30, f.height * 0.09), withVelocity: .fast, thenHoldForDuration: 0); sleep(3); shot("09-title-swipe")
        at(f.width * 0.115, f.height - 70).tap(); sleep(2); shot("10-tab-tasks")
        at(f.width * 0.885, f.height - 70).tap(); sleep(2); shot("11-tab-more")
    }
}
