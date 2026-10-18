//! `--pin` start-up: pty hand-over, panel lifetime and failure reporting
//! (change `pin-mbv-in-pinwin`, design D3/D5), and the `pinwin` CLI boundary
//! (change `pinwin-app-host`, design D3/D4).
//!
//! The pinned panel is a process-wide singleton: `pinwin` allows one panel at
//! a time, so a launch either completes the stdio hand-over once or the TUI
//! keeps running in the terminal it was started from. Every later panel
//! request leaves this process as one `pinwin` child, and the panel identity
//! is claimed from the environment at start-up.

use std::ffi::OsStr;
use std::fmt;
use std::io::IsTerminal;
use std::num::NonZeroU16;
use std::os::fd::{AsRawFd, FromRawFd};
use std::process::Command;
use std::sync::OnceLock;

use mbv_config::PanelConfig;
use mbv_ctrl::SWAP_TOKEN_ENV;
use pinwin::instance::{InstanceError, InstanceName, InstanceSocket, Request, SendError};
use pinwin::layout::{Accent, Keyboard, Layout, Side};
use pinwin::panel::Startup;

/// The running panel handle the shell owns.
pub(crate) type PinnedPanel = pinwin::Panel;

/// The fixed instance name every pinned mbv listens under (design D1):
/// pinwin scopes the socket path by `WAYLAND_DISPLAY`, so this names one
/// pinned panel per display.
fn instance_name() -> InstanceName {
    InstanceName::parse("mbv").expect("the fixed instance name is valid")
}

/// The animated apply duration (design D3): the C ABI wrapper carried this
/// default, the Rust crate only clamps it, so mbv owns the value.
const ANIM_DEFAULT_MS: u32 = 200;

/// Which of the two saved widths the running panel shows (design D1). The
/// active width is runtime state on `App`; it is never persisted, so every
/// pinned launch starts collapsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum PinnedWidth {
    #[default]
    Collapsed,
    Expanded,
}

impl PinnedWidth {
    /// The other width (design D3).
    #[must_use]
    pub(crate) fn toggled(self) -> Self {
        match self {
            Self::Collapsed => Self::Expanded,
            Self::Expanded => Self::Collapsed,
        }
    }
}

/// Apply a full `[panel]` layout to the live panel at `width` (design D6).
pub(crate) fn apply_layout(
    panel: &PinnedPanel,
    config: &PanelConfig,
    width: PinnedWidth,
) -> Result<(), String> {
    panel
        .apply_layout_animated(layout_from_config(config, width), ANIM_DEFAULT_MS)
        .map_err(|error| error.to_string())
}

/// What a pinned launch did (design D2).
#[derive(Debug)]
pub(crate) enum PinLaunch {
    /// This process owns the panel.
    Started(PinnedPanel),
    /// A pinned mbv already runs on this display and was asked to show.
    ShownExisting,
}

/// Why a pinned launch did not reach the stdio hand-over (design D5).
#[derive(Debug)]
pub(crate) enum PinStartError {
    /// `WAYLAND_DISPLAY` is unset or empty.
    NoWaylandDisplay,
    /// The instance socket could not be bound.
    Instance(InstanceError),
    /// A running pinned mbv did not answer the show request.
    ShowExisting(SendError),
    /// The pty pair could not be opened.
    Pty(std::io::Error),
    /// `pinwin` refused to start the panel.
    Panel(pinwin::PinwinError),
    /// The stdio hand-over failed after the panel started.
    HandOver(std::io::Error),
}

impl fmt::Display for PinStartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoWaylandDisplay => f.write_str("WAYLAND_DISPLAY is unset"),
            Self::Instance(error) => write!(f, "the instance socket could not be bound: {error}"),
            Self::ShowExisting(error) => {
                write!(f, "the running pinned panel did not show: {error}")
            }
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
            Self::Instance(error) => Some(error),
            Self::ShowExisting(error) => Some(error),
            Self::NoWaylandDisplay => None,
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

    /// Whether this launch runs in a pinned panel.
    #[must_use]
    fn is_pinned(&self) -> bool {
        self.name.is_some()
    }
}

/// The panel identity this launch was started with, claimed once at start-up.
static PANEL_IDENTITY: OnceLock<PanelIdentity> = OnceLock::new();

/// Whether this launch runs in a pinned panel (design D3).
#[must_use]
pub(crate) fn is_pinned() -> bool {
    identity().is_pinned()
}

/// Read `PINWIN_PANEL` and `PINWIN_SOCKET` and remove both from this
/// process's environment (design D3). Called once at start-up, on the main
/// thread next to [`claim_swap_token`], before any worker or child process
/// spawns, so the Owner process and every other child run unpinned.
pub(crate) fn claim_panel_identity() {
    let panel = std::env::var(PINWIN_PANEL_ENV).ok();
    let socket = std::env::var(PINWIN_SOCKET_ENV).ok();
    // SAFETY: this runs on the main thread before any thread exists, so no
    // other thread can observe the environment mid-update.
    unsafe { std::env::remove_var(PINWIN_PANEL_ENV) };
    // SAFETY: as above, and the second removal is still before any thread
    // exists.
    unsafe { std::env::remove_var(PINWIN_SOCKET_ENV) };
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
    let token = std::env::var(SWAP_TOKEN_ENV).ok();
    // SAFETY: this runs on the main thread before any thread exists, so no
    // other thread can observe the environment mid-update.
    unsafe { std::env::remove_var(SWAP_TOKEN_ENV) };
    let _ = SWAP_TOKEN.set(token);
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

/// The terminal argv prefix that hosts the panel (design D1): the `[panel]
/// pin_terminal` value when set, else `TERMINAL -e`. With neither set the
/// launch fails with a reason that names both sources.
fn terminal_argv(
    pin_terminal: Option<&[String]>,
    terminal_env: Option<&str>,
) -> Result<Vec<String>, String> {
    let prefix = pin_terminal.filter(|argv| !argv.is_empty());
    let terminal = terminal_env.filter(|terminal| !terminal.trim().is_empty());
    match (prefix, terminal) {
        (Some(prefix), _) => Ok(prefix.to_vec()),
        (None, Some(terminal)) => Ok(vec![terminal.to_string(), "-e".to_string()]),
        (None, None) => Err(
            "cannot start the pinned panel: neither [panel] pin_terminal \
             (config) nor TERMINAL (environment) is set"
                .to_string(),
        ),
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
fn request(argv: &[String]) -> Result<(), String> {
    let mut command = Command::new("pinwin");
    command.args(argv);
    if let Some(socket) = identity().socket.as_deref() {
        command.env(PINWIN_SOCKET_ENV, socket);
    }
    let Ok(output) = command.output() else {
        return Err("pinwin is not installed".to_string());
    };
    request_result(
        output.status.code(),
        &String::from_utf8_lossy(&output.stderr),
    )
}

/// Start the pinned panel and hand this process's stdio to its pty (design
/// D2). The Wayland check comes first, then the instance socket bind: a
/// duplicate leaves the environment, the pty and the terminal untouched, and
/// only asks the running panel to show. The terminal environment is restored
/// if anything fails after the bind completes.
pub(crate) fn start(config: &PanelConfig) -> Result<PinLaunch, PinStartError> {
    if std::env::var_os("WAYLAND_DISPLAY").is_none_or(|value| value.is_empty()) {
        return Err(PinStartError::NoWaylandDisplay);
    }
    let name = instance_name();
    let socket = match InstanceSocket::bind(&name) {
        Ok(socket) => socket,
        // A pinned mbv already owns the name on this display: ask it to
        // show and report that no second panel opened (design D2).
        Err(InstanceError::Duplicate) => {
            return match pinwin::instance::send(&name, Request::Show) {
                Ok(()) => Ok(PinLaunch::ShownExisting),
                Err(error) => Err(PinStartError::ShowExisting(error)),
            };
        }
        Err(error) => return Err(PinStartError::Instance(error)),
    };
    let saved_env = PanelEnv::apply();
    match start_panel(config, socket) {
        Ok(panel) => Ok(PinLaunch::Started(panel)),
        Err(error) => {
            saved_env.restore();
            Err(error)
        }
    }
}

/// The pty pair, the panel, and the stdio hand-over. A failure after
/// `Panel::start` succeeded drops the panel, which stops it and removes the
/// instance socket file (design D3).
fn start_panel(config: &PanelConfig, socket: InstanceSocket) -> Result<PinnedPanel, PinStartError> {
    let (master, slave) = open_pty()?;
    // The panel reads the master fd for its whole life and the library never
    // closes it (design D7), so the fd must stay open even though `Panel` no
    // longer borrows it: the owned fd is leaked for the life of the process,
    // which the panel's whole life fits inside.
    let master: &'static std::os::fd::OwnedFd = Box::leak(Box::new(master));
    let panel = pinwin::Panel::start(
        Startup::new(
            master.as_raw_fd(),
            layout_from_config(config, PinnedWidth::default()),
            Keyboard::OnDemand,
            config
                .accent
                .then(|| Accent::new(config.accent_color.0, config.accent_width)),
        )
        // A panel started with a socket serves `toggle` and `show` on a
        // detached thread until the `Panel` drops (design D2).
        .with_instance(socket),
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
    Ok(panel)
}

/// Ask the running pinned mbv on this display to toggle its panel (design
/// D3): hide it if shown, show it if hidden. Exits 1 with a report when no
/// panel is listening or the request fails; exits 0 silently on success.
pub(crate) fn toggle_running() {
    match pinwin::instance::send(&instance_name(), Request::Toggle) {
        Ok(()) => {}
        Err(SendError::NotListening { .. }) => {
            report_toggle_failure("no pinned mbv is running");
        }
        // `SendError` is non-exhaustive, so mbv reports its `Display` and
        // does not re-classify it.
        Err(error) => {
            report_toggle_failure(&format!("cannot toggle the pinned panel: {error}"));
        }
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

fn layout_from_config(config: &PanelConfig, width: PinnedWidth) -> Layout {
    let side = match config.side {
        mbv_config::PanelSide::Left => Side::Left,
        mbv_config::PanelSide::Right => Side::Right,
    };
    // The pinwin 0.3.4 layout takes terminal columns; `[panel]` now holds
    // logical pixels, and the pinwin CLI `--size <N>px` replaces this call in
    // this change (design D5, D6).
    let cols = match width {
        PinnedWidth::Collapsed => config.width,
        PinnedWidth::Expanded => config.width_expanded,
    };
    let layout = Layout::new(
        side,
        // The fallback is unreachable: config clamps panel widths to at
        // least one column on parse and on every F2 step.
        NonZeroU16::new(cols).unwrap_or(NonZeroU16::MIN),
        config.gutter_top,
        config.gutter_bottom,
        config.gutter_left,
        config.gutter_right,
    );
    // The collapsed panel pushes (the compositor reserves a strip beside it)
    // and the expanded panel covers (it draws over the tiled windows while
    // holding that strip); the width alone decides, for the start layout and
    // every later apply alike, with no setting or runtime lever (issue #898,
    // change `pinned-expand-covers-collapsed-pushes`).
    match width {
        PinnedWidth::Collapsed => layout,
        PinnedWidth::Expanded => layout.covering(),
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
/// `/dev/null`: after the hand-over, native libraries (Wayland, xkbcommon,
/// fontconfig) writing to stderr must not draw over the panel. mbv reports fatal
/// post-hand-over errors via the log and a notification (design D5), so the
/// TUI does not need fd 2.
fn redirect_stderr() -> Result<(), PinStartError> {
    let file = crate::open_crash_log()
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
        // environment exists: the pinwin panel thread is not started yet, and
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
        // SAFETY: a failed start leaves no pinwin panel thread running
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
