//! Timers on GPUI's background executor (set once at launch, used from the main thread).
use gpui::{BackgroundExecutor, Task};
use std::time::Duration;

thread_local!(static EXEC: std::cell::RefCell<Option<BackgroundExecutor>> = const { std::cell::RefCell::new(None) });
pub fn set_executor(e: BackgroundExecutor) {
    EXEC.with(|x| *x.borrow_mut() = Some(e));
}
pub struct Timer;
impl Timer {
    /// a future that completes after `d` (create it on the main thread)
    pub fn after(d: Duration) -> Task<()> {
        EXEC.with(|x| x.borrow().as_ref().expect("executor set at launch").timer(d))
    }
}
