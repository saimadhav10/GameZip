use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preferences {
    pub quiet: bool,
    pub idle_only: bool,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            quiet: true,
            idle_only: false,
        }
    }
}

pub struct Policy {
    pub quiet: AtomicBool,
    pub idle_only: AtomicBool,
}
impl Policy {
    pub fn new(value: Preferences) -> Self {
        Self {
            quiet: AtomicBool::new(value.quiet),
            idle_only: AtomicBool::new(value.idle_only),
        }
    }
    pub fn get(&self) -> Preferences {
        Preferences {
            quiet: self.quiet.load(Ordering::Relaxed),
            idle_only: self.idle_only.load(Ordering::Relaxed),
        }
    }
    pub fn set(&self, value: Preferences) {
        self.quiet.store(value.quiet, Ordering::Relaxed);
        self.idle_only.store(value.idle_only, Ordering::Relaxed);
    }
}

fn idle() -> bool {
    // Deterministic native smoke tests use an isolated debug-only profile.
    #[cfg(debug_assertions)]
    if std::env::var_os("GAMEZIP_TEST_DATA_DIR").is_some()
        && std::env::var_os("GAMEZIP_TEST_ACTIVE").is_some()
    {
        return false;
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::{
            System::SystemInformation::GetTickCount,
            UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO},
        };
        let mut input = LASTINPUTINFO {
            cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
            dwTime: 0,
        };
        // Reads only this session's last-input timestamp, never keys or content.
        unsafe {
            GetLastInputInfo(&mut input) != 0
                && GetTickCount().wrapping_sub(input.dwTime) >= 120_000
        }
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[derive(Default)]
pub struct Pause {
    pub requested: AtomicBool,
    pub waiting: AtomicBool,
    pub waiting_idle: AtomicBool,
}

impl Pause {
    /// Wait only between files, retaining the active-job reservation. Cancel and
    /// tray Quit always wake this loop without needing Resume first.
    #[cfg(test)]
    pub fn checkpoint(&self, cancel: &AtomicBool) -> bool {
        self.ready(cancel, &Policy::new(Preferences::default()), || true)
    }

    pub fn scheduled_checkpoint(&self, cancel: &AtomicBool, policy: &Policy) -> bool {
        self.ready(cancel, policy, idle)
    }

    fn ready(
        &self,
        cancel: &AtomicBool,
        policy: &Policy,
        mut is_idle: impl FnMut() -> bool,
    ) -> bool {
        while !cancel.load(Ordering::Relaxed) {
            let manual = self.requested.load(Ordering::Relaxed);
            let automatic = policy.idle_only.load(Ordering::Relaxed) && !is_idle();
            self.waiting.store(manual, Ordering::Relaxed);
            self.waiting_idle
                .store(automatic && !manual, Ordering::Relaxed);
            if !manual && !automatic {
                break;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        self.waiting.store(false, Ordering::Relaxed);
        self.waiting_idle.store(false, Ordering::Relaxed);
        !cancel.load(Ordering::Relaxed)
    }

    /// Quiet mode gives other work a rest interval equal to the preceding file's
    /// processing time. This limits sustained duty cycle, not peak disk bandwidth.
    pub fn rest(&self, elapsed: Duration, cancel: &AtomicBool, policy: &Policy) {
        let start = std::time::Instant::now();
        while policy.quiet.load(Ordering::Relaxed)
            && start.elapsed() < elapsed
            && !cancel.load(Ordering::Relaxed)
        {
            if self.requested.load(Ordering::Relaxed) {
                break;
            }
            std::thread::sleep(
                (elapsed - start.elapsed().min(elapsed)).min(Duration::from_millis(100)),
            );
        }
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
    fn idle_wait_releases_on_activity_policy_change_and_cancel() {
        for action in 0..3 {
            let pause = Arc::new(Pause::default());
            let policy = Arc::new(Policy::new(Preferences {
                quiet: true,
                idle_only: true,
            }));
            let cancel = Arc::new(AtomicBool::new(false));
            let idle = Arc::new(AtomicBool::new(false));
            let (p, q, c, i) = (pause.clone(), policy.clone(), cancel.clone(), idle.clone());
            let worker = std::thread::spawn(move || p.ready(&c, &q, || i.load(Ordering::Relaxed)));
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while !pause.waiting_idle.load(Ordering::Relaxed) {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(Duration::from_millis(10));
            }
            assert!(!worker.is_finished());
            match action {
                0 => idle.store(true, Ordering::Relaxed),
                1 => policy.idle_only.store(false, Ordering::Relaxed),
                _ => cancel.store(true, Ordering::Relaxed),
            }
            assert_eq!(worker.join().unwrap(), action != 2);
            assert!(!pause.waiting_idle.load(Ordering::Relaxed));
        }
    }

    #[test]
    fn quiet_rests_and_cancel_does_not_wait_out_a_long_file() {
        let pause = Pause::default();
        let cancel = AtomicBool::new(false);
        let policy = Policy::new(Preferences::default());
        let start = std::time::Instant::now();
        pause.rest(Duration::from_millis(30), &cancel, &policy);
        assert!(start.elapsed() >= Duration::from_millis(30));
        cancel.store(true, Ordering::Relaxed);
        let start = std::time::Instant::now();
        pause.rest(Duration::from_secs(60), &cancel, &policy);
        assert!(start.elapsed() < Duration::from_secs(1));
    }
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
