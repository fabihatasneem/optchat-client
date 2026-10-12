//! The macOS Platform: tokens in the Keychain, resident memory via proc_pidinfo, and (tests only) a QR "scanner"
//! returning OPTCHAT_TEST_QR. Macs have no QR scanner otherwise.
use gpui::{App, Window};
use optchat_platform::{Keychain, Platform, ScanDone, SecureStore};
use std::sync::Arc;

/// the Keychain service the app has always used (items survive the move to this shell)
pub const KEYCHAIN_SERVICE: &str = "com.reframe.optchat.gpui";

pub struct Mac {
    store: Arc<dyn SecureStore>,
    test_qr: Option<String>,
}

impl Mac {
    pub fn new() -> Self {
        Mac { store: Arc::new(Keychain { service: KEYCHAIN_SERVICE }), test_qr: std::env::var("OPTCHAT_TEST_QR").ok() }
    }
}

impl Platform for Mac {
    fn secure_store(&self) -> Option<Arc<dyn SecureStore>> {
        Some(self.store.clone())
    }
    fn resident_mb(&self) -> Option<f64> {
        // proc_pidinfo(PROC_PIDTASKINFO): a syscall, no subprocess on the UI thread
        let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
        let size = std::mem::size_of::<libc::proc_taskinfo>() as i32;
        let n = unsafe { libc::proc_pidinfo(std::process::id() as i32, libc::PROC_PIDTASKINFO, 0, &mut info as *mut _ as *mut libc::c_void, size) };
        (n == size).then(|| info.pti_resident_size as f64 / (1024.0 * 1024.0))
    }
    fn can_scan_qr(&self) -> bool {
        self.test_qr.is_some()
    }
    fn scan_qr(&self, window: &mut Window, cx: &mut App, done: ScanDone) {
        done(self.test_qr.clone(), window, cx)
    }
}

#[cfg(test)]
mod tests {
    use optchat_platform::{Keychain, SecureStore};
    /// the Keychain round trip (an item this test binary creates is readable by it without a prompt)
    #[::core::prelude::v1::test]
    fn keychain_round_trip() {
        let k = Keychain { service: super::KEYCHAIN_SERVICE };
        let key = format!("test|roundtrip-{}", std::process::id());
        k.set(&key, "not-a-real-token").unwrap();
        assert_eq!(k.get(&key).as_deref(), Some("not-a-real-token"));
        k.delete(&key);
        assert_eq!(k.get(&key), None);
    }
}
