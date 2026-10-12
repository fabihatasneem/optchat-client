// The end-to-end check on iPad, driven by touch (GPUI exposes no accessibility tree: coordinates in points from the
// window's corners). The connection is already seeded (prefs + Keychain). OPTCHAT_AUTOTEST_OPEN opens the task;
// then: scroll the long chat by flicks, tap the composer, type and send (return), wait for the reply, switch tasks.
// Env (TEST_RUNNER_*): OPEN_TASK, MESSAGE (empty = don't send), SHOTS (host dir, simulator only).
import XCTest
final class InteractTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    func shot(_ name: String) {
        let s = XCUIScreen.main.screenshot()
        let a = XCTAttachment(screenshot: s); a.name = name; a.lifetime = .keepAlways; add(a)
        if let d = env["SHOTS"], !d.isEmpty { try? s.pngRepresentation.write(to: URL(fileURLWithPath: d).appendingPathComponent("e2e-\(name).png")) }
    }
    func testIPadFlow() throws {
        guard let task = env["OPEN_TASK"] else { throw XCTSkip("no OPEN_TASK") }
        XCUIDevice.shared.orientation = .landscapeLeft
        let app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = task
        app.launch()
        sleep(9)
        shot("1-opened")
        let f = app.windows.firstMatch.frame
        let o = app.coordinate(withNormalizedOffset: .zero)
        func at(_ x: CGFloat, _ y: CGFloat) -> XCUICoordinate { o.withOffset(CGVector(dx: x, dy: y)) }
        // scroll the long chat: flick up twice (back in history), then down again
        let feedX = f.width * 0.62
        for _ in 0..<2 { at(feedX, f.height * 0.30).press(forDuration: 0.02, thenDragTo: at(feedX, f.height * 0.75), withVelocity: .fast, thenHoldForDuration: 0) ; usleep(400_000) }
        sleep(1); shot("2-scrolled-up")
        for _ in 0..<3 { at(feedX, f.height * 0.75).press(forDuration: 0.02, thenDragTo: at(feedX, f.height * 0.25), withVelocity: .fast, thenHoldForDuration: 0); usleep(400_000) }
        sleep(1)
        if let msg = env["MESSAGE"], !msg.isEmpty {
            at(feedX, f.height - 118).tap() // the composer
            sleep(2)
            app.typeText(msg)
            app.typeText("\n") // Enter = run
            sleep(20)
            shot("3-reply")
            // dismiss the keyboard: a tap on the feed
            at(feedX, f.height * 0.3).tap(); sleep(1)
        }
        at(110, 80).tap() // the sidebar's first row: main chat
        sleep(4)
        shot("4-switched")
    }
}
