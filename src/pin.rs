//! The `pinwin` launch: the argv that docks this terminal as the `mbv`
//! panel, the pinned launch and its failure reporting, and the running-panel
//! toggle.
//!
//! The panel identity and every running-panel request live in
//! [`crate::app::pin`]; this module builds the launch argv from the `pub`
//! parts there and claims that identity at start-up. The pinned panel is a
//! process-wide singleton: a second `--pin` shows the running panel.

use std::ffi::OsStr;
use std::fmt;
use std::io::IsTerminal;
use std::process::Command;

use mbv_config::PanelConfig;
use mbv_ctrl::SWAP_TOKEN_ENV;
use mbv_desktop::launch::{
    PANEL_NAME, PinSpawnError, launch_exe, notify, run_request, terminal_prefix, toggle_argv,
};

pub(crate) use crate::app::pin::IN_PANEL_FLAG;
pub use crate::app::pin::{claim_panel_identity, claim_swap_token, is_pinned, swap_token};

use crate::app::pin::{PanelReserveMode, collapsed_reserve, focus_ring_arg, size_arg};

/// Why a pinned launch did not reach `pinwin run`.
#[derive(Debug)]
pub(crate) enum PinStartError {
    /// `WAYLAND_DISPLAY` is unset or empty.
    NoWaylandDisplay,
    /// No terminal argv resolved for the panel host.
    Terminal(String),
    /// The mbv binary could not be located.
    Executable(std::io::Error),
    /// `pinwin` could not be spawned.
    Spawn(PinSpawnError),
}

impl fmt::Display for PinStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWaylandDisplay => f.write_str("WAYLAND_DISPLAY is unset"),
            Self::Terminal(reason) => f.write_str(reason),
            Self::Executable(error) => write!(f, "cannot locate the mbv binary: {error}"),
            Self::Spawn(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for PinStartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Executable(error) => Some(error),
            Self::Spawn(error) => Some(error),
            Self::NoWaylandDisplay | Self::Terminal(_) => None,
        }
    }
}

/// The `pinwin run` argv that docks the terminal as the `mbv` panel and runs
/// mbv in it. `--show-existing` makes a second `--pin`
/// show the running panel instead of opening a second one, and the launcher
/// always passes its four gutters so pinwin's own config never applies them.
#[must_use]
fn run_argv(
    config: &PanelConfig,
    terminal: &[String],
    exe: &OsStr,
    log_level: Option<&str>,
) -> Vec<String> {
    let mut argv: Vec<String> = [
        "run".to_string(),
        "--name".to_string(),
        PANEL_NAME.to_string(),
        "--show-existing".to_string(),
        "--edge".to_string(),
        config.side.as_str().to_string(),
        "--size".to_string(),
        size_arg(config.width),
        "--reserve".to_string(),
        match collapsed_reserve(config) {
            PanelReserveMode::Push => "push",
            PanelReserveMode::Cover => "cover",
        }
        .to_string(),
        "--gutter-top".to_string(),
        config.gutter_top.to_string(),
        "--gutter-bottom".to_string(),
        config.gutter_bottom.to_string(),
        "--gutter-left".to_string(),
        config.gutter_left.to_string(),
        "--gutter-right".to_string(),
        config.gutter_right.to_string(),
        "--focus-ring".to_string(),
        focus_ring_arg(config),
        "--".to_string(),
    ]
    .into();
    argv.extend_from_slice(terminal);
    argv.push(exe.to_string_lossy().into_owned());
    argv.push(IN_PANEL_FLAG.to_string());
    if let Some(log_level) = log_level {
        argv.push("--log-level".to_string());
        argv.push(log_level.to_string());
    }
    argv
}

/// What a finished `pinwin run` reports: a success exits with
/// its status, a failure is always logged, and a desktop launch also
/// notifies because it has no stderr to read.
#[derive(Debug, PartialEq, Eq)]
enum PanelExitAction {
    /// Exit with the child's status and report nothing.
    Quiet,
    /// Log the failure status, then exit with it.
    Log,
    /// Log the failure status, notify, then exit with it.
    LogAndNotify,
}

fn exit_action(success: bool, stdin_is_terminal: bool) -> PanelExitAction {
    match (success, stdin_is_terminal) {
        (true, _) => PanelExitAction::Quiet,
        (false, true) => PanelExitAction::Log,
        (false, false) => PanelExitAction::LogAndNotify,
    }
}

/// Start the panel host and wait for it.
///
/// The pre-checks need no Wayland code: `WAYLAND_DISPLAY`, the terminal argv,
/// the mbv binary and the `pinwin` program itself. Each failure takes the
/// start-failure path, so a terminal launch falls back to this terminal and a
/// desktop launch notifies and exits 1. Once `pinwin run` is spawned there is
/// no fallback: this process waits and exits with the child's status.
pub(crate) fn launch(config: &PanelConfig, log_level: Option<&str>) {
    if std::env::var_os("WAYLAND_DISPLAY").is_none_or(|value| value.is_empty()) {
        report_start_failure(&PinStartError::NoWaylandDisplay);
        return;
    }
    let terminal_env = std::env::var("TERMINAL").ok();
    let terminal = match terminal_prefix(
        config.pin_terminal.as_deref(),
        terminal_env.as_deref(),
        "pin_terminal",
    ) {
        Ok(terminal) => terminal,
        Err(reason) => {
            report_start_failure(&PinStartError::Terminal(reason));
            return;
        }
    };
    let current = match std::env::current_exe() {
        Ok(current) => current,
        Err(error) => {
            report_start_failure(&PinStartError::Executable(error));
            return;
        }
    };
    let exe = launch_exe(&current, std::process::id());
    let mut command = Command::new("pinwin");
    command.args(run_argv(config, &terminal, exe.as_os_str(), log_level));
    if let Some(token) = swap_token() {
        command.env(SWAP_TOKEN_ENV, token);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            report_start_failure(&PinStartError::Spawn(PinSpawnError::from(error)));
            return;
        }
    };
    match child.wait() {
        Ok(status) => finish(status),
        Err(error) => {
            tracing::warn!(
                name: "startup.pin.wait_failed",
                target: "startup",
                { error = %error },
                "pinned panel wait failed"
            );
            if !std::io::stdin().is_terminal() {
                notify(&format!("mbv: cannot wait for the pinned panel: {error}"));
            }
            std::process::exit(1);
        }
    }
}

/// Exit with the child's status, logging and, on a desktop launch, notifying
/// a failure.
fn finish(status: std::process::ExitStatus) -> ! {
    let action = exit_action(status.success(), std::io::stdin().is_terminal());
    if action != PanelExitAction::Quiet {
        tracing::warn!(
            name: "startup.pin.exited",
            target: "startup",
            status = %status,
            "pinwin run ended with a failure status"
        );
    }
    if action == PanelExitAction::LogAndNotify {
        let reason = match status.code() {
            Some(code) => format!("the pinned panel could not open (status {code})"),
            None => "the pinned panel could not open (it ended by a signal)".to_string(),
        };
        notify(&format!("mbv: {reason}"));
    }
    std::process::exit(status.code().unwrap_or(1));
}

/// Ask pinwin on the current Wayland display to toggle the `mbv` panel:
/// hide it if shown, show it if hidden. Exits 1 with pinwin's
/// reason when no panel runs or the request fails; exits 0 silently on
/// success.
pub(crate) fn toggle_running() {
    if let Err(error) = run_request(&toggle_argv(PANEL_NAME), None) {
        report_toggle_failure(&error.to_string());
    }
}

/// Report a failed `--toggle` and exit 1: stderr when stdin is a
/// terminal, otherwise a desktop notification, because a compositor key has
/// no terminal.
fn report_toggle_failure(reason: &str) -> ! {
    let message = format!("mbv: {reason}");
    match failure_action(std::io::stdin().is_terminal()) {
        StartFailureAction::WarnInTerminal => {
            eprintln!("{message}");
        }
        StartFailureAction::NotifyAndExit => {
            notify(&message);
        }
    }
    std::process::exit(1);
}

/// What a failed pinned start does next.
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

/// Report a pinned launch that never reached `pinwin run`.
fn report_start_failure(reason: &PinStartError) {
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

/// Report a fatal start-up failure of the TUI inside the panel:
/// the panel is the only output and vanishes with the process, so the reason
/// is logged and sent as a notification instead of printed.
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

#[cfg(test)]
mod tests;
