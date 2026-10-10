# OptChat for iPhone and iPad: plan

Status: draft, 2026-10-10. The simulator runs and the Rust-for-iOS build test are waiting for approval to download
the iOS 27 Simulator runtime and the Rust iOS targets. Sections marked **(pending)** get filled in after that.

## 1. Recommendation

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
- **GPUI on iOS is not available.** The pinned GPUI is crates.io 0.2.2 (2025-10-22), which has no iOS platform
  module. Upstream, `gpui_ios` is an unmerged PR (zed-industries/zed#63068, opened 2026-08-22, merge state "blocked")
  that leaves out momentum scrolling, edit menus and keyboard accessories. Only the groundwork merged (#64990, a
  shared Metal renderer and dispatcher, 2026-09-30). Building on it means tracking an unmerged fork and writing the
  iOS text input, IME, dictation and scrolling physics ourselves.
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
| GPUI on iOS | unknown; no momentum scroll yet | full | IME/dictation to write | to write | L+, blocked upstream | track an unmerged fork |
| WKWebView of the web page | OK at best | none (shares with web) | web-grade | JS suspended, reload | S | low, but web limits stay |
| React Native | good with FlashList | with web (TS) only | modules | modules | L | third UI stack |

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
- Simulator verification **(pending runtime)**: launch on an iPhone and an iPad, take screenshots, and run
  `-probeRotate 1`, which makes the app call `requestGeometryUpdate` with the forbidden orientation and log whether
  it is refused. The physical-rotation behaviour (Device → Rotate) also gets checked.

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
3. **Gap found in `~/.reframe/local/tunnel.ts`.** `relayWebSocket` forwards only `origin`, `cookie`, `user-agent`
   and the subprotocols to the upstream socket, so an `Authorization: Bearer <client token>` on `/engine/ws` would be
   dropped at the tunnel. Either the tunnel forwards `authorization` on WebSocket upgrades (a one-line change; HTTP
   requests already pass it), or the engine also accepts the token as a subprotocol (`optchat-bearer.<token>`, the
   pattern ChatGPT's dictation endpoint uses). Recommendation: forward `authorization`, and let the engine token
   replace the tunnel's access key for native clients later, so a phone holds one secret, not two.
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

0. **Prototype (this repo).** Universal app; locks; connect; task list; chat; send → reply.
   - Verified on macOS via the probe. Simulator screenshots on iPhone and iPad are **(pending runtime)**.
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
