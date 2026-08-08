use super::App;
use std::time::{Duration, Instant};

/// Severity class for toast notifications. Neutral and Success display for 2 s;
/// Warning and Error display for 5 s.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToastSeverity {
    /// Progress, information, lifecycle events.
    #[default]
    Neutral,
    /// A user-requested action completed.
    Success,
    /// An operation failed but recovered through a working fallback.
    Warning,
    /// An operation failed without recovery.
    Error,
}

impl ToastSeverity {
    pub fn ttl(&self) -> Duration {
        match self {
            ToastSeverity::Neutral | ToastSeverity::Success => Duration::from_secs(2),
            ToastSeverity::Warning | ToastSeverity::Error => Duration::from_secs(5),
        }
    }

    /// Toast background color for this severity when rendered as a colored
    /// toast (not status-bar styling). Neutral is never rendered this way,
    /// but is mapped for exhaustiveness rather than left `unreachable!()`.
    pub fn toast_bg(&self) -> ratatui::style::Color {
        match self {
            ToastSeverity::Neutral => super::palette::DARK_BG,
            ToastSeverity::Success => super::palette::TOAST_BG_SUCCESS,
            ToastSeverity::Warning => super::palette::TOAST_BG_WARNING,
            ToastSeverity::Error => super::palette::TOAST_BG,
        }
    }
}

// #286: `App::ring_terminal_bell()` writes to a thread-local buffer instead
// of real stderr in test builds, so tests never touch the process-wide
// STDERR_FILENO fd. `cargo test` runs each test on its own OS thread, so a
// thread-local is naturally isolated per test with no locking required.
#[cfg(test)]
thread_local! {
    pub(super) static TEST_BELL_LOG: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
}

impl App {
    fn notify_system(&self, msg: &str) {
        if self.system_notifications {
            let tx = self.notif_action_tx.clone();
            let mut cmd = std::process::Command::new("notify-send");
            cmd.arg("--app-name=mbv")
                .arg("mbv")
                .arg(msg)
                .stderr(std::process::Stdio::null());
            std::thread::spawn(move || {
                if !cmd.output().map(|o| o.status.success()).unwrap_or(false) {
                    let _ = tx.send("__notif_failed__".into());
                }
            });
        }
    }

    #[cfg(not(test))]
    fn ring_terminal_bell() {
        use std::io::Write;

        let mut stderr = std::io::stderr();
        let _ = stderr.write_all(b"\x07");
        let _ = stderr.flush();
    }

    // See the `TEST_BELL_LOG` doc comment above for why test builds don't
    // touch real stderr here.
    #[cfg(test)]
    fn ring_terminal_bell() {
        TEST_BELL_LOG.with(|log| log.borrow_mut().push(b'\x07'));
    }

    pub(super) fn notify_with_actions(&self, title: &str, body: &str, actions: &[(&str, &str)]) {
        Self::ring_terminal_bell();
        if !self.system_notifications {
            return;
        }
        let mut cmd = std::process::Command::new("notify-send");
        cmd.arg("--app-name=mbv")
            .arg(title)
            .arg(body)
            .stderr(std::process::Stdio::null());
        for (id, label) in actions {
            cmd.arg(format!("--action={}={}", id, label));
        }
        let tx = self.notif_action_tx.clone();
        std::thread::spawn(move || match cmd.output() {
            Ok(out) if out.status.success() => {
                let chosen = String::from_utf8_lossy(&out.stdout).trim().to_string();
                let _ = tx.send(chosen);
            }
            _ => {
                let _ = tx.send("__notif_failed__".into());
            }
        });
    }

    pub(super) fn trigger_lib_rescan(&mut self, lib_idx: usize) {
        self.clear_saved_library_position(lib_idx);
        let client = self.client.lock().unwrap().clone();
        let library_id = self.libs[lib_idx].library.id.clone();
        let name = self.libs[lib_idx].library.name.clone();
        std::thread::spawn(move || {
            let _ = client.post_library_refresh(&library_id);
        });
        self.flash(format!("Scanning '{name}'..."), ToastSeverity::Neutral);
    }

    /// Display a toast classified by `severity`. Duration derives from severity
    /// (Neutral/Success → 2 s, Warning/Error → 5 s). Never rings the terminal
    /// bell. Desktop notification is attempted only when severity is not Neutral
    /// (preserving the existing `system_notifications` gating and the
    /// hide-on-success behavior in the render path).
    pub(super) fn flash(&mut self, msg: String, severity: ToastSeverity) {
        if severity != ToastSeverity::Neutral {
            self.notify_system(&msg);
        }
        self.status = msg;
        self.status_severity = severity;
        self.status_expires = Some(Instant::now() + severity.ttl());
    }

    /// Shorthand for the common `flash(format!("Error: {e}"), ToastSeverity::Error)` case.
    pub(super) fn flash_error(&mut self, e: impl std::fmt::Display) {
        self.flash(format!("Error: {e}"), ToastSeverity::Error);
    }

    /// Enforces #223's queue-route invariant: an item whose resolved
    /// route differs from the queue's current route (`active_route`) is
    /// rejected with a toast instead of being appended or silently
    /// swapping the player. Returns `true` if the enqueue was rejected --
    /// the caller must abort without mutating the queue.
    ///
    /// Short-circuits `false` (no conflict) whenever the app is currently
    /// in a thin-client mode that has nothing to do with library routing
    /// (a Sessions-panel attached session, or a non-library-route direct
    /// remote / local-daemon connection) -- both leave `active_route` at
    /// `None`, so without this check any item resolving to a configured
    /// `library_routes` entry would be wrongly rejected for a reason
    /// unrelated to library routing. Mirrors the same condition Task 9
    /// uses to gate `apply_route_for_playback`.
    pub(super) fn enqueue_route_conflict(&mut self, resolved_name: Option<String>) -> bool {
        if self.in_non_library_thin_client_mode() {
            return false;
        }
        if resolved_name != self.active_route {
            self.flash(
                "Can't mix libraries in a routed queue -- clear queue first".to_string(),
                ToastSeverity::Error,
            );
            true
        } else {
            false
        }
    }
}
