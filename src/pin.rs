//! The `pinwin` CLI boundary: the panel identity (change `pinwin-app-host`,
//! design D3), the argv of every panel request (design D4 to D8), and the
//! pinned launch and its failure reporting (design D1, D2).
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

use crate::launch::launch_exe;
use mbv_config::PanelConfig;
use mbv_ctrl::SWAP_TOKEN_ENV;
use mbv_ui_model::settings::PinnedWidth;

/// Why the `pinwin` program could not be spawned. A missing binary asks for an
/// install; any other failure names its cause.
#[derive(Debug)]
pub(crate) struct PinSpawnError(std::io::Error);

impl PinSpawnError {
    fn new(error: std::io::Error) -> Self {
        Self(error)
    }
}

impl fmt::Display for PinSpawnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.kind() == std::io::ErrorKind::NotFound {
            f.write_str("pinwin is not installed")
        } else {
            write!(f, "pinwin could not start: {}", self.0)
        }
    }
}

impl std::error::Error for PinSpawnError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// Why a pinned launch did not reach `pinwin run` (design D2, D5).
#[derive(Debug)]
pub(crate) enum PinStartError {
    /// `WAYLAND_DISPLAY` is unset or empty.
    NoWaylandDisplay,
    /// No terminal argv resolved for the panel host (design D1).
    Terminal(String),
    /// The mbv binary could not be located.
    Executable(std::io::Error),
    /// `pinwin` could not be spawned (design D2).
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

/// Why one `pinwin` request did not finish.
#[derive(Debug)]
enum PinRequestError {
    /// The `pinwin` program could not be spawned.
    Spawn(PinSpawnError),
    /// `pinwin` ran and refused the request.
    Refused(String),
}

impl fmt::Display for PinRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(error) => error.fmt(f),
            Self::Refused(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for PinRequestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Spawn(error) => Some(error),
            Self::Refused(_) => None,
        }
    }
}

/// The panel name every pinned mbv runs under (design D1, D8).
pub(crate) const PANEL_NAME: &str = "mbv";

/// The variable pinwin sets on the Client it runs inside a panel: the name of
/// that panel (design D3).
const PINWIN_PANEL_ENV: &str = "PINWIN_PANEL";

/// The variable pinwin sets beside [`PINWIN_PANEL_ENV`]: the daemon socket
/// that serves that panel (design D3).
const PINWIN_SOCKET_ENV: &str = "PINWIN_SOCKET";

/// The panel identity this launch was started with (design D3).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct PanelIdentity {
    /// The panel name, `None` when this launch runs outside a panel.
    name: Option<String>,
    /// The daemon socket that serves that panel.
    socket: Option<String>,
}

impl PanelIdentity {
    /// Map the raw `PINWIN_PANEL` and `PINWIN_SOCKET` values to the identity
    /// (design D3): a non-empty panel name means pinned, and an empty value
    /// counts as unset.
    fn new(panel: Option<String>, socket: Option<String>) -> Self {
        Self {
            name: panel.filter(|name| !name.is_empty()),
            socket: socket.filter(|socket| !socket.is_empty()),
        }
    }
}

/// The panel identity this launch was started with, claimed once at start-up.
static PANEL_IDENTITY: OnceLock<PanelIdentity> = OnceLock::new();

/// Whether this launch runs in a pinned panel (design D3).
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
/// process's environment (design D3). Called once at start-up, on the main
/// thread next to [`claim_swap_token`], before any worker or child process
/// spawns, so the Owner process and every other child run unpinned.
pub(crate) fn claim_panel_identity() {
    let panel = claim_env_once(PINWIN_PANEL_ENV);
    let socket = claim_env_once(PINWIN_SOCKET_ENV);
    let _ = PANEL_IDENTITY.set(PanelIdentity::new(panel, socket));
}

/// The claimed panel identity, the empty identity when nothing was claimed.
fn identity() -> &'static PanelIdentity {
    PANEL_IDENTITY.get_or_init(PanelIdentity::default)
}

/// The swap token this launch was started with, claimed once at startup.
/// `None` for every launch the user started.
static SWAP_TOKEN: OnceLock<Option<String>> = OnceLock::new();

/// Read `MBV_SWAP_TOKEN` and remove it from this process's environment
/// (tray-pin-swap design D2). Called once, before any worker or child process
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

/// The reserve mode a panel change asks for (design D6): the collapsed panel
/// pushes the tiled windows aside, the expanded one covers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelReserve {
    /// Reserve a strip beside the panel and move the tiled windows.
    Push,
    /// Draw over the tiled windows, holding the strip of the last push.
    Cover,
}

impl PanelReserve {
    /// The `--reserve` value.
    #[must_use]
    fn as_str(self) -> &'static str {
        match self {
            Self::Push => "push",
            Self::Cover => "cover",
        }
    }
}

/// One of the panel's four gutters (design D5, D7).
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

/// The one `pinwin` request a Panel row sends while pinned (change
/// `pinwin-app-host`, design D7). A width row asks for the width it edits, so
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
}

impl PinnedSettingRequest {
    /// The pinned width this request validates, `None` for a row that applies
    /// at the panel's current width.
    #[must_use]
    pub(crate) fn edited_width(self) -> Option<PinnedWidth> {
        match self {
            Self::Width(width) => Some(width),
            Self::Side(_) | Self::Gutter(..) => None,
        }
    }
}

/// A width in the pinwin size grammar: logical pixels (design D5).
#[must_use]
fn size_arg(width: u16) -> String {
    format!("{width}px")
}

/// The `--focus-ring` value of a `run` (design D5): `off`, or the configured
/// colour and width. Only a launch reads it; a running panel keeps its ring.
#[must_use]
fn focus_ring_arg(config: &PanelConfig) -> String {
    if config.accent {
        format!("{}:{}", config.accent_color, config.accent_width)
    } else {
        "off".to_string()
    }
}

/// The terminal argv prefix that hosts a Client (design D1): the configured
/// prefix when set, else `TERMINAL -e`. With neither set the launch fails with
/// a reason that names both sources; `config_key` is the configuration key the
/// caller reads the prefix from.
pub(crate) fn terminal_prefix(
    configured: Option<&[String]>,
    terminal_env: Option<&str>,
    config_key: &str,
) -> Result<Vec<String>, String> {
    let prefix = configured.filter(|argv| !argv.is_empty());
    let terminal = terminal_env.filter(|terminal| !terminal.trim().is_empty());
    match (prefix, terminal) {
        (Some(prefix), _) => Ok(prefix.to_vec()),
        (None, Some(terminal)) => Ok(vec![terminal.to_string(), "-e".to_string()]),
        (None, None) => Err(format!(
            "neither [panel] {config_key} (config) nor TERMINAL (environment) is set"
        )),
    }
}

/// The `pinwin run` argv that docks the terminal as the `mbv` panel and runs
/// mbv in it (design D1, D2, D8). `--show-existing` makes a second `--pin`
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
        PanelReserve::Push.as_str().to_string(),
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
    if let Some(log_level) = log_level {
        argv.push("--log-level".to_string());
        argv.push(log_level.to_string());
    }
    argv
}

/// `set-size <panel> <N>px --reserve push|cover` (design D6): one press of `x`
/// and one F2 width step each send one change with the width and the reserve
/// mode, so no tiled window moves.
#[must_use]
fn set_size_argv(panel: &str, width: u16, reserve: PanelReserve) -> Vec<String> {
    vec![
        "set-size".to_string(),
        panel.to_string(),
        size_arg(width),
        "--reserve".to_string(),
        reserve.as_str().to_string(),
    ]
}

/// The `set-size` argv one pinned-view change sends (design D6): the
/// collapsed view pushes at `width`, the expanded view covers at
/// `width_expanded`, so one press changes the width and the reserve mode in
/// one request.
#[must_use]
fn pinned_width_argv(panel: &str, config: &PanelConfig, width: PinnedWidth) -> Vec<String> {
    let (pixels, reserve) = match width {
        PinnedWidth::Collapsed => (config.width, PanelReserve::Push),
        PinnedWidth::Expanded => (config.width_expanded, PanelReserve::Cover),
    };
    set_size_argv(panel, pixels, reserve)
}

/// The claimed panel name of this launch, or the reason there is none
/// (design D3, D4).
fn panel_name() -> Result<&'static str, String> {
    identity()
        .name
        .as_deref()
        .ok_or_else(|| "this launch is not pinned".to_string())
}

/// Send one panel request to the running panel (design D6, D7). The requested
/// view decides the width request, never the terminal size, so a resize mbv did
/// not request leaves [`PinnedWidth`] untouched. An unpinned launch, or a
/// refused request, reports the reason and changes nothing.
pub(crate) fn apply_pinned(
    config: &PanelConfig,
    request: PinnedSettingRequest,
) -> Result<(), String> {
    let panel = panel_name()?;
    let argv = match request {
        PinnedSettingRequest::Width(width) => pinned_width_argv(panel, config, width),
        PinnedSettingRequest::Side(side) => set_edge_argv(panel, side),
        PinnedSettingRequest::Gutter(gutter, value) => set_gutters_argv(panel, gutter, value),
    };
    run_request(&argv).map_err(|error| error.to_string())
}

/// `set-edge <panel> left|right` (design D7).
#[must_use]
fn set_edge_argv(panel: &str, side: mbv_config::PanelSide) -> Vec<String> {
    vec![
        "set-edge".to_string(),
        panel.to_string(),
        side.as_str().to_string(),
    ]
}

/// `set-gutters <panel> --<gutter> N` (design D7): the F2 row edits one
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

/// `toggle <panel>` (design D8).
#[must_use]
fn toggle_argv(panel: &str) -> Vec<String> {
    vec!["toggle".to_string(), panel.to_string()]
}

/// The result a finished `pinwin` request reports (design D4): exit 0 is
/// success, exit 1 is pinwin's own reason from stderr, exit 2 is a usage
/// mismatch between mbv and pinwin, and any other ending is a failure that
/// names it.
fn request_result(code: Option<i32>, stderr: &str) -> Result<(), String> {
    match code {
        Some(0) => Ok(()),
        Some(1) => {
            let reason = stderr.trim();
            if reason.is_empty() {
                Err("pinwin refused the request".to_string())
            } else {
                Err(reason.to_string())
            }
        }
        Some(2) => Err(
            "pinwin rejected the command line: mbv and pinwin disagree on the pinwin CLI"
                .to_string(),
        ),
        Some(code) => Err(format!("pinwin exited with status {code}")),
        None => Err("pinwin ended by a signal".to_string()),
    }
}

/// Run one `pinwin` request against the claimed panel daemon (design D4). The
/// child is the untested boundary; the argv it runs and the result it maps to
/// are pure.
fn run_request(argv: &[String]) -> Result<(), PinRequestError> {
    let mut command = Command::new("pinwin");
    command.args(argv);
    if let Some(socket) = identity().socket.as_deref() {
        command.env(PINWIN_SOCKET_ENV, socket);
    }
    let output = command
        .output()
        .map_err(|error| PinRequestError::Spawn(PinSpawnError::new(error)))?;
    request_result(
        output.status.code(),
        &String::from_utf8_lossy(&output.stderr),
    )
    .map_err(PinRequestError::Refused)
}

/// What a finished `pinwin run` reports (design D2): a success exits with
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

/// Start the panel host and wait for it (design D1, D2).
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
            report_start_failure(&PinStartError::Spawn(PinSpawnError::new(error)));
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
/// a failure (design D2).
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
            Some(code) => format!("the pinned panel ended with status {code}"),
            None => "the pinned panel ended by a signal".to_string(),
        };
        notify(&format!("mbv: {reason}"));
    }
    std::process::exit(status.code().unwrap_or(1));
}

/// Ask pinwin on the current Wayland display to toggle the `mbv` panel
/// (design D8): hide it if shown, show it if hidden. Exits 1 with pinwin's
/// reason when no panel runs or the request fails; exits 0 silently on
/// success.
pub(crate) fn toggle_running() {
    if let Err(error) = run_request(&toggle_argv(PANEL_NAME)) {
        report_toggle_failure(&error.to_string());
    }
}

/// Report a failed `--toggle` and exit 1 (design D3): stderr when stdin is a
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

/// Report a pinned launch that never reached `pinwin run` (design D2, D5).
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

/// Report a fatal start-up failure of the TUI inside the panel (design D5):
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

/// Best-effort `notify-send`, matching the TUI's system notifications; a
/// missing binary is only logged (design D5). Also the swap-failure
/// notification path for the daemon's `notify` hook (`local_daemon.rs`,
/// tray-pin-swap D5).
pub(crate) fn notify(message: &str) {
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
mod tests;
