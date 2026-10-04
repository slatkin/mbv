//! Tray ownership for the daemon event loop (change stay-alive-is-lifetime-only,
//! design D4). The loop reconciles the Tray on a 1 s cadence against the live
//! owner settings: the hook is called only when the enabled value turns on, and
//! the stored Tray is dropped when it turns off. Dropping the boxed ksni handle
//! is what removes the icon (design D5).

use crate::OwnerSettingsReader;
use crate::core::OnTrayReady;
use std::sync::mpsc;
use std::time::{Duration, Instant};

/// How often the loop re-reads the owner settings to reconcile the Tray.
const TRAY_POLL_INTERVAL: Duration = Duration::from_secs(1);

/// The loop's Tray state: the last value acted on, the Tray itself, and the
/// hook that produces it. The hook is called only on an off→on transition, so
/// a headless host (hook returns `None`) is not retried every second.
pub(crate) struct TrayState {
    on_tray_ready: OnTrayReady,
    shutdown_signal_tx: mpsc::SyncSender<()>,
    tray: Option<Box<dyn Send>>,
    /// The last value `reconcile` acted on, not the settings' current value.
    enabled: bool,
    last_check: Instant,
}

impl TrayState {
    pub(crate) fn new(
        on_tray_ready: OnTrayReady,
        shutdown_signal_tx: mpsc::SyncSender<()>,
    ) -> Self {
        Self {
            on_tray_ready,
            shutdown_signal_tx,
            tray: None,
            enabled: false,
            last_check: Instant::now(),
        }
    }

    /// Brings the Tray in line with `enabled`: starts it on an off→on turn,
    /// removes it on an on→off turn, and does nothing when the value is
    /// unchanged — even when the Tray happens to be `None`.
    pub(crate) fn reconcile(&mut self, enabled: bool) {
        if enabled == self.enabled {
            return;
        }
        self.enabled = enabled;
        self.tray = if enabled {
            (self.on_tray_ready)(self.shutdown_signal_tx.clone())
        } else {
            None
        };
    }

    /// Re-reads the owner settings and reconciles the Tray, at most once per
    /// [`TRAY_POLL_INTERVAL`]. The caller supplies `now`, so tests can drive
    /// the cadence without sleeping.
    pub(crate) fn poll(&mut self, now: Instant, owner_settings: &OwnerSettingsReader) {
        if now.duration_since(self.last_check) < TRAY_POLL_INTERVAL {
            return;
        }
        self.last_check = now;
        self.reconcile((*owner_settings)().tray_enabled());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ConsumeKinds, OwnerSettings};
    use std::cell::Cell;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    /// A boxed Tray stand-in whose `Drop` sets a flag, standing in for the
    /// ksni handle whose drop stops the service (design D5). The flag is
    /// shared because the returned box must be `Send`.
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    fn shutdown_tx() -> mpsc::SyncSender<()> {
        mpsc::sync_channel(1).0
    }

    /// The hook-call counter lives inside one test thread, so `Rc` is enough.
    fn counting_dropping_hook(calls: Rc<Cell<usize>>, dropped: Arc<AtomicBool>) -> OnTrayReady {
        Box::new(move |_tx| {
            calls.set(calls.get() + 1);
            Some(Box::new(DropFlag(Arc::clone(&dropped))) as Box<dyn Send>)
        })
    }

    fn counting_none_hook(calls: Rc<Cell<usize>>) -> OnTrayReady {
        Box::new(move |_tx| {
            calls.set(calls.get() + 1);
            None
        })
    }

    /// An `OwnerSettingsReader` that counts how many times the loop read it.
    fn counting_reader(reads: Arc<Mutex<usize>>) -> OwnerSettingsReader {
        Arc::new(move || {
            *Mutex::lock(&reads).unwrap() += 1;
            OwnerSettings {
                stay_alive: true,
                show_systray_icon: false,
                consume: ConsumeKinds {
                    videos: false,
                    audio: false,
                },
            }
        })
    }

    /// Contract: `local-daemon-tray` "Stay-alive turned on during the session"
    /// — starting the Tray goes through the hook, exactly once per enable.
    #[test]
    fn turning_on_calls_the_hook_once() {
        let calls = Rc::new(Cell::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let mut state = TrayState::new(
            counting_dropping_hook(Rc::clone(&calls), Arc::clone(&dropped)),
            shutdown_tx(),
        );

        state.reconcile(true);

        assert_eq!(calls.get(), 1);
        assert!(state.tray.is_some());
        assert!(!dropped.load(Ordering::SeqCst));
    }

    /// Contract: `local-daemon-tray` "Stay-alive turned off during the
    /// session" — removing the Tray drops the boxed handle, which is the stop
    /// operation (design D5).
    #[test]
    fn turning_off_drops_the_tray() {
        let calls = Rc::new(Cell::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let mut state = TrayState::new(
            counting_dropping_hook(calls, Arc::clone(&dropped)),
            shutdown_tx(),
        );
        state.reconcile(true);
        assert!(!dropped.load(Ordering::SeqCst));

        state.reconcile(false);

        assert!(dropped.load(Ordering::SeqCst));
        assert!(state.tray.is_none());
    }

    /// Contract: `local-daemon-tray` "A missing tray is not an error" — a
    /// headless host's `None` result is not retried while the enabled value
    /// stays on.
    #[test]
    fn unchanged_true_with_a_missing_tray_does_not_call_the_hook_again() {
        let calls = Rc::new(Cell::new(0));
        let mut state = TrayState::new(counting_none_hook(Rc::clone(&calls)), shutdown_tx());

        state.reconcile(true);
        state.reconcile(true);

        assert_eq!(calls.get(), 1);
    }

    /// Contract: design D4's 1 s cadence — `poll` reads the owner settings at
    /// most once per interval, and does read them once the interval passes.
    #[test]
    fn poll_within_one_second_of_the_last_check_does_not_read_settings() {
        let reads = Arc::new(Mutex::new(0));
        let mut state = TrayState::new(counting_none_hook(Rc::new(Cell::new(0))), shutdown_tx());
        let start = Instant::now();
        state.last_check = start;

        state.poll(start, &counting_reader(Arc::clone(&reads)));
        assert_eq!(*Mutex::lock(&reads).unwrap(), 0);

        state.poll(
            start + Duration::from_secs(2),
            &counting_reader(Arc::clone(&reads)),
        );
        assert_eq!(*Mutex::lock(&reads).unwrap(), 1);
    }
}
