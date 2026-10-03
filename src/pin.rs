//! `--pin` start-up: pty hand-over, panel lifetime and failure reporting
//! (change `pin-mbv-in-pinwin`, design D3/D5).
//!
//! The pinned panel is a process-wide singleton: `pinwin` allows one panel at
//! a time, so a launch either completes the stdio hand-over once or the TUI
//! keeps running in the terminal it was started from.

use std::fmt;
use std::io::IsTerminal;
use std::os::fd::{AsRawFd, FromRawFd};
use std::sync::atomic::{AtomicBool, Ordering};

use mbv_config::PanelConfig;

/// The running panel handle the shell owns.
pub(crate) type PinnedPanel = mbv_pinwin::Panel<'static>;

/// Which of the two saved `[panel]` widths the live panel shows (change
/// `panel-expand-toggle`, design D1). Runtime state only: the width is never
/// saved and every pinned launch starts `Collapsed`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum PinnedWidth {
    /// `[panel] cols`, the width every pinned launch starts at.
    #[default]
    Collapsed,
    /// `[panel] cols_expanded`.
    Expanded,
}

impl PinnedWidth {
    /// The other width, for the toggle action (design D3).
    #[must_use]
    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Collapsed => Self::Expanded,
            Self::Expanded => Self::Collapsed,
        }
    }
}

/// Apply a full `[panel]` layout at `width` to the live panel (design D6).
pub(crate) fn apply_layout(
    panel: &PinnedPanel,
    config: &PanelConfig,
    width: PinnedWidth,
) -> Result<(), String> {
    panel
        .apply_layout(layout_from_config(config, width))
        .map_err(|error| error.to_string())
}

/// Why a pinned launch did not reach the stdio hand-over (design D5).
#[derive(Debug)]
pub(crate) enum PinStartError {
    /// `WAYLAND_DISPLAY` is unset or empty.
    NoWaylandDisplay,
    /// The pty pair could not be opened.
    Pty(std::io::Error),
    /// `pinwin` refused to start the panel.
    Panel(mbv_pinwin::PinwinError),
    /// The stdio hand-over failed after the panel started.
    HandOver(std::io::Error),
}

impl fmt::Display for PinStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWaylandDisplay => f.write_str("WAYLAND_DISPLAY is unset"),
            Self::Pty(error) => write!(f, "could not open a pty pair: {error}"),
            Self::Panel(error) => write!(f, "{error}"),
            Self::HandOver(error) => write!(f, "terminal hand-over failed: {error}"),
        }
    }
}

impl std::error::Error for PinStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Pty(error) | Self::HandOver(error) => Some(error),
            Self::Panel(error) => Some(error),
            Self::NoWaylandDisplay => None,
        }
    }
}

/// Set once the stdio hand-over has completed: the panel is then the TUI's
/// only output.
static HANDED_OVER: AtomicBool = AtomicBool::new(false);

/// Whether this launch runs in a pinned panel.
#[must_use]
pub(crate) fn is_pinned() -> bool {
    HANDED_OVER.load(Ordering::Relaxed)
}

/// Start the pinned panel and hand this process's stdio to its pty (design
/// D3). The terminal environment is restored if anything fails before the
/// hand-over completes.
pub(crate) fn start(config: &PanelConfig) -> Result<PinnedPanel, PinStartError> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none_or(|value| value.is_empty()) {
        return Err(PinStartError::NoWaylandDisplay);
    }
    let saved_env = PanelEnv::apply();
    match start_panel(config) {
        Ok(panel) => Ok(panel),
        Err(error) => {
            saved_env.restore();
            Err(error)
        }
    }
}

/// The pty pair, the panel, and the stdio hand-over. A failure after
/// `pinwin_start` succeeded drops the panel, which stops it (design D3).
fn start_panel(config: &PanelConfig) -> Result<PinnedPanel, PinStartError> {
    use std::os::fd::AsFd;

    let (master, slave) = open_pty()?;
    // The panel reads the master until `pinwin_stop`; `App` owns the handle
    // and cannot borrow a local fd, so the master is leaked for the life of
    // the process, which the panel's whole life fits inside.
    let master: &'static std::os::fd::OwnedFd = Box::leak(Box::new(master));
    let panel = mbv_pinwin::Panel::start(
        master.as_fd(),
        // Every pinned launch starts at the collapsed width (design D4).
        layout_from_config(config, PinnedWidth::Collapsed),
        mbv_pinwin::KeyboardMode::OnDemand,
    )
    .map_err(PinStartError::Panel)?;
    detach_controlling_terminal()?;
    hand_over_stdio(&slave)?;
    // `openpty` may reuse a closed standard fd for the slave; only close the
    // extra descriptor when it is not one of the duplicates.
    if slave.as_raw_fd() > libc::STDERR_FILENO {
        drop(slave);
    } else {
        std::mem::forget(slave);
    }
    HANDED_OVER.store(true, Ordering::Relaxed);
    Ok(panel)
}

/// The pinwin layout for `config` at `width` (design D2): `side` and the
/// gutters are shared by both widths, only the columns differ.
fn layout_from_config(config: &PanelConfig, width: PinnedWidth) -> mbv_pinwin::Layout {
    mbv_pinwin::Layout {
        side: match config.side {
            mbv_config::PanelSide::Left => mbv_pinwin::Side::Left,
            mbv_config::PanelSide::Right => mbv_pinwin::Side::Right,
        },
        cols: match width {
            PinnedWidth::Collapsed => config.cols,
            PinnedWidth::Expanded => config.cols_expanded,
        },
        gutters: mbv_pinwin::Gutters {
            top: config.gutter_top,
            bottom: config.gutter_bottom,
            left: config.gutter_left,
            right: config.gutter_right,
        },
    }
}

fn open_pty() -> Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd), PinStartError> {
    let mut master = -1;
    let mut slave = -1;
    // SAFETY: `openpty` writes two owned fds on success; null termios and
    // winsize request the platform defaults.
    let opened = unsafe {
        libc::openpty(
            &raw mut master,
            &raw mut slave,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    if opened != 0 {
        return Err(PinStartError::Pty(std::io::Error::last_os_error()));
    }
    // SAFETY: a successful `openpty` returned two fresh fds this process owns.
    unsafe {
        Ok((
            std::os::fd::OwnedFd::from_raw_fd(master),
            std::os::fd::OwnedFd::from_raw_fd(slave),
        ))
    }
}

fn detach_controlling_terminal() -> Result<(), PinStartError> {
    // A process has a controlling terminal iff `/dev/tty` opens; only then is
    // there anything to detach from. Detaching through that fd also handles a
    // launch whose stdin is redirected.
    // SAFETY: probing `/dev/tty` with `O_NOCTTY` so it cannot become ours.
    let tty = unsafe { libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR | libc::O_NOCTTY) };
    if tty < 0 {
        return Ok(());
    }

    // `SIGHUP` is ignored across `TIOCNOTTY`: when mbv is the session leader
    // (a terminal emulator started it directly), detaching hangs up its own
    // foreground group (design D3).
    // SAFETY: the disposition is process-wide and is restored below; no other
    // thread relies on `SIGHUP` across this call.
    let previous = unsafe { libc::signal(libc::SIGHUP, libc::SIG_IGN) };
    // SAFETY: `TIOCNOTTY` takes no argument and detaches the terminal behind
    // the `/dev/tty` fd opened just above.
    let detached = unsafe { libc::ioctl(tty, libc::TIOCNOTTY) };
    // SAFETY: restoring the disposition saved above.
    unsafe { libc::signal(libc::SIGHUP, previous) };
    // SAFETY: `tty` is the valid fd opened just above.
    unsafe { libc::close(tty) };
    if detached != 0 {
        return Err(PinStartError::HandOver(std::io::Error::last_os_error()));
    }
    Ok(())
}

fn hand_over_stdio(slave: &std::os::fd::OwnedFd) -> Result<(), PinStartError> {
    for target in [libc::STDIN_FILENO, libc::STDOUT_FILENO] {
        // SAFETY: `dup2` duplicates the live slave fd onto a standard fd.
        if unsafe { libc::dup2(slave.as_raw_fd(), target) } < 0 {
            return Err(PinStartError::HandOver(std::io::Error::last_os_error()));
        }
    }
    redirect_stderr()
}

/// Point fd 2 at the application log (opened for append), falling back to
/// `/dev/null`: after the hand-over, native libraries (GTK/GLib/layer-shell)
/// writing to stderr must not draw over the panel. mbv reports fatal
/// post-hand-over errors via the log and a notification (design D5), so the
/// TUI does not need fd 2.
fn redirect_stderr() -> Result<(), PinStartError> {
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(crate::crash_log_path())
        .or_else(|_| std::fs::OpenOptions::new().write(true).open("/dev/null"))
        .map_err(PinStartError::HandOver)?;
    // SAFETY: `dup2` duplicates the fd owned by `file` onto stderr.
    if unsafe { libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) } < 0 {
        return Err(PinStartError::HandOver(std::io::Error::last_os_error()));
    }
    // If fd 2 was closed, the open above may have taken fd 2 itself; never
    // close the target it now aliases.
    if file.as_raw_fd() == libc::STDERR_FILENO {
        std::mem::forget(file);
    }
    Ok(())
}

/// The terminal environment a pinned launch runs with, saved so a failed start
/// can restore it (design D3).
struct PanelEnv {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
}

const PANEL_ENV_SET: [(&str, &str); 2] = [("TERM", "xterm-256color"), ("COLORTERM", "truecolor")];
const PANEL_ENV_REMOVED: [&str; 2] = ["TERM_PROGRAM", "TERM_PROGRAM_VERSION"];

impl PanelEnv {
    fn apply() -> Self {
        let mut saved = Vec::with_capacity(PANEL_ENV_SET.len() + PANEL_ENV_REMOVED.len());
        for (name, _) in PANEL_ENV_SET {
            saved.push((name, std::env::var_os(name)));
        }
        for name in PANEL_ENV_REMOVED {
            saved.push((name, std::env::var_os(name)));
        }
        // SAFETY: this runs before any thread that reads or writes the process
        // environment exists: the pinwin GTK thread is not started yet, and
        // the TUI has not spawned its workers.
        unsafe {
            for (name, value) in PANEL_ENV_SET {
                std::env::set_var(name, value);
            }
            for name in PANEL_ENV_REMOVED {
                std::env::remove_var(name);
            }
        }
        Self { saved }
    }

    fn restore(self) {
        // SAFETY: a failed start leaves no pinwin GTK thread running
        // (upstream addition 3) and the TUI has not spawned its workers, so no
        // other thread reads the environment across these writes.
        unsafe {
            for (name, value) in self.saved {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }
}

/// What a failed pinned start does next (design D5).
#[derive(Debug, PartialEq, Eq)]
enum StartFailureAction {
    /// stdin is a terminal: print one line and keep running in it.
    WarnInTerminal,
    /// No terminal to fall back to: notify and exit 1.
    NotifyAndExit,
}

fn failure_action(stdin_is_terminal: bool) -> StartFailureAction {
    if stdin_is_terminal {
        StartFailureAction::WarnInTerminal
    } else {
        StartFailureAction::NotifyAndExit
    }
}

/// Report a pinned launch that never reached the hand-over (design D5).
pub(crate) fn report_start_failure(reason: &PinStartError) {
    let reason = reason.to_string();
    tracing::warn!(
        name: "startup.pin.failed",
        target: "startup",
        { error = %reason },
        "pinned panel did not start"
    );
    let message = format!("mbv: cannot open the pinned panel: {reason}");
    match failure_action(std::io::stdin().is_terminal()) {
        StartFailureAction::WarnInTerminal => {
            eprintln!("{message}; running in this terminal");
        }
        StartFailureAction::NotifyAndExit => {
            notify(&message);
            std::process::exit(1);
        }
    }
}

/// Report a fatal start-up failure after the hand-over (design D5): the panel
/// is the only output and vanishes with the process, so the reason is logged
/// and sent as a notification instead of printed.
pub(crate) fn fatal(message: impl fmt::Display) -> ! {
    let message = message.to_string();
    tracing::error!(
        name: "startup.fatal",
        target: "startup",
        { error = %message },
        "fatal start-up error"
    );
    if is_pinned() {
        notify(&message);
    } else {
        eprintln!("{message}");
    }
    std::process::exit(1);
}

/// Best-effort `notify-send`, matching the TUI's system notifications; a
/// missing binary is only logged (design D5).
fn notify(message: &str) {
    let result = std::process::Command::new("notify-send")
        .arg("--app-name=mbv")
        .arg("mbv")
        .arg(message)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    match result {
        Ok(status) if status.success() => {}
        Ok(status) => tracing::warn!(
            name: "startup.pin.notify.failed",
            target: "startup",
            code = status.code().unwrap_or(-1),
            "notify-send failed"
        ),
        Err(error) => tracing::warn!(
            name: "startup.pin.notify.failed",
            target: "startup",
            { error = %error },
            "notify-send could not run"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{PinnedWidth, StartFailureAction, failure_action, layout_from_config};
    use mbv_config::PanelConfig;
    use rstest::rstest;

    /// Design D5: a failed pinned start falls back to the terminal only when
    /// there is one; otherwise it notifies and exits 1.
    #[test]
    fn start_failure_falls_back_to_the_terminal_only_for_a_tty_stdin() {
        assert_eq!(failure_action(true), StartFailureAction::WarnInTerminal);
        assert_eq!(failure_action(false), StartFailureAction::NotifyAndExit);
    }

    /// Change `panel-expand-toggle`, design D2/D4: the layout's columns are
    /// the active width's saved value, and `Expanded` stays as saved even
    /// when it is narrower than `cols`.
    #[rstest]
    #[case::collapsed(PinnedWidth::Collapsed, 40, 80, 40)]
    #[case::expanded(PinnedWidth::Expanded, 40, 80, 80)]
    #[case::expanded_narrower_than_collapsed(PinnedWidth::Expanded, 40, 12, 12)]
    fn layout_from_config_uses_the_active_width(
        #[case] width: PinnedWidth,
        #[case] cols: u16,
        #[case] cols_expanded: u16,
        #[case] expected_cols: u16,
    ) {
        let config = PanelConfig {
            cols,
            cols_expanded,
            ..PanelConfig::default()
        };
        assert_eq!(layout_from_config(&config, width).cols, expected_cols);
    }
}
