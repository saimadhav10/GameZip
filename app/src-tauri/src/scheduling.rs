use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Default)]
pub struct Pause {
    pub requested: AtomicBool,
    pub waiting: AtomicBool,
}

impl Pause {
    /// Wait only between files, retaining the active-job reservation. Cancel and
    /// tray Quit always wake this loop without needing Resume first.
    pub fn checkpoint(&self, cancel: &AtomicBool) -> bool {
        while self.requested.load(Ordering::Relaxed) && !cancel.load(Ordering::Relaxed) {
            self.waiting.store(true, Ordering::Relaxed);
            std::thread::sleep(Duration::from_millis(100));
        }
        self.waiting.store(false, Ordering::Relaxed);
        !cancel.load(Ordering::Relaxed)
    }
}

/// Scope background scheduling to the worker, never the UI thread. Windows
/// lowers this thread's CPU, memory and I/O scheduling priorities together.
pub struct Background(bool);
impl Background {
    pub fn enter() -> Self {
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Threading::*;
            // Current-thread pseudo-handle is valid for the duration of this call.
            Self(
                unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN) } != 0,
            )
        }
        #[cfg(not(windows))]
        Self(false)
    }
}
impl Drop for Background {
    fn drop(&mut self) {
        #[cfg(windows)]
        if self.0 {
            use windows_sys::Win32::System::Threading::*;
            // Restore before a pooled preparation thread is reused.
            unsafe {
                SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    #[test]
    fn pause_blocks_until_resumed_and_cancel_wakes_it() {
        for cancelling in [false, true] {
            let pause = Arc::new(Pause::default());
            let cancel = Arc::new(AtomicBool::new(false));
            pause.requested.store(true, Ordering::Relaxed);
            let p = pause.clone();
            let c = cancel.clone();
            let worker = std::thread::spawn(move || p.checkpoint(&c));
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !pause.waiting.load(Ordering::Relaxed) {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(!worker.is_finished());
            if cancelling {
                cancel.store(true, Ordering::Relaxed);
            } else {
                pause.requested.store(false, Ordering::Relaxed);
            }
            assert_eq!(worker.join().unwrap(), !cancelling);
            assert!(!pause.waiting.load(Ordering::Relaxed));
        }
    }
}
