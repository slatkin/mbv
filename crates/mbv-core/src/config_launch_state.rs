// Versioned TUI launch-state snapshot: the one coherent launch location a
// completed TUI session leaves for the next launch. Included into `config`'s
// module scope (see `config.rs`), so callers reach these items as
// `crate::config::…`.
//
// Delta spec: `openspec/changes/persist-tui-launch-state-on-exit/specs/
// tui-launch-state/spec.md`. The snapshot holds exactly the selected tab
// (`TabIdentity`), which Panel held focus (`LaunchPanelFocus`), and — for the
// selected tab only — the selected main Selector pill (`SelectorIdentity`)
// and selected library item (`LibraryItemIdentity`).
//
// The selector and item identities are tagged per destination (Home, Feeds,
// Emby, Audiobookshelf) so an identity from one destination cannot resolve
// in another. Payloads are stable Service content keys, never presentation
// indices or display strings: catalog ordering and labels can change between
// exit and the next launch. The narrow per-destination stable identities
// (closed pill-set enums, feed-group/show/book keys) are introduced
// alongside extraction in tasks 2.1/2.2, which extend these tagged enums.
//
// Never in this snapshot (per spec): state for unselected tabs, a selected
// Queue item, a nested Workspace selector, overlays/sidebars, search,
// multi-selection, scroll offsets, loading/error state, or paint geometry.

use std::path::Path;

/// Current on-disk format version. The loader accepts only this version and
/// returns `None` for anything else, so a newer file never mis-resolves into
/// stale identities — startup falls back to first-valid-choice rules.
pub const TUI_LAUNCH_STATE_VERSION: u32 = 1;

const fn default_tui_launch_state_version() -> u32 {
    TUI_LAUNCH_STATE_VERSION
}

/// One exit-time launch location: the selected tab, the focused Panel, and
/// the selected tab's pill/item identities. Written only at orderly TUI
/// exit; loaded once at startup into a pending restoration value.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TuiLaunchState {
    #[serde(default = "default_tui_launch_state_version")]
    pub version: u32,
    pub tab: TabIdentity,
    #[serde(default)]
    pub panel_focus: LaunchPanelFocus,
    #[serde(default)]
    pub selector: Option<SelectorIdentity>,
    #[serde(default)]
    pub item: Option<LibraryItemIdentity>,
}

/// Stable identity of a selectable Library tab: the fixed Home/Feeds tabs,
/// or a Service library keyed by Service kind plus library ID. A tab with
/// no main Selector pills records `selector: None`; the loader resolves a
/// missing tab to the first guaranteed tab in presentation order.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TabIdentity {
    Home,
    Feeds,
    ServiceLibrary {
        kind: ServiceKind,
        library_id: String,
    },
}

/// Which Panel held focus at exit. Queue focus restores focus only — Queue
/// selection always initializes from normal Queue rules, never from this
/// snapshot (which carries no Queue target by construction).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchPanelFocus {
    #[default]
    Library,
    Queue,
}

/// Selected main Selector pill, tagged per destination. Each payload is a
/// stable identity — never a row index. Home and Emby carry narrowed keys
/// (tasks 2.1): Home sections resolve to the persisted section source, Emby
/// letter pills to their closed bucket label and Emby group pills to the
/// group folder's content ID. Feeds and Audiobookshelf keep opaque keys
/// until task 2.2 narrows them the same way. Matching on the destination
/// variant is what makes cross-destination resolution impossible.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "destination", rename_all = "snake_case")]
pub enum SelectorIdentity {
    Home { key: HomeSelectorKey },
    Feeds { key: String },
    Emby { key: EmbySelectorKey },
    Audiobookshelf { key: String },
}

/// Stable Home section identity (task 2.1): Continue Watching is a fixed
/// scope, while each latest section resolves to its persisted source key
/// (`emby:<id>`, `abs:<id>`, `feeds`) — never a pill index or title.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HomeSelectorKey {
    Continue,
    Section(String),
}

/// Stable generic-Emby pill identity (task 2.1): letter pills are a closed
/// bucket set, so the bucket label is enum-like and stable; feed/home-video
/// group pills are dynamic, so they resolve to the group folder's Service
/// content ID — never a pill index or group display name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbySelectorKey {
    Letter(String),
    Group(String),
}

/// Selected library item, tagged per destination. Each payload is the
/// destination's stable content ID — never a row index or display string.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "destination", rename_all = "snake_case")]
pub enum LibraryItemIdentity {
    Home { id: String },
    Feeds { id: String },
    Emby { id: String },
    Audiobookshelf { id: String },
}

/// Domain error for launch-state writes. Reads are infallible by design
/// (missing/malformed → `None`, never a startup failure), so only the
/// writer surfaces errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TuiLaunchStateError {
    CreateDirectory(String),
    Serialize(String),
    Write(String),
    Replace(String),
}

impl std::fmt::Display for TuiLaunchStateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CreateDirectory(detail) => write!(formatter, "create directory: {detail}"),
            Self::Serialize(detail) => write!(formatter, "serialize launch state: {detail}"),
            Self::Write(detail) => write!(formatter, "write launch state: {detail}"),
            Self::Replace(detail) => write!(formatter, "replace launch state: {detail}"),
        }
    }
}

impl std::error::Error for TuiLaunchStateError {}

pub fn tui_launch_state_path() -> PathBuf {
    state_dir().join("tui_launch_state.json")
}

/// Serialize the complete snapshot, write a process-unique temporary file
/// in the state directory, and atomically rename it over the launch-state
/// file. The temporary name carries the pid plus a fresh uuid, so two
/// concurrently exiting Clients never share a temp path and cannot corrupt
/// each other's replace. A killed or crashed TUI leaves the previous
/// completed snapshot intact: only a finished rename is ever observed by
/// the next launch, whose snapshot is intentionally last-completed-exit
/// wins (no locking, merge, or Client identity).
pub fn save_tui_launch_state(state: &TuiLaunchState) -> Result<(), TuiLaunchStateError> {
    save_tui_launch_state_at(&tui_launch_state_path(), state)
}

fn save_tui_launch_state_at(
    path: &Path,
    state: &TuiLaunchState,
) -> Result<(), TuiLaunchStateError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| TuiLaunchStateError::CreateDirectory(format!("{}: {e}", dir.display())))?;
    }
    let json =
        serde_json::to_string(state).map_err(|e| TuiLaunchStateError::Serialize(e.to_string()))?;
    let tmp = tui_launch_state_tmp_path(path);
    std::fs::write(&tmp, &json)
        .map_err(|e| TuiLaunchStateError::Write(format!("{}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        TuiLaunchStateError::Replace(format!("{} to {}: {e}", tmp.display(), path.display()))
    })
}

/// Load the saved snapshot. Missing, unreadable, malformed, or
/// version-mismatched files all yield `None` — startup falls back to
/// first-valid-choice rules and is never prevented by launch-state data.
pub fn load_tui_launch_state() -> Option<TuiLaunchState> {
    load_tui_launch_state_at(&tui_launch_state_path())
}

fn load_tui_launch_state_at(path: &Path) -> Option<TuiLaunchState> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<TuiLaunchState>(&text) {
        Ok(state) if state.version == TUI_LAUNCH_STATE_VERSION => Some(state),
        Ok(state) => {
            log::warn!(
                target: "launch_state",
                "tui_launch_state.json version {} unsupported, launch state not restored",
                state.version
            );
            None
        }
        Err(e) => {
            log::warn!(
                target: "launch_state",
                "tui_launch_state.json failed to parse, launch state not restored: {e}"
            );
            None
        }
    }
}

/// Process-unique sibling path for the atomic replace: same directory (so
/// the rename stays atomic), qualified by pid and a fresh uuid so two
/// Clients writing concurrently never share a pathname.
fn tui_launch_state_tmp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "tui_launch_state.json".to_string());
    path.with_file_name(format!(
        "{file_name}.tmp-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4()
    ))
}
