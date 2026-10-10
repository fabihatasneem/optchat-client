# OptChat for iPhone and iPad: plan

Status: 2026-10-10. Prototype in this repo, verified in the iOS 27 Simulator (iPhone 18 Pro, iPad Pro 11-inch M5)
and against the hub. Screenshots are in `docs/shots/` (§11).

## 1. Recommendation

> **Superseded by §12 (2026-10-10).** The user asked for the decision to rest on measured performance and efficiency
> only. The bake-off in §12 favours **GPUI on iOS provisionally**, to be confirmed by a device run with a pre-set
> decision rule. The text below is the first evaluation, which also weighed code sharing and effort.

**A native SwiftUI app, with UIKit where it matters (the chat feed and the composer), and its own small Swift
protocol layer (`OptChatKit`).** The wire contract is shared with the desktop client through golden JSON fixtures
run by both test suites, not through shared code.

Why:

- **Fidelity and speed.** Long-chat scrolling at 120 Hz and native text selection, the keyboard, dictation, haptics
  and accessibility all come from UIKit/SwiftUI directly. The protocol layer is small. In the prototype,
  `OptChat/Core` is about 450 lines and already does snapshot, events, rpc, reconnect, trust and Keychain.
- **What a shared core would actually share is small.** The GPUI client's reusable part is `crates/proto`: wire
  types plus pure helpers (metrics, rates, trim), 2.3k lines. Its connection code (`crates/app/src/client.rs`) and its
  state (`crates/app/src/state.rs`) live in the app crate and are tied to tokio and to GPUI entities. Sharing them
  would first mean pulling a GUI-free `client-core` crate out of the desktop app. Then UniFFI bindings, an
  xcframework with three slices, and async/threading across the FFI on every event. The browser already keeps its
  own reducer (`app/ui/state.ts`, 160 lines), so OptChat already lives with one reducer per client. A fixture suite
  keeps them honest more cheaply than an FFI layer.
- **iOS networking should be the system's.** `URLSessionWebSocketTask` and Network.framework handle cellular
  bring-up, VPN-on-demand, proxies, path changes and ATS. Apple documents that plain BSD sockets, which is what a
  Rust tokio/tungstenite stack uses, don't get these. So even a Rust core would want Swift to own the socket.
- **GPUI on iOS exists and works, but not for us yet** (full findings in §1a). Zedra ships a GPUI app on the App
  Store from its own Zed fork, and upstream has an unmerged `gpui_ios` PR that builds and runs in the simulator.
  But the desktop pins crates.io gpui 0.2.2 and gpui-component 0.5.0. Sharing its UI with iOS would first mean
  moving the desktop to current GPUI (603 commits on `crates/gpui` since 0.2.2, a split into platform crates, API
  changes; gpui-component is at 0.7.1 on `gpui-pre` 0.3.8), then onto an unmerged backend. That backend exposes
  next to no accessibility tree and leaves keyboard avoidance and edit menus to the app.
- **A WKWebView wrapper of the web page is the fastest path, but a weaker app.** The page already has a phone
  layout and dictation over getUserMedia. The drawbacks:
  - The JS is suspended in the background, and every return reloads or resnapshots in a web view.
  - Viewport and keyboard quirks.
  - No native list virtualization: 120 Hz with markdown-heavy feeds is not reliable.
  - The page speaks only today's same-origin `/ws`, not `/engine/ws` with tokens.

  It stays a fallback: a "web view" tab could ship in a day if needed.
- **React Native.** It could share `state.ts` and the TS types with the web page, but it adds a third UI stack
  (Hermes, Metro, native modules for audio and Keychain) next to GPUI and the web. It shares nothing with Rust, and
  its keyboard and composer quality depends on community modules. Not worth the maintenance.

| option | fidelity / 120 Hz | sharing with GPUI | composer + dictation | background | effort | maintenance |
|---|---|---|---|---|---|---|
| **SwiftUI + UIKit, Swift protocol** | best | fixtures only | native, best | system sockets, APNs | M | low (one Swift app) |
| SwiftUI over a Rust core (UniFFI) | best (UI is Swift) | proto + reducer, after a desktop refactor | native | Rust sockets lose iOS network behaviour, or Swift owns the socket anyway | M–L | FFI + xcframework build in CI |
| GPUI on iOS (zed#63068 / Zedra fork) | runs; fling present; no keyboard avoidance | full UI, after a desktop migration off 0.2.2 | typing, accents, IME marked text work; dictation and edit menus partial | to write | L+ (desktop migration + unmerged backend) | track an unmerged backend; near-empty accessibility tree |
| WKWebView of the web page | OK at best | none (shares with web) | web-grade | JS suspended, reload | S | low, but web limits stay |
| React Native | good with FlashList | with web (TS) only | modules | modules | L | third UI stack |

## 1a. GPUI on iOS: what exists (searched 2026-10-10) and what was tried

Projects found on GitHub (searches: gpui ios, gpui_ios, gpui-mobile, gpui uikit, forks of zed with `crates/gpui_ios`):

| project | what | state | license |
|---|---|---|---|
| **deltaqdev/zedra** (240★) | Mobile remote for AI coding agents; GPUI UI. **On the App Store and Google Play.** | Uses `vendor/zed` = tanlethanh/zed `feat/gpui-mobile`: `crates/gpui_ios` is about 428 KB of source (UITextInput with marked text, `insertDictationResult`, keyboard accessory, touch fling), plus gpui_android and gpui_wgpu. The fork is 115 commits ahead and **2,757 behind** zed main (merge base 2026-05-08). | MIT |
| **zed-industries/zed#63068** `gpui_ios` | The upstream version of that backend ("does not include Delta-specific UI", i.e. Zedra's). UIKit scene/window, CoreText, Metal, safe area and keyboard insets, native text input, Keychain, touch + direct-drag scrolling. | Open, merge state "blocked"; prerequisite #64990 merged 2026-09-30. Explicitly leaves out custom momentum, edit menus and keyboard accessories. | Apache-2.0 |
| itsbalamurali/gpui-mobile (413★) | iOS (wgpu/Metal) + Android platform crate; claims momentum scrolling and keyboard support; Flutter-like plugin features. | Pins zed rev `5688167` (2026-04-14). | repo: AGPL-3.0; Cargo: GPL/AGPL/Apache-2.0 |
| Glass-HQ/gpui (138★) | GPUI fork with an iOS host shell (`cargo gpui run ios`). | Last push 2026-05-16. | — |
| maddada/Ghostex (910★) | Agent workspace (GPUI desktop). **Its shipped mobile app is React Native (Expo)**. `apps/gpui-mobile-poc` is a proof of concept: the desktop's GPUI chat transcript renderer drawn inside an RN screen, with a native composer below, on a vendored gpui-mobile. | PoC | — |

**Built and run here:** zed#63068's branch (shallow clone in `vendor-probe/zed-gpui-ios`, not committed).
- The example build script only runs `cargo build`, copies the binary into an .app and ad-hoc signs it.
- Needed: zed's pinned toolchain (Rust 1.98.1, auto-installed by rustup), the `aarch64-apple-ios-sim` target, and
  `--features gpui_platform/runtime_shaders`. Without that feature the build needs Apple's separate Metal Toolchain
  download.
- First build ~5 min, target dir 2.1 GB.
- I replaced the example with a probe (`vendor-probe/gpui_ios_probe_example.rs`): GPUI's own `TextInput` (from
  `examples/input.rs`, with IME marked text) above a 2,000-row `uniform_list`.
- Results, from XCUITest on the iPhone 18 Pro simulator (`testGPUIProbe`, `testNativeFlickReference`):
  - Renders, at the right safe-area insets. Taps work: Send clears the input and the counter updates.
  - A tap on the input brings up the **system keyboard**. Typing "Hello wörld, café" works, including accents.
  - The list **does not move out of the keyboard's way**: content stays under it. That is the app's job, done by
    hand.
  - Fast flick: the list is still moving 0.15–0.75 s after release, so there is a fling. A native UIKit list
    (Settings) under the same flick is also still moving then. Flicks travelled about 10–22 rows. Fling quality
    against UIScrollView (deceleration curve, rubber-banding, scroll-to-top tap) still needs a device check.
  - **Accessibility:** with the keyboard down, XCUITest sees about 5 elements for the whole GPUI window. VoiceOver,
    Voice Control and UI tests get next to nothing; text is pixels to the system.
  - API drift seen right away: `flex_grow()` takes an argument upstream and none in 0.2.2.
  - Screenshots: `docs/shots/gpui-ios-probe.png`, `gpui-probe-strip.png` (launch, two flicks, typing).

**Judgement.** GPUI on iOS is real: Zedra proves it in production on its own fork, and the upstream PR runs.
- A shared GPUI UI would mean, in order:
  1. Migrate the desktop from gpui 0.2.2 / gpui-component 0.5.0 to current GPUI. That is a project of its own;
     gpui-component's iOS behaviour is untested.
  2. Adopt an unmerged backend: the PR, or Zedra's fork, which is 2,757 commits behind main.
  3. Own the iOS polish ourselves: keyboard avoidance, edit menus, dictation UX, accessibility, and fling tuning.
- So it does not change the recommendation now. The recommendation changes when (a) `gpui_ios` merges upstream and
  (b) the desktop is on current GPUI.
- At that point the cheapest step is Ghostex's hybrid: **the GPUI feed renderer embedded as one view inside the
  native app**, with the native composer, keyboard, dictation and navigation kept. Desktop and phone then share the
  transcript (the costliest UI to build twice) without giving GPUI the parts iOS does best.
- The Swift protocol layer and the window-shape layout rule stay valid either way.

## 2. Orientation locks (one universal app)

- Info.plist per device: `UISupportedInterfaceOrientations~iphone = [Portrait]` and
  `UISupportedInterfaceOrientations~ipad = [LandscapeLeft, LandscapeRight]`. These are also enforced in
  `AppDelegate.application(_:supportedInterfaceOrientationsFor:)`, per idiom.
- **iPad multitasking.** An iPad app that doesn't support all four orientations has always needed
  `UIRequiresFullScreen = YES`, and the prototype sets it. Apple's TN3192 (rev. 2026-08-13) says this key is
  deprecated since iPadOS 26. From iPadOS 27, when an app is built with the iOS 27 SDK, it no longer opts out of
  resizing: the system resizes the scene discretely, moving it to a screen of the window's exact size when a drag
  ends. Apple's stated direction is "support all orientations", with `prefersInterfaceOrientationLocked` as a
  request the system may ignore. Consequences:
  - Landscape-only on iPad is honoured in full screen, but on iPadOS 27 the app must still lay out at any window
    size, because Stage Manager and windowed mode can hand it a tall, narrow window. The layout uses
    `NavigationSplitView`, which collapses to one column below regular width, so that already works.
  - App Store risk: the console says "Support for all orientations will soon be required". If review starts to
    enforce it, the fallback is a letterboxed landscape layout, or accepting portrait on iPad.
**Layout rule (user decision, 2026-10-10): the layout follows the window's shape, not the device.**

- Wide (landscape, width ≥ 700 pt) → the desktop/iPad layout (`NavigationSplitView`: tasks beside the chat).
- Narrow or portrait (an iPad in portrait, Slide Over, a narrow split or window, later a narrow desktop window) →
  the phone layout (`NavigationStack`: tasks → chat; the design agent's phone design).
- The iPad landscape lock is therefore best-effort: whatever shape the system hands us renders correctly.
- iPhone stays portrait-only. If landscape ever shows anyway, a "Rotate to portrait" screen.
- Implemented as one pure function at the root, `layoutFor(size:idiom:)` in `UI/RootView.swift`, re-evaluated on
  every size change.

**Verified in the iOS 27 Simulator** (XCUITest `testOrientationAndLayout`, `testWindowedNarrow`, plus the app's
`-probeRotate 1` probe and its `layout-state` label):

| device | what was done | result |
|---|---|---|
| iPhone 18 Pro | device rotated portrait → landscapeLeft → landscapeRight → upside down | stays **portrait**, 402×778, phone layout every time; the rotate-back screen never needed |
| iPhone 18 Pro | `requestGeometryUpdate(landscapeRight)` | refused: "None of the requested orientations are supported… Supported: portrait" |
| iPad Pro 11" | device rotated through all four | stays **landscape** (landscapeRight/Left), wide layout, 1210×782–804 |
| iPad Pro 11" | `requestGeometryUpdate(portrait)` | refused: "Supported: landscapeLeft, landscapeRight" |
| iPad Pro 11" | window corner dragged left (iPadOS 27 windowing) | **window resized to 375×782 → phone layout**, although `UIRequiresFullScreen = YES` |

The last row confirms TN3192: with the iOS 27 SDK, `UIRequiresFullScreen` no longer prevents resizing, so the
window-shape rule is what keeps the iPad right. Plan: keep the per-idiom orientation keys (the lock held in full
screen), drop `UIRequiresFullScreen` before the App Store (deprecated, and the ~ipad landscape-only keys will be
flagged "support all orientations"). If review insists on all orientations, declare them; the layout rule already
renders an iPad in portrait as the phone layout. Screenshots: `docs/shots/iphone-device-*.png`,
`ipad-device-*.png`, `ipad-narrow-window.png`.

## 3. Reaching the engine from a device

Today the engine is local-only. `chat.reframe.dev` resolves to 127.0.0.1 through `/etc/resolver/reframe.dev` and the
Caddy edge. The Simulator shares the Mac's network and resolver, so it reaches the engine as-is. Two client details:

- **TLS.** The edge serves an mkcert certificate. macOS trusts the mkcert root; the Simulator has its own trust
  store. The app does not change the Simulator's settings. Instead, a DEBUG build bundles mkcert's root (the public
  certificate only, `Resources/dev-root-ca.der`) and adds it as an extra anchor for `*.reframe.dev` only, with full
  verification (`Core/Trust.swift`). Release builds have no custom trust.
- **Origin.** Today's `/ws` refuses any handshake whose Origin isn't `https://<host>`; the edge answers 403. A macOS
  check against the edge's echo app confirmed this: a custom `Origin` on `URLSessionWebSocketTask` is sent and
  accepted, and without it the handshake gets a 403. The app sets it; `/engine/ws` replaces it with a token.

For a real phone the path is the existing tunnel: `https://chat.tunnel.reframe.sh`, through the relay to
`reframe-tunnel`, then to the local edge and the engine. It has a publicly trusted certificate (verified, so no
custom trust is needed) and a gate: until the access key is presented as the `reframe_tunnel_key` cookie (or once
via `?key=`), every request gets a 401 "locked". What the client needs:

1. **Endpoint plus secrets in the Keychain.** The endpoint is `{endpoint, name}`. Per connection: the tunnel access
   key (today) and the engine client token (spec §2), stored as `kSecClassGenericPassword` with
   `kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly`, so a reconnect works after the first unlock and nothing
   syncs to iCloud or goes into backups. Onboarding: scan a QR code shown by the engine's admin page, with the
   endpoint, workspace and token. The token is shown once on the page and never typed.
2. **Headers.** For the WebSocket: `Origin: https://chat.tunnel.reframe.sh` (the tunnel requires its own origin
   and rewrites it to the upstream one) and `Cookie: reframe_tunnel_key=…`. The same cookie goes on `/blob` GETs.
3. **Gap found in `~/.reframe/local/tunnel.ts`, now handled engine-side.** `relayWebSocket` forwards only
   `origin`, `cookie`, `user-agent` and the subprotocols, so an `Authorization: Bearer` on `/engine/ws` is dropped at
   the tunnel. The engine (`lib/routes.ts tokenOf`) now also accepts the token as the subprotocol
   `optchat.token.<token>` and answers with `optchat`. The prototype sends it that way
   (`Sec-WebSocket-Protocol: optchat, optchat.token.<t>`), so the same client works direct and through the tunnel.
   Still open: let the engine token replace the tunnel's access key for native clients, so a phone holds one secret,
   not two.
4. **Reconnect.** Same rule as the page and GPUI: 0.4 s doubling to 5 s. In addition, reconnect at once on an
   `NWPathMonitor` change (Wi-Fi to cellular) and when the scene becomes active. A 15 s ping notices a half-open
   socket, since iOS doesn't always report a dead connection after a network switch. Every reconnect resnapshots;
   the server is the truth.

## 4. Architecture

```
OptChat (app target, SwiftUI; UIKit for the feed and composer)
├── App: scenes, orientation, scene phase → connect/disconnect, deep links, notifications
├── UI: TaskList (sidebar / stack root) · Chat (feed + composer) · Notifications · Settings · WorkspaceMenu
│     feed = UICollectionView (diffable data source, self-sizing cells, TextKit 2 markdown, prefetching),
│            hosted in SwiftUI; cells cache attributed strings by (entry i, width)
└── OptChatKit (Swift package, no UI; also builds for macOS so tests and probes run without a simulator)
    ├── Wire: JSON → records (TaskRecord, LogEntry, TurnRecord, …), tolerant like crates/proto/api.rs
    ├── Store: per-workspace mirror; a port of app/ui/state.ts (snapshot replaces; events apply in order)
    ├── EngineClient: one URLSessionWebSocketTask per engine; ws-tagged messages; connect/disconnect per
    │   workspace; rpc by id with timeouts; backoff, path monitor, ping; blob fetch with a disk cache
    ├── Dictation: AVAudioEngine → pcm16 16 kHz → /engine/stt socket; tap/hold rule; span replacement
    ├── Accounts: engines, workspaces, tokens in the Keychain; QR onboarding
    └── Fixtures: golden JSON captured from the hub (snapshot, every event type, rpc results)
```

What is shared with the desktop:

- **The protocol, as data.** `tests/fixtures/*.json`, captured from a test workspace, are parsed by
  `optchat-proto`'s tests (Rust) and `OptChatKitTests` (Swift), with the same assertions (counts, ids, cut entries,
  live text). A hub change that breaks either client fails CI in both.
- **The client model.** spec-workspaces §4 (engines, workspaces, tokens, notices, switching) is implemented the same
  way: the same states, the same names, the same keyboard-free flows.
- **Not shared:** rendering and the reducer code. The reducer is ~150 lines per client.
- **Measured:** `optchat-mac-gpui/crates/proto` builds unchanged for `aarch64-apple-ios-sim` (12 s) and
  `aarch64-apple-ios` (6 s) (`cargo build -p optchat-proto --release --target …`, a 5.7 MB rlib). So a shared Rust
  core is technically cheap at the proto level. The cost is in the rest: UniFFI, the xcframework, extracting the
  client and state code out of the GPUI app, and the socket. This is why it's deferred, not rejected; if the GPUI
  feed is ever embedded (§1a), proto comes along for free.

The prototype's `OptChat/Core` is that package's seed. `tools/probe` compiles the same files for macOS.

## 5. The protocol client

Today, as implemented and verified:

- `wss://chat.reframe.dev/ws?task=<n>&db=<test instance>` with `Origin`. A snapshot arrives on open; `open {task}`
  gives a fresh snapshot.
- Events: `task`, `log`, `turn`, `request`, `delta`, `status`, `error`, `controlDone`, …
- Commands: `send {task, text, run}`, `cancel`, `rpc {id, method, params}`.
- `rpc log.range {from, to}` pages back.

Next, the `/engine` API from spec-workspaces §2:

- `GET /engine/workspaces` lists workspaces.
- `wss://<endpoint>/engine/ws` with `Authorization: Bearer`: one socket per engine; `connect {ws}` / `disconnect {ws}`;
  `ws` on every message. `open {ws, task}` also returns recent turns and requests.
- `/engine/blob/<sha>?ws=` and `/engine/stt?ws=` take the same token.

The Store becomes `[workspace: Store]`. Background workspaces stay subscribed while the app is in the foreground, so
switching is instant.

Measured from the Simulator's network position (the macOS probe over the app's own `Core` sources, test instance
`tmv23x72y`, a copy of the live chat with 125 tasks):

| check | result |
|---|---|
| first snapshot (cold test hub) | 1.29 s (hub start; warm connects are well under 0.3 s) |
| task switch, open → snapshot applied (5 tasks + main chat) | 16–22 ms |
| `log.range` rpc, 100 older entries | 3 ms |
| drop → reconnect → resnapshot | 284 ms |
| send → turn running | 52 ms |
| send → Haiku reply logged ("pong") | 2.3 s |

## 6. Offline and reconnect

- **Foreground.** Status dot (green/orange/red). Commands wait for the socket: the composer keeps its draft and Send
  is disabled while the socket is closed. Later, an outbox: a message typed offline is sent once after the
  resnapshot, deduplicated by a client message id (needs a `clientId` echo from the hub).
- **Background.** iOS suspends the app within seconds. A socket can't be kept alive: VoIP and other background modes
  are not allowed for this. So when the scene goes to `.background`, the app asks
  `beginBackgroundTask` for time to flush an in-flight send, then closes with 1001. On `.active` it reconnects
  immediately and resnapshots (284 ms measured).
- **Cache.** Persist the last snapshot per workspace and task (SQLite or files, in a protected file class) so a cold
  launch draws the chat at once and greys it until the snapshot arrives. Blobs go in a URLCache or disk cache keyed by
  sha, which never changes.

## 7. Dictation on iOS

Two paths, both needed:

1. **The keyboard's own dictation** works in the composer for free: on-device, any language. No app vocabulary.
2. **The OptChat mic**, the same as web and GPUI, so the engine's vocabulary (keyterms) and service choice apply.
   - `AVAudioSession` is set to `.playAndRecord`, mode `.spokenAudio`, options `.duckOthers` and
     `.allowBluetoothHFP`.
   - An `AVAudioEngine` input tap feeds an `AVAudioConverter` that produces pcm16 mono at 16 kHz in 20–40 ms frames.
     These go as binary messages on a second socket to `/engine/stt?ws=&task=`, and the socket gets `{type:"stop"}`
     at the end.
   - Recording starts at the press; frames are buffered until the socket is up, as the page does.
   - Streamed text replaces the dictated span at the cursor in the UITextView.
   - Press-and-hold: a long-press on the mic button dictates until release; a tap toggles. Haptics mark start and
     stop.
   - Interruptions (a call, Siri) and route changes (AirPods) stop recording and keep the text.
   - Background: dictation stops. No `audio` background mode is wanted.
   - Offline fallback, later: iOS 26's on-device `SpeechAnalyzer`/`SpeechTranscriber` with the keyterms as
     contextual strings.

## 8. Push notifications (later)

Notices (spec §3) are the payload.

- The engine, or a small relay on reframe.so that holds the APNs `.p8` key so it isn't copied to every engine, sends
  APNs alerts for `ok`/`warn`/`error` notices to the device tokens a client registers (an
  `rpc push.register {token, levels}` per workspace).
- Alerts are generic ("#12 done", "budget reached") with `mutable-content`. A Notification Service Extension can
  fetch the text over the tunnel with the Keychain token (shared via a keychain access group), so notice text never
  passes through Apple in clear.
- Tapping a notification deep-links to `optchat://<engine>/<workspace>/t/<id>`; the app reconnects and the notice is
  marked read on the engine (read state is engine-side).
- Later still: a Live Activity for a running turn (ActivityKit push updates: tokens, cost, status).
- Needs a paid Apple Developer team, the push entitlement and a real device; the Simulator accepts `.apns`
  drag-and-drop or `simctl push` for local tests.

## 9. Phases and verification

0. **Prototype (this repo).** Universal app; locks plus the window-shape layout; connect (page `/ws?db=` or
   engine `/engine/ws` + token); task list; chat; send → reply.
   - Protocol verified on macOS via the probe. Layouts and locks verified in the Simulator on iPhone and iPad (§2).
   - **Send → reply verified in the Simulator over `/engine/ws` + token on workspace `test-ios`** (task #106, Haiku,
     one message per device; XCUITest `testSendMessage`):
     - iPhone: task tap → composer 3.4 s (XCUITest overhead included); Send → turn running 1.7 s; reply ("pong")
       finished 4.8 s after Send.
     - iPad: 3.6 s / 1.6 s / 2.7 s.
     - Both replies confirmed through the protocol (`log.range`).
1. **OptChatKit + fixtures.** Move `Core` into a Swift package with XCTest. Capture fixtures from a test workspace;
   add the same fixture tests to `optchat-proto`.
   - Verify: both suites pass; the probe runs in CI against a test workspace.
2. **Engine protocol.** `/engine/ws`, a token per workspace, workspace menu, background subscriptions, notices,
   Keychain, QR onboarding, tunnel endpoint (after the `authorization` forwarding fix).
   - Verify: connect two workspaces; a turn in the background one shows a dot; kill and restart the engine → offline
     within 5 s and back by itself; airplane mode on/off → reconnect; revoke the token → refused at once.
3. **Production feed.** UICollectionView, markdown and code blocks, tool cards, images from blobs, paging, follow
   mode.
   - Verify on a device with Instruments: scroll a 2,000-entry chat with no hitches over 8.3 ms at 120 Hz; task switch
     p90 < 100 ms; memory < 200 MB.
4. **Composer + dictation.** ↑/→ actions (append/run), Stop, images (PhotosPicker, downscaled), the mic with
   tap/hold.
   - Verify with XCUITest using a recorded clip injected via a test hook: tap, hold, interruptions, the cursor span.
5. **Notifications.** APNs relay, registration, service extension, deep links.
   - Verify on a device via the tunnel: finish a Haiku task with the app in the background → notification → tap →
     the task opens.
6. **iPad polish and release.** Keyboard shortcuts (⌘↩, ⌘B), pointer hover, windowed-mode layouts at narrow widths,
   TestFlight.

## 10. Risks

- **iPad landscape lock vs iPadOS 27 windowing** (`UIRequiresFullScreen` no longer stops resizing; App Review may
  require all orientations). Mitigation: adaptive layout, a letterboxed fallback.
- **The tunnel drops `Authorization`** on WebSocket upgrades (see §3). It must be fixed before phase 2 works
  remotely.
- **Protocol drift** between three clients. Mitigated by the fixtures and by tolerant parsing (unknown fields and
  types are ignored).
- **Snapshot size on cellular.** ~130 KB per open, and per workspace on connect. Fine, but a persisted cache plus a
  `since` resume parameter on the hub would make reconnects cheaper.
- **Exposure.** The engine runs local agents with shell access, so a leaked phone token is a remote shell. Use
  Keychain this-device-only storage, per-workspace scope, revocation, last-use display, and an optional Face ID gate
  before Send on remote engines.
- **No background sockets.** Notices must come by push; without the APNs phase the phone only knows what happened
  when it is opened.

## 11. Prototype: how to run, and what is verified

- `./build.sh` builds the universal app for the Simulator.
- Install and launch:
  `xcrun simctl install <dev> build/dd/Build/Products/Debug-iphonesimulator/OptChat.app`, then
  `xcrun simctl launch <dev> com.reframe.optchat.ios -ws test-<id>`, with `SIMCTL_CHILD_OPTCHAT_TOKEN=<token>` set
  once to seed the Keychain. The older page socket: `-db t<id>`.
- The app refuses `main` and anything without a test workspace.
- UI tests: `TEST_RUNNER_SHOTS=$PWD/docs/shots TEST_RUNNER_OPTCHAT_ARGS="-ws test-<id>" TEST_RUNNER_OPTCHAT_TOKEN=…
  [TEST_RUNNER_SEND_TASK=n TEST_RUNNER_SEND_TEXT=…] [TEST_RUNNER_GPUI_PROBE=1] [TEST_RUNNER_WINDOWED=1]
  xcodebuild -project OptChat.xcodeproj -scheme OptChat -destination id=<udid> test`.
  Other switches: `TEST_RUNNER_SHOW_TASK=n` (open a chat and take a screenshot) and `TEST_RUNNER_RESTORE=1`.
- Gotcha: iPadOS remembers a resized window per bundle id even across uninstall. A window narrowed by the corner-drag
  test stays narrow; the full-width screenshots were taken on a second iPad simulator.
- Protocol probe (macOS, the app's own `Core`): `tools/probe/run.sh -db t<id>` or `-ws test-<id>` with
  `OPTCHAT_TOKEN` in the environment (never written to the login keychain).

Screenshots (`docs/shots/`):

| file | shows |
|---|---|
| `iphone-tasks.png`, `iphone-chat.png`, `iphone-chat-typing.png` | iPhone on `test-ios`: task list, task #106 after the two Haiku replies, composer with the keyboard |
| `ipad-tasks.png`, `ipad-chat.png` | iPad Pro 13" on `test-ios`: wide layout, task list beside #106's chat |
| `iphone-device-landscapeLeft.png` (… `-portrait`, `-landscapeRight`, `-upsideDown`) | iPhone rotated: app stays portrait |
| `ipad-device-portrait.png` (… `-landscapeLeft`, `-landscapeRight`, `-upsideDown`) | iPad rotated: app stays landscape, wide layout |
| `ipad-narrow-window.png` | iPad window resized to 375 pt: phone layout |
| `gpui-ios-probe.png`, `gpui-probe-strip.png` | GPUI on iOS (zed#63068): launch, flicks, typing |
| `jpg/*.jpg` | phone-legible copies (~1000 px), one device per image |

## 12. Bake-off: SwiftUI/UIKit vs GPUI on iOS, measured

The user's rule: decide on measured performance and efficiency only (frame times, CPU, energy, memory, launch,
input latency), not on code sharing or effort. GPUI may be upgraded; the latest upstream with the iOS backend is
preferred.

**The two prototypes, with the same scope and the same data:**

- **UIKit** (`bench/uikit`, the stack PLAN §4 recommends for the feed):
  - `UICollectionView` (compositional list, self-sizing cells), one TextKit `UILabel` per entry, attributed strings
    built lazily and cached.
  - Composer bar pinned to `view.keyboardLayoutGuide`; the list keeps its end pinned when the keyboard moves it.
- **GPUI** (`bench/gpui`, built by `bench/gpui/build.sh` into the zed checkout):
  - Upstream zed `gpui-ios-platform` = zed#63068 at 97dc71d (2026-10-09), i.e. current GPUI + the `gpui_ios`
    backend.
  - A bottom-aligned `list`, one element tree per entry, `StyledText` with highlights cached per item. GPUI's own
    `TextInput` (examples/input.rs, IME marked text) as the composer.
  - **Keyboard avoidance implemented:** the backend reports the keyboard as `window.insets().ime`. The root pads its
    bottom by `insets().effective().bottom`, animated over 250 ms with an ease-out cubic, and the bottom-aligned list
    keeps its end pinned. The check after each keyboard animation (last entry inside the viewport, composer above the
    keyboard) passes, as it does for UIKit.
  - Built with `runtime_shaders`: Metal shaders compile at launch, because the offline Metal Toolchain is a separate
    download not taken here (see the launch caveat below).
- **Fixture:** `bench/make_fixture.py` turns a one-time export from the `test-ios` workspace
  (`tools/probe -export`, read-only `log.range`) into `blocks.json`.
  - Markdown-lite is parsed once there, so the apps differ only in their UI stack.
  - The long task (main chat + #67 + #53) has 2,151 entries: 177 user, 337 assistant markdown, 712 tool cards,
    724 tool outputs, 201 report cards; 1,469 code blocks, 2,352 list items.
  - The four other tasks have 64–202 entries.
  - The fixture holds the user's real chat text, so it is gitignored; regenerate it with the two commands above.

**Scenario** (both apps run it themselves from first frame; `bench/run.sh` drives the keyboard part via XCUITest):

| phase | what |
|---|---|
| launch | process creation (`kinfo_proc` / `proc_pidinfo` start time) → first frame with the long task on screen |
| idle | 5 s, nothing changing (our frame callbacks off; GPUI pauses its display link by itself) |
| scroll | 8 s programmatic scroll up through the long task at 2,000 pt/s (identical, repeatable; not a touch fling) |
| stream | 8 s, a live reply growing at 30 tokens/s at the bottom (pinned) |
| stream + scroll | 8 s, streaming continues while scrolling up at 1,000 pt/s |
| task switch | B C D E A, three rounds: request → first frame showing the task |
| keyboard (iPhone) | XCUITest taps the composer (coordinates, same for both), types 30 characters; the app dismisses the keyboard 2.5 s after the last key; show and hide animations recorded for 0.8 s each |

**Metrics** (identical definitions in `Metrics.swift` and `metrics.rs`):
- **Frame interval:** the time between frame callbacks. UIKit: its CADisplayLink. GPUI: `on_next_frame`, which runs
  right after each rendered frame.
- **Hitch:** an interval over 1.5× the screen's frame time.
- **Main-thread busy per frame:** a CFRunLoop observer sums AfterWaiting → BeforeWaiting spans.
- **CPU:** `getrusage` for the process, `thread_info` for the main thread, as % of one core over the phase.
- **Memory:** `phys_footprint`.
- **Keystroke → frame:** input → the frame that carries it. UIKit: the Core Animation commit, via a one-shot
  observer after CA's commit observer. GPUI: its next rendered frame. Both then wait for the same vsync.

**What the simulator can't tell us, and how to measure it on a device:**
- **Not measurable in the simulator:**
  - Energy. "The Power Profiler instrument is not supported on the Simulator" (xctrace, verified).
  - GPU time and render-server hitches. The simulator renders through the Mac's GPU, and the Animation Hitches
    template needs a device.
  - 120 Hz. The simulator runs at 60 Hz, so ProMotion behaviour is unknown; both apps set
    `CADisableMinimumFrameDurationOnPhone`.
  - Thermal behaviour, real touch-to-photon latency, and real launch costs (dyld from flash, precompiled shaders).
  - CPU percentages here are the Mac's cores, so only relative numbers carry over.
- **Device run (ready): `bench/run-device.sh <udid> [N]`.**
- Signing uses `.secrets/asc.json` (App Store Connect API key; `-allowProvisioningUpdates
  -allowProvisioningDeviceRegistration -authenticationKey…`).
- It builds and signs both bench apps and the bench's own XCUITest runner, then installs with `devicectl`.
- Per app and run: a traced run (launch with `-delay 8`, xctrace Power Profiler + Hitches + GPU attached for 55 s),
  then a scenario + keyboard run via XCUITest. Results are pulled from `Documents/bench.json`.
- Then launch-only (6× each: uikit, gpui, gpui-lazy), `bench/trace_summary.py` and `bench/summarize.py`.
- The device must stay unlocked (Auto-Lock: Never); the script checks first.
- The GPUI app is wrapped in `bench/gpui/xcode/OptBenchGPUI.xcodeproj`: a script phase runs cargo (`build.sh
  xcode`), so Xcode's automatic signing and provisioning apply as for any app.

The manual steps it replaces, for reference:
  1. **UIKit:** `xcodebuild -project bench/uikit/OptBenchUIKit.xcodeproj -scheme OptBenchUIKit -configuration Release
     -destination 'generic/platform=iOS' DEVELOPMENT_TEAM=<team> -allowProvisioningUpdates`.
  2. **GPUI:**
     - Build with `DEVICE=1 bench/gpui/build.sh`.
     - For a fair launch, first fetch the Metal Toolchain (`xcodebuild -downloadComponent MetalToolchain`) and drop
       `--features gpui_platform/runtime_shaders` from the script.
     - Embed a provisioning profile and sign: `cp <profile> OptBenchGPUI.app/embedded.mobileprovision; codesign -f -s
       "Apple Development: …" --entitlements <ent.plist> OptBenchGPUI.app`.
  3. **Install and run:** `xcrun devicectl device install app --device <udid> <app>`, then `xcrun devicectl device
     process launch --device <udid> <bundle id>`. After ~45 s, fetch the results with `xcrun devicectl device copy
     from --device <udid> --domain-type appDataContainer --domain-identifier <bundle id> --source Documents/bench.json
     --destination bench/results/raw/<app>-<device>-<n>.json`, then run `python3 bench/summarize.py`.
  4. **Keyboard + keystrokes:** run `testBenchKeyboard` on the device. It polls a host file in the simulator; on a
     device, drop `BENCH_OUT` and wait a fixed 60 s. iPad keyboard phases need a real iPad (see below).
  5. **Energy:** `xcrun xctrace record --template 'Power Profiler' --device <udid> --attach <pid> --time-limit 60s`
     during a run.
     - Phases start at fixed offsets from the first frame: idle +1 s, scroll +6 s, stream +14.5 s,
       stream + scroll +22.5 s, switches +31 s.
     - For battery drain, loop stream + scroll for 30 min at fixed brightness on battery, once per app, and compare
       battery level (and MetricKit `MXCPUMetric`/`MXGPUMetric` from TestFlight builds).
  6. **Hitches as the render server sees them:** the `Animation Hitches` template, the same 60 s window.

### Results (simulator: iPhone 18 Pro and iPad Pro 13" M5, iOS 27.0, 60 Hz, 3 runs per app per device, medians)

Full table: `bench/results/summary.md`; raw per-run JSON: `bench/results/raw/`. The UIKit streaming path was tuned
once, so it is the path a competent UIKit engineer would write: update the visible cell in place and let iOS 16+
self-sizing invalidation resize it. The first version, which re-applied the 2,152-item snapshot per token, cost 23%
CPU while streaming; it is kept in `bench/results/uikit-v1/`.

| metric (iPhone · iPad) | UIKit | GPUI | better |
|---|---|---|---|
| scroll 2000 pt/s: hitches in 8 s | 6 · 10 | 1 · 2 | GPUI |
| scroll: frame interval p99 (ms) | 27.3 · 32.8 | 18.0 · 17.1 | GPUI |
| scroll: main-thread busy p50 / p99 (ms) | 0.8 / 8.5 · 0.9 / 22.6 | 1.6 / 12.2 · 2.6 / 13.4 | mixed (UIKit typical, GPUI worst case on iPad) |
| scroll: CPU, process (% of a core) | 9.8 · 12.1 | 14.8 · 19.7 | UIKit |
| stream 30 tok/s: CPU, process | 15.7 · 11.6 | 9.6 · 15.2 | split |
| stream + scroll: CPU, process / hitches | 19.0 / 2 · 16.5 / 2 | 17.1 / 0 · 21.9 / 0 | split CPU; GPUI on hitches |
| idle: main thread (% of a core) | 0.37 · 0.63 | 0.94 · 1.21 | UIKit (both small) |
| memory after launch / after 15 switches (MB) | 46 / 76 · 41 / 90 | 24 / 56 · 25 / 53 | GPUI (−25 to −40%) |
| task switch p50 / max (ms) | 15.6 / 64 · 22.2 / 81 | 13.6 / 13.7 · 13.6 / 15.1 | GPUI |
| keystroke → frame p50 / p95 (ms, iPhone) | 2.8 / 4.2 | 0.35 / 0.56 | GPUI |
| keyboard show: hitches / busy p99 (ms, iPhone) | 2 / 26.3 | 1 / 13.7 | GPUI |
| keyboard: last entry + composer visible | yes | yes (after the fix above) | tie |
| launch → first frame (ms) | 2,659 · 3,529 | 2,596 · 2,551 | tie / GPUI (simulator launch is noisy; GPUI still compiles shaders at launch here) |

How to read it:
- **GPUI was smoother in every animated phase.** It had fewer hitches and a tighter p99. UIKit's hitches come from
  first-time self-sizing of long attributed-text cells (main-thread spikes of 8–23 ms); GPUI's list measures
  lazily per frame.
- **GPUI used less memory everywhere,** 25–40% less. Core Animation keeps a backing store per cell layer; GPUI keeps
  glyph atlases and draws everything each frame.
- **Task switches are flat in GPUI** (~13.6 ms every time). UIKit spikes to 64–81 ms on the switch back to the
  2,151-entry task (`reloadData` plus `scrollToItem` across estimated sizes).
- **UIKit used less main-thread and process CPU while scrolling and idle.** Part of that is structural: UIKit hands
  rasterization and compositing to the render server, outside the app's process, so the app's CPU numbers
  understate its total. GPUI renders in-process and through its own Metal pipeline. Streaming CPU split by device.
- **The simulator cannot see the decisive cost: GPU and energy.** GPUI redraws the whole window on every change.
  That is 30 full-window redraws a second while a reply streams, and every frame while scrolling. Core Animation
  re-renders only the changed layer and composites cached bitmaps. On a phone that difference shows up as GPU power
  and battery, which only a device can measure (§12, device run, steps 5–6). The simulator also runs at 60 Hz, so
  120 Hz ProMotion headroom is untested for both.

### Launch, re-measured (Metal Toolchain installed, shaders precompiled)

`bench/launch-sim.sh <udid> 8`: each variant is reinstalled, then launched 8 times with `simctl launch`. That is
plain launches, without the XCUITest overhead that flattened the earlier launch row to ~2.6 s for both apps.
The table shows the first launch after install / the warm median, process creation → first frame with the long task
(`bench/results/launch-sim.txt`):

| variant | iPhone 18 Pro sim | iPad Pro 13" sim |
|---|---|---|
| UIKit | 1,689 / **647 ms** | 859 / **479 ms** |
| GPUI, upstream as pinned, precompiled shaders | 5,115 / 2,052 ms | 2,494 / 2,038 ms |
| GPUI, shaders compiled at launch (`SHADERS=runtime`) | 3,421 / 1,997 ms | 3,823 / 2,216 ms |
| **GPUI + lazy SafariServices** (`LAZY_SAFARI=1`) | 928 / **424 ms** | 957 / **493 ms** |

Where GPUI's time went (`launch_breakdown_ms` in the results):
- **The process reached `main` only 1.8–2.1 s after creation**, against UIKit's `didFinishLaunching` at ~0.32 s.
- `simctl spawn` of the bare binary with an exit at `main` took 2.7–3.9 s (`/usr/bin/true`: 0.44 s).
- dyld loaded **1,468 images** before `main`, against **553** for the whole UIKit app. The cause:
  `gpui_ios/src/ios/platform.rs` hard-links `SafariServices` (for `SFSafariViewController` in `open_url`), which
  pulls WebKit and ~900 more images into every launch.
- From `main` to first frame GPUI is fast: ~180 ms (app ready +70 ms, window and view +45 ms, fixture +4 ms,
  first frame +60 ms).
- `bench/gpui/patches/gpui_ios-lazy-safariservices.patch` (57 lines) `dlopen`s SafariServices on the first in-app
  browser open instead. With it, **GPUI launches as fast as UIKit** (−34% on iPhone, +3% on iPad). It is a candidate
  to upstream to zed#63068.
- Precompiled shaders matter mainly for the first launch after install; the warm difference is small.

### Recommendation from the numbers

On everything the simulator can measure, **GPUI on iOS is at least as fast as UIKit and leaner in memory**. It wins
on smoothness, task switching, input latency and memory, and costs more CPU only while scrolling and idle (≤ +8
points of one core). Nothing measured rules it out. So **the numbers favour GPUI provisionally**, which replaces §1's
UIKit recommendation (§1 weighed sharing and effort, which are now out of scope).

**Devices are 60 Hz** (iPad mini A17 Pro, iPadOS 26.1; iPhone 13 mini, iOS 26.5.2), so the rule below drops the
120 Hz criterion and compares relative energy, CPU, GPU, hitches and launch at 60 Hz:
- **Keep GPUI if**, on the iPad mini (A17 Pro) over the same scenario:
  - (a) energy (Power Profiler: the whole traced scenario, and the stream phases) is ≤ 1.15× UIKit's;
  - (b) render-server hitches (Hitches instrument) are ≤ UIKit's, and the in-app p99 frame interval is no worse;
  - (c) warm launch is ≤ UIKit's + 150 ms, with the lazy-SafariServices patch (upstream as pinned is reported too);
  - (d) GPU busy (GPU instrument) during stream and scroll is ≤ 1.5× UIKit's (GPUI redraws the window; CA composites).
- **Otherwise UIKit.**
- The iPhone 13 mini run follows when it is plugged in.

The original rule, written for a 120 Hz iPhone 15 Pro or later:
- **Keep GPUI if**, on an iPhone 15 Pro or later at 120 Hz, over the same scenario:
  - (a) energy (Power Profiler, whole scenario and the stream phase) is ≤ 1.15× UIKit's;
  - (b) the render-server hitch ratio (Animation Hitches) is ≤ UIKit's;
  - (c) launch with precompiled shaders is ≤ UIKit's + 150 ms;
  - (d) a 30-minute stream + scroll battery loop drains ≤ 1.15× UIKit's.
- **Otherwise UIKit.** The SwiftUI shell, protocol layer and layout rule from this repo carry over either way.

What GPUI on iOS still needs for the product (not performance, listed so the device run tests a realistic build):
- keyboard avoidance as done here, or upstreamed into the backend;
- precompiled shaders (the Metal Toolchain download);
- the backend is unmerged (zed#63068); the alternative is Zedra's fork;
- system dictation through the input view, edit menus and selection handles, which still need checking on a device.

Accessibility is not a requirement (user decision), so the backend's near-empty accessibility tree no longer
counts against it.
