//! The `pinwin` CLI boundary: the panel identity, the argv of every panel
//! request, and the pinned launch and its failure reporting.
//!
//! The pinned panel is a process-wide singleton: `pinwin` allows one panel at
//! a time and a second `--pin` shows the running panel. Every panel request
//! leaves this process as one `pinwin` child, and the panel identity is
//! claimed from the environment at start-up.

use std::ffi::OsStr;
use std::fmt;
use std::io::IsTerminal;
use std::process::Command;
use std::sync::OnceLock;

use mbv_config::PanelConfig;
use mbv_ctrl::SWAP_TOKEN_ENV;
use mbv_desktop::launch::{
    PANEL_NAME, PinSpawnError, launch_exe, notify, run_request, terminal_prefix,
};
use mbv_ui_model::settings::PinnedWidth;
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

/// The hidden launch flag that marks an mbv pinwin started inside a panel.
/// It is parsed but never shown in `--help`; a panel environment without it
/// leaves the launch unpinned.
pub(crate) const IN_PANEL_FLAG: &str = "--in-panel";

/// The variable pinwin sets on the Client it runs inside a panel: the name of
/// that panel.
const PINWIN_PANEL_ENV: &str = "PINWIN_PANEL";

/// The variable pinwin sets beside [`PINWIN_PANEL_ENV`]: the daemon socket
/// that serves that panel.
const PINWIN_SOCKET_ENV: &str = "PINWIN_SOCKET";

/// The panel identity this launch was started with.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PanelIdentity {
    /// The panel name, `None` when this launch runs outside a panel.
    name: Option<String>,
    /// The daemon socket that serves that panel.
    socket: Option<String>,
}

impl PanelIdentity {
    /// Map the raw `PINWIN_PANEL` and `PINWIN_SOCKET` values and the hidden
    /// launch flag to the identity: a non-empty panel name means pinned only
    /// together with the flag, and an empty value counts as unset. The socket
    /// is kept either way; it is scrubbed from the environment whether this
    /// launch is pinned or not.
    fn new(panel: Option<String>, socket: Option<String>, in_panel: bool) -> Self {
        Self {
            name: panel.filter(|name| !name.is_empty()).filter(|_| in_panel),
            socket: socket.filter(|socket| !socket.is_empty()),
        }
    }
}

/// The panel identity this launch was started with, claimed once at start-up.
static PANEL_IDENTITY: OnceLock<PanelIdentity> = OnceLock::new();

/// Whether this launch runs in a pinned panel.
#[must_use]
pub(crate) fn is_pinned() -> bool {
    identity().name.is_some()
}

/// Read `name` from the environment and remove it from this process's
/// environment, so no spawned child inherits it. Called once at start-up, on
/// the main thread before any thread exists.
fn claim_env_once(name: &str) -> Option<String> {
    let value = std::env::var(name).ok();
    // SAFETY: this runs on the main thread before any thread exists, so no
    // other thread can observe the environment mid-update.
    unsafe { std::env::remove_var(name) };
    value
}

/// Read `PINWIN_PANEL` and `PINWIN_SOCKET` and remove both from this
/// process's environment. Called once at start-up, on the main
/// thread next to [`claim_swap_token`], before any worker or child process
/// spawns, so the Owner process and every other child run unpinned. The
/// launch is pinned only when `in_panel` (the hidden launch flag) is also
/// set, but the variables are scrubbed either way.
pub(crate) fn claim_panel_identity(in_panel: bool) {
    let panel = claim_env_once(PINWIN_PANEL_ENV);
    let socket = claim_env_once(PINWIN_SOCKET_ENV);
    let _ = PANEL_IDENTITY.set(PanelIdentity::new(panel, socket, in_panel));
}

/// The claimed panel identity, the empty identity when nothing was claimed.
fn identity() -> &'static PanelIdentity {
    PANEL_IDENTITY.get_or_init(PanelIdentity::default)
}

/// The swap token this launch was started with, claimed once at startup.
/// `None` for every launch the user started.
static SWAP_TOKEN: OnceLock<Option<String>> = OnceLock::new();

/// Read `MBV_SWAP_TOKEN` and remove it from this process's environment.
/// Called once, before any worker or child process
/// spawns, so the token is neither read twice nor inherited by a spawned
/// daemon.
pub(crate) fn claim_swap_token() {
    let _ = SWAP_TOKEN.set(claim_env_once(SWAP_TOKEN_ENV));
}

/// The claimed swap token, `None` for every launch the user started.
#[must_use]
pub(crate) fn swap_token() -> Option<String> {
    SWAP_TOKEN.get().cloned().flatten()
}

pub use self::swap::claim_pin_swap_participant;
pub(crate) use self::swap::pin_swap_hello;
mod swap;

/// The reserve mode a panel change asks for: the collapsed panel
/// pushes the tiled windows aside, the expanded one covers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelReserveMode {
    /// Reserve a strip beside the panel and move the tiled windows.
    Push,
    /// Draw over the tiled windows, holding the strip of the last push.
    Cover,
}

impl PanelReserveMode {
    /// The mode the `[panel] reserve` flag picks: `Push` while it is on,
    /// `Cover` while it is off.
    #[must_use]
    pub(crate) fn for_flag(reserve: bool) -> Self {
        if reserve { Self::Push } else { Self::Cover }
    }

    /// The `--reserve` value.
    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Cover => "cover",
        }
    }
}

/// One of the panel's four gutters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelGutter {
    /// Inset from the output's top edge.
    Top,
    /// Inset from the output's bottom edge.
    Bottom,
    /// Inset from the output's left edge.
    Left,
    /// Inset from the output's right edge.
    Right,
}

impl PanelGutter {
    /// The `set-gutters` option that carries this gutter.
    #[must_use]
    fn flag(self) -> &'static str {
        match self {
            Self::Top => "--top",
            Self::Bottom => "--bottom",
            Self::Left => "--left",
            Self::Right => "--right",
        }
    }
}

/// The one `pinwin` request a Panel row sends while pinned. A width row asks
/// for the width it edits, so
/// the edited value is the width pinwin validates; a side or gutter row applies
/// at the panel's current width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PinnedSettingRequest {
    /// `Width` (`Collapsed`) or `Expanded width` (`Expanded`): one `set-size`
    /// with the candidate width and its reserve mode.
    Width(PinnedWidth),
    /// `Side`: one `set-edge`.
    Side(mbv_config::PanelSide),
    /// A gutter row: one `set-gutters` that carries only the edited gutter.
    Gutter(PanelGutter, i32),
    /// The `Reserve space` row: one `set-reserve` with the candidate reserve
    /// mode, so an off collapse stops pushing and an on collapse reserves the
    /// strip at the panel's current size.
    Reserve(PanelReserveMode),
}

impl PinnedSettingRequest {
    /// The pinned width this request validates, `None` for a row that applies
    /// at the panel's current width.
    #[must_use]
    pub(crate) fn edited_width(self) -> Option<PinnedWidth> {
        match self {
            Self::Width(width) => Some(width),
            Self::Side(_) | Self::Gutter(..) | Self::Reserve(_) => None,
        }
    }
}

/// A width in the pinwin size grammar: logical pixels.
#[must_use]
fn size_arg(width: u16) -> String {
    format!("{width}px")
}

/// The reserve mode the collapsed panel asks for: the `[panel] reserve`
/// flag's mode ([`PanelReserveMode::for_flag`]). The expanded panel's mode
/// is always `Cover`; no config key picks it.
#[must_use]
fn collapsed_reserve(config: &PanelConfig) -> PanelReserveMode {
    PanelReserveMode::for_flag(config.reserve)
}

/// The `--focus-ring` value of a `run`: `off`, or the configured
/// colour and width. Only a launch reads it; a running panel keeps its ring.
#[must_use]
fn focus_ring_arg(config: &PanelConfig) -> String {
    if config.accent {
        format!("{}:{}", config.accent_color, config.accent_width)
    } else {
        "off".to_string()
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
        collapsed_reserve(config).as_str().to_string(),
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

/// `set-size <panel> <N>px --reserve push|cover`: one press of `x`
/// and one F2 width step each send one change with the width and the reserve
/// mode, so no tiled window moves.
#[must_use]
fn set_size_argv(panel: &str, width: u16, reserve: PanelReserveMode) -> Vec<String> {
    vec![
        "set-size".to_string(),
        panel.to_string(),
        size_arg(width),
        "--reserve".to_string(),
        reserve.as_str().to_string(),
    ]
}

/// The `set-size` argv one pinned-view change sends: the
/// collapsed view pushes at `width`, the expanded view covers at
/// `width_expanded`, so one press changes the width and the reserve mode in
/// one request.
#[must_use]
fn pinned_width_argv(panel: &str, config: &PanelConfig, width: PinnedWidth) -> Vec<String> {
    let (pixels, reserve) = match width {
        PinnedWidth::Collapsed => (config.width, collapsed_reserve(config)),
        PinnedWidth::Expanded => (config.width_expanded, PanelReserveMode::Cover),
    };
    set_size_argv(panel, pixels, reserve)
}

/// The claimed panel name of this launch, or the reason there is none.
fn panel_name() -> Result<&'static str, String> {
    identity()
        .name
        .as_deref()
        .ok_or_else(|| "this launch is not pinned".to_string())
}

/// Start one panel request on a worker thread. The argv is built on the
/// caller's thread, so an unpinned launch reports its reason before any
/// worker exists; only the `pinwin` child runs behind the worker and its
/// outcome comes back on `tx`. At most one request is in flight because the
/// caller holds the pending record that ignores further presses.
pub(crate) fn spawn_pinned_request(
    config: &PanelConfig,
    request: PinnedSettingRequest,
    tx: std::sync::mpsc::Sender<Result<(), String>>,
) -> Result<(), String> {
    #[cfg(test)]
    if let Some(outcome) = PINNED_REQUEST_OVERRIDE.lock().unwrap().clone() {
        PINNED_REQUESTS.lock().unwrap().push(request);
        let _ = tx.send(outcome);
        return Ok(());
    }
    let panel = panel_name()?;
    let argv = match request {
        PinnedSettingRequest::Width(width) => pinned_width_argv(panel, config, width),
        PinnedSettingRequest::Side(side) => set_edge_argv(panel, side),
        PinnedSettingRequest::Gutter(gutter, value) => set_gutters_argv(panel, gutter, value),
        PinnedSettingRequest::Reserve(reserve) => set_reserve_argv(panel, reserve),
    };
    std::thread::Builder::new()
        .name("pinwin-request".to_string())
        .spawn(move || {
            let outcome =
                run_request(&argv, identity().socket.as_deref()).map_err(|error| error.to_string());
            let _ = tx.send(outcome);
        })
        .map(|_| ())
        .map_err(|error| format!("pinwin request worker could not start: {error}"))
}

/// Test seam: when set, `spawn_pinned_request` records the request and
/// delivers this injected outcome on the caller's thread instead of running
/// the `pinwin` child. Test callers hold [`PINNED_REQUEST_TEST_LOCK`].
#[cfg(test)]
pub(crate) static PINNED_REQUEST_OVERRIDE: std::sync::Mutex<Option<Result<(), String>>> =
    std::sync::Mutex::new(None);

/// The requests the test seam delivered, in order.
#[cfg(test)]
pub(crate) static PINNED_REQUESTS: std::sync::Mutex<Vec<PinnedSettingRequest>> =
    std::sync::Mutex::new(Vec::new());

/// Serializes tests that install [`PINNED_REQUEST_OVERRIDE`].
#[cfg(test)]
pub(crate) static PINNED_REQUEST_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `set-edge <panel> left|right`.
#[must_use]
fn set_edge_argv(panel: &str, side: mbv_config::PanelSide) -> Vec<String> {
    vec![
        "set-edge".to_string(),
        panel.to_string(),
        side.as_str().to_string(),
    ]
}

/// `set-gutters <panel> --<gutter> N`: the F2 row edits one
/// gutter, so one gutter changes at the panel's current width.
#[must_use]
fn set_gutters_argv(panel: &str, gutter: PanelGutter, value: i32) -> Vec<String> {
    vec![
        "set-gutters".to_string(),
        panel.to_string(),
        gutter.flag().to_string(),
        value.to_string(),
    ]
}

/// `set-reserve <panel> push|cover`: the `Reserve space` row sends the mode
/// resolved from the saved `reserve` flag, so the panel's reserve mode
/// changes at its current size; a cover holds any strip a push already
/// reserved and reserves nothing new.
#[must_use]
fn set_reserve_argv(panel: &str, reserve: PanelReserveMode) -> Vec<String> {
    vec![
        "set-reserve".to_string(),
        panel.to_string(),
        reserve.as_str().to_string(),
    ]
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
    // The panel host owns no socket of its own: it was started by the Client
    // whose `PINWIN_SOCKET` that Client claimed and removed.
    if let Some(socket) = identity().socket.as_deref() {
        command.env(PINWIN_SOCKET_ENV, socket);
    }
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
