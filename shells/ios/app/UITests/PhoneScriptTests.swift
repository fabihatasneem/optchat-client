// A scripted tap walk on a device, a screenshot after each step (for flows GPUI exposes no accessibility tree for).
// Env (TEST_RUNNER_ prefix): RESET=1 (start from no prefs), SEED_OC1 (base64 oc1 engine token → Add engine),
// OPEN_TASK, WAIT (s after launch), STEPS="name:x,y[,hold];…" — x, y: fractions of the window (y < 0: points from the
// bottom); hold: seconds of press; a step "name:close" taps the scanner's native close button, "name:wait" just waits 3 s.
import XCTest
final class PhoneScriptTests: XCTestCase {
    let env = ProcessInfo.processInfo.environment
    var app: XCUIApplication!
    func shot(_ name: String) { let a = XCTAttachment(screenshot: XCUIScreen.main.screenshot()); a.name = name; a.lifetime = .keepAlways; add(a) }
    func testScript() throws {
        app = XCUIApplication(bundleIdentifier: "com.reframe.optchat")
        if env["RESET"] == "1" { app.launchEnvironment["OPTCHAT_RESET"] = "1" }
        if let t = env["SEED_OC1"], !t.isEmpty { app.launchEnvironment["OPTCHAT_SEED_OC1"] = t }
        if let t = env["OPEN_TASK"] { app.launchEnvironment["OPTCHAT_AUTOTEST_OPEN"] = t }
        app.launch(); sleep(UInt32(env["WAIT"] ?? "10") ?? 10)
        let f = app.windows.firstMatch.frame
        shot("00-launch")
        let sb = XCUIApplication(bundleIdentifier: "com.apple.springboard")
        for (i, step) in (env["STEPS"] ?? "").split(separator: ";").enumerated() {
            let parts = step.split(separator: ":", maxSplits: 1)
            guard parts.count == 2 else { continue }
            let name = String(format: "%02d-", i + 1) + parts[0]
            let spec = String(parts[1])
            if spec == "close" {
                let b = app.buttons["qr-close"]
                if b.waitForExistence(timeout: 3) { b.tap() } else { print("PROBE no qr-close") }
            } else if spec != "wait" {
                let v = spec.split(separator: ",").compactMap { Double($0) }
                guard v.count >= 2 else { continue }
                let y = v[1] < 0 ? Double(f.height) + v[1] : v[1] * Double(f.height)
                let c = app.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: v[0] * Double(f.width), dy: y))
                if v.count > 2 { c.press(forDuration: v[2]) } else { c.tap() }
            }
            sleep(3)
            shot(name)
            print("PROBE \(name) scanner=\(app.otherElements["qr-scanner"].exists) alert=\(sb.alerts.firstMatch.exists || app.alerts.firstMatch.exists)")
            if sb.alerts.firstMatch.exists || app.alerts.firstMatch.exists { print("PROBE system prompt shown: stopping, nothing tapped"); return }
        }
    }
}
