//! Platform hooks the app shell installs (no-op until it does; the macOS shell installs none):
//! - scan a QR code (iOS: the camera), returning the scanned string; the Workspaces panel shows a camera button next
//!   to the engine token field when the platform can scan.
use gpui::{App, Global, Window};
use std::rc::Rc;

pub type ScanDone = Box<dyn FnOnce(Option<String>, &mut Window, &mut App)>;

pub trait Platform: 'static {
    /// whether a QR scanner is available (a camera)
    fn can_scan_qr(&self) -> bool {
        false
    }
    /// scan one QR code; call `done` on the main thread with its text (None: cancelled / failed)
    fn scan_qr(&self, window: &mut Window, cx: &mut App, done: ScanDone) {
        done(None, window, cx)
    }
}

struct Hooks(Rc<dyn Platform>);
impl Global for Hooks {}

/// the shell installs its platform hooks once at launch
pub fn install(cx: &mut App, p: Rc<dyn Platform>) {
    cx.set_global(Hooks(p));
}
pub fn can_scan_qr(cx: &App) -> bool {
    cx.try_global::<Hooks>().is_some_and(|h| h.0.can_scan_qr())
}
pub fn scan_qr(window: &mut Window, cx: &mut App, done: ScanDone) {
    match cx.try_global::<Hooks>().map(|h| h.0.clone()) {
        Some(p) => p.scan_qr(window, cx, done),
        None => done(None, window, cx),
    }
}

/// tests: a "scanner" that returns OPTCHAT_TEST_QR (installed by the macOS shell only when that is set)
pub struct TestScanner(pub String);
impl Platform for TestScanner {
    fn can_scan_qr(&self) -> bool {
        true
    }
    fn scan_qr(&self, window: &mut Window, cx: &mut App, done: ScanDone) {
        done(Some(self.0.clone()), window, cx)
    }
}
