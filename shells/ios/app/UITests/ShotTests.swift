// Device/simulator screenshots of the running app without relaunching it (keeps its state): activate + attach.
import XCTest
final class ShotTests: XCTestCase {
    func testScreenshot() {
        let app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        app.activate()
        sleep(3)
        let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot()); a.name = "optchat"; a.lifetime = .keepAlways; add(a)
    }
}
