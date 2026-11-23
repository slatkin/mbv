//! The `pinwin` panel identity and the panel's setting requests: the panel
//! identity claimed from the environment at start-up, the swap-token claim,
//! and the argv of every panel request a running Client sends.
//!
//! The panel is a process-wide singleton: `pinwin` allows one panel at a
//! time. Every panel request leaves this process as one `pinwin` child, and
//! the panel identity is claimed from the environment at start-up.

use std::sync::OnceLock;

use mbv_config::PanelConfig;
use mbv_ctrl::SWAP_TOKEN_ENV;
use mbv_desktop::launch::run_request;
use mbv_ui_model::settings::PinnedWidth;

/// The hidden launch flag that marks an mbv pinwin started inside a panel.
/// It is parsed but never shown in `--help`; a panel environment without it
/// leaves the launch unpinned.
pub const IN_PANEL_FLAG: &str = "--in-panel";

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
pub fn is_pinned() -> bool {
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
pub fn claim_panel_identity(in_panel: bool) {
    let panel = claim_env_once(PINWIN_PANEL_ENV);
    let socket = claim_env_once(PINWIN_SOCKET_ENV);
    let _ = PANEL_IDENTITY.set(PanelIdentity::new(panel, socket, in_panel));
}

/// The claimed panel identity, the empty identity when nothing was claimed.
fn identity() -> &'static PanelIdentity {
    PANEL_IDENTITY.get_or_init(PanelIdentity::default)
}

/// The claimed `PINWIN_SOCKET` of this launch: the daemon socket that serves
/// this panel, `None` outside a panel. Every panel request this process sends,
/// including the `--toggle` launch's, names that socket.
#[must_use]
pub fn claimed_socket() -> Option<&'static str> {
    identity().socket.as_deref()
}

/// The swap token this launch was started with, claimed once at startup.
/// `None` for every launch the user started.
static SWAP_TOKEN: OnceLock<Option<String>> = OnceLock::new();

/// Read `MBV_SWAP_TOKEN` and remove it from this process's environment.
/// Called once, before any worker or child process
/// spawns, so the token is neither read twice nor inherited by a spawned
/// daemon.
pub fn claim_swap_token() {
    let _ = SWAP_TOKEN.set(claim_env_once(SWAP_TOKEN_ENV));
}

/// The claimed swap token, `None` for every launch the user started.
#[must_use]
pub fn swap_token() -> Option<String> {
    SWAP_TOKEN.get().cloned().flatten()
}

pub use self::swap::claim_pin_swap_participant;
pub(in crate::app) use self::swap::pin_swap_hello;
mod swap;

/// The reserve mode a panel change asks for: the collapsed panel
/// pushes the tiled windows aside, the expanded one covers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelReserveMode {
    /// Reserve a strip beside the panel and move the tiled windows.
    Push,
    /// Draw over the tiled windows, holding the strip of the last push.
    Cover,
}

impl PanelReserveMode {
    /// The mode the `[panel] reserve` flag picks: `Push` while it is on,
    /// `Cover` while it is off.
    #[must_use]
    pub(in crate::app) fn for_flag(reserve: bool) -> Self {
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
pub(in crate::app) enum PanelGutter {
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
pub(in crate::app) enum PinnedSettingRequest {
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
    pub(in crate::app) fn edited_width(self) -> Option<PinnedWidth> {
        match self {
            Self::Width(width) => Some(width),
            Self::Side(_) | Self::Gutter(..) | Self::Reserve(_) => None,
        }
    }
}

/// A width in the pinwin size grammar: logical pixels.
#[must_use]
pub fn size_arg(width: u16) -> String {
    format!("{width}px")
}

/// The reserve mode the collapsed panel asks for: the `[panel] reserve`
/// flag's mode ([`PanelReserveMode::for_flag`]). The expanded panel's mode
/// is always `Cover`; no config key picks it.
#[must_use]
pub fn collapsed_reserve(config: &PanelConfig) -> PanelReserveMode {
    PanelReserveMode::for_flag(config.reserve)
}

/// The `--focus-ring` value of a `run`: `off`, or the configured
/// colour and width. Only a launch reads it; a running panel keeps its ring.
#[must_use]
pub fn focus_ring_arg(config: &PanelConfig) -> String {
    if config.accent {
        format!("{}:{}", config.accent_color, config.accent_width)
    } else {
        "off".to_string()
    }
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
pub(in crate::app) fn pinned_width_argv(
    panel: &str,
    config: &PanelConfig,
    width: PinnedWidth,
) -> Vec<String> {
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
pub(in crate::app) fn spawn_pinned_request(
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
pub(in crate::app) static PINNED_REQUEST_OVERRIDE: std::sync::Mutex<Option<Result<(), String>>> =
    std::sync::Mutex::new(None);

/// The requests the test seam delivered, in order.
#[cfg(test)]
pub(in crate::app) static PINNED_REQUESTS: std::sync::Mutex<Vec<PinnedSettingRequest>> =
    std::sync::Mutex::new(Vec::new());

/// Serializes tests that install [`PINNED_REQUEST_OVERRIDE`].
#[cfg(test)]
pub(in crate::app) static PINNED_REQUEST_TEST_LOCK: std::sync::Mutex<()> =
    std::sync::Mutex::new(());

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

#[cfg(test)]
mod tests;
