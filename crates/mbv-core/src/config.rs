use crate::api::MediaItem;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    pub server_url: String,
    pub username: String,
    pub password: String,
    pub api_key: String,
    pub hidden_libraries: Vec<String>,
    pub hidden_latest: Vec<String>,
    pub show_audio_window: bool,
    pub use_mpv_config: bool,
    pub audio_pipe_enabled: bool,
    pub audio_pipe_path: String,
    pub audio_pipe_samplerate: u32, // fixed output rate forced on the pipe (Hz); mpv resamples everything to this
    pub audio_pipe_bitdepth: u8,    // fixed PCM bit depth for the pipe (16|24|32)
    pub always_play_next: bool,
    pub consume_videos: bool,
    pub consume_audio: bool,
    pub always_skip_intro: bool,
    pub show_systray_icon: bool,
    pub no_scripts: bool,
    pub start_on_queue: bool,
    /// Stay-alive mode (issue #156): survive the controlling terminal
    /// closing while still playing, via an owned pty relay. Consulted only
    /// at launch (`-a`/`--alive` forces it for one launch regardless); a
    /// running bare session is never live-promoted. Default off.
    pub stay_alive: bool,
    /// On any quit with a dirty saved-playlist queue: `true` (default)
    /// silently pushes the edits to Emby; `false` silently discards them.
    /// `queue_state.json` local persistence is unconditional either way.
    pub save_playlist_on_quit: bool,
    pub autoload: bool,
    pub music_levels: Vec<String>,
    pub system_notifications: bool,
    pub save_playlist_on_consume: bool,
    pub save_playlist_on_consume_audio: bool,
    // [playback] — client-only subtitle/audio preferences (never pushed to Emby server)
    pub subtitle_mode: String, // "Default"|"Always"|"Smart"|"OnlyForced"|"None"|"HearingImpaired"; "" = inherit from Emby
    pub subtitle_lang: String, // full language name, e.g. "English"; "" = any
    pub audio_lang: String,    // full language name, e.g. "English"; "" = any
    pub my_languages: Vec<String>, // user's relevant languages; filters subtitle/audio lang cycling
    pub feed_view_libraries: Vec<String>, // libraries treated as feed view (unplayed, date-sorted)
    /// Library name (lowercased) -> resolved `tcp://host:port` daemon
    /// endpoint, from `[library_routes]` (#256, replacing #239's
    /// device-name values). Playback/enqueue resolved to one of these
    /// libraries connects straight to this stored endpoint -- no
    /// `/Sessions` lookup on the play/enqueue path at all. No device name
    /// is stored; a device's friendly name is used only transiently by
    /// the F2 "Library Routes" picker to let the user *pick* a device,
    /// then immediately resolved to an endpoint before being written here.
    /// A value that isn't a valid `tcp://` endpoint (including a stale
    /// pre-#256 device-name string) is malformed: logged and skipped by
    /// `resolve_library_route`, never routed. No `"*"` wildcard. Editable
    /// via the F2 Settings "Library routes" row, and hand-editable in
    /// `config.toml`.
    pub library_routes: std::collections::HashMap<String, String>,
    pub progress_interval_secs: u64, // how often to report playback progress to Emby (seconds)
    pub quit_timeout_secs: u64,      // how long quit waits for local player teardown (seconds)
    pub daemon_broadcast_ms: u64, // how often the daemon broadcasts status to connected TUIs (ms)
    pub daemon_client_endpoint: String, // [daemon.client] endpoint; empty = auto-detect local daemon
    pub daemon_server_tcp_listen: String, // [daemon.server] tcp_listen; empty = unix-only unless system instance default applies
    /// Reconnect at startup to whatever remote connection (a #223 library
    /// route, or a Sessions-panel direct-remote/attached session) was
    /// active when mbv last exited (issue #236 -- #222's original
    /// "auto-reconnect" intent, which #222's own lazy-connect-only design
    /// never actually implemented). Client-side setting: deliberately
    /// under `[general]`, not `[daemon.client]`/`[daemon.server]`, since
    /// this is a routing/reconnect *preference*, not daemon configuration.
    /// Default off; editable from config.toml or the F2 Settings panel.
    pub auto_reconnect: bool,
    /// Top-level view mode ("standard" | "power"), replacing the old
    /// `prefs.json["playlist_view"]` flag (issue #275). Read at startup and
    /// written whenever the mode changes via `App::set_view_mode`.
    pub view_mode: String,
}

pub const DEFAULT_SYSTEM_DAEMON_TCP_LISTEN: &str = "0.0.0.0:47788";

impl Default for Config {
    fn default() -> Self {
        Config {
            server_url: String::new(),
            username: String::new(),
            password: String::new(),
            api_key: String::new(),
            hidden_libraries: vec!["live tv".into()],
            hidden_latest: vec![],
            show_audio_window: false,
            use_mpv_config: false,
            audio_pipe_enabled: false,
            audio_pipe_path: "/tmp/mbv-pipe".to_string(),
            audio_pipe_samplerate: 192_000,
            audio_pipe_bitdepth: 32,
            always_play_next: false,
            consume_videos: false,
            consume_audio: false,
            always_skip_intro: false,
            show_systray_icon: true,
            no_scripts: false,
            start_on_queue: false,
            stay_alive: false,
            save_playlist_on_quit: true,
            autoload: false,
            music_levels: vec![],
            system_notifications: false,
            save_playlist_on_consume: false,
            save_playlist_on_consume_audio: false,
            subtitle_mode: String::new(),
            subtitle_lang: String::new(),
            audio_lang: String::new(),
            my_languages: vec![],
            feed_view_libraries: vec![],
            library_routes: std::collections::HashMap::new(),
            progress_interval_secs: 10,
            quit_timeout_secs: 5,
            daemon_broadcast_ms: 500,
            daemon_client_endpoint: String::new(),
            daemon_server_tcp_listen: String::new(),
            auto_reconnect: false,
            view_mode: "standard".to_string(),
        }
    }
}

impl Config {
    /// The mpv audio-pipe FIFO path to write to, or `None` when the feature
    /// is disabled. Centralizes the enabled/path pair so callers never need
    /// to re-derive this themselves.
    pub fn audio_pipe_target(&self) -> Option<String> {
        if self.audio_pipe_enabled {
            Some(self.audio_pipe_path.clone())
        } else {
            None
        }
    }
}

/// Resolves the configured endpoint for a library name (#256). Matches
/// case-insensitively (the query is lowercased before lookup; `routes`'
/// keys are already lowercased by `parse_config`). No wildcard fallback --
/// returns `None` if the library has no route, and the caller stays local.
///
/// Parses the stored string via `DaemonEndpoint::parse` and requires it to
/// be `Tcp(_)` -- library routing is a remote-only feature (#239 addendum:
/// "#222 and #223 are remote-connection features only"), so anything else
/// is malformed: a bare pre-#256 device-name string (which `parse` would
/// otherwise silently accept as a bogus `Unix(PathBuf)` socket path), a
/// `unix://` value, or a bare `local`/empty value are all logged and
/// skipped rather than routed. This is a pure, synchronous, no-network
/// lookup -- the entire point of #256 is that route resolution on the
/// play/enqueue path never touches `/Sessions` again.
pub fn resolve_library_route(
    routes: &std::collections::HashMap<String, String>,
    library_name: &str,
) -> Option<crate::remote_player::DaemonEndpoint> {
    let raw = routes.get(&library_name.to_lowercase())?;
    match crate::remote_player::DaemonEndpoint::parse(raw) {
        Ok(endpoint @ crate::remote_player::DaemonEndpoint::Tcp(_)) => Some(endpoint),
        Ok(other) => {
            log::warn!(
                target: "library_route",
                "library_routes entry {raw:?} parsed as {other:?}, but library routing is tcp://-only; skipping"
            );
            None
        }
        Err(e) => {
            log::warn!(
                target: "library_route",
                "library_routes entry {raw:?} is not a valid tcp:// endpoint: {e}; skipping"
            );
            None
        }
    }
}

pub fn is_system_instance() -> bool {
    env::var("MBV_SYSTEM").ok().as_deref() == Some("1")
}

pub fn default_daemon_server_tcp_listen() -> String {
    if is_system_instance() {
        DEFAULT_SYSTEM_DAEMON_TCP_LISTEN.to_string()
    } else {
        String::new()
    }
}

fn config_dir() -> PathBuf {
    // See `TEST_CONFIG_DIR_OVERRIDE` / `TestStateDirGuard`: without this,
    // `config_dir()` (and therefore `config_path()`/`save_config_settings`)
    // had no test isolation at all -- unlike `state_dir()`, *every* call in
    // a test build hit the real `$XDG_CONFIG_HOME`/`~/.config/mbv` on disk.
    // Any App-level test that reached a synchronous config-saving path
    // (e.g. `set_view_mode`, `cycle_subtitle_mode`, closing a multiselect
    // settings popup) would silently clobber the developer's real
    // config.toml. `TestStateDirGuard` already attaches to every `App`
    // built in test mode, so piggybacking the override there closes this
    // for the whole suite at once.
    #[cfg(any(test, feature = "test-support"))]
    if let Some(dir) = TEST_CONFIG_DIR_OVERRIDE.with(|c| c.borrow().clone()) {
        return dir;
    }
    if is_system_instance() {
        return PathBuf::from("/etc/mbv");
    }
    let base = env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".config")
        });
    base.join("mbv")
}

pub fn cache_dir() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/cache/mbv");
    }
    let base = env::var("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".cache")
        });
    base.join("mbv")
}

// Test-only escape hatch: `state_dir()` (and therefore `queue_state_path()`,
// `save_queue_state`/`load_queue_state`/`clear_queue_state`) is used not just
// by tests that are explicitly *about* path resolution, but incidentally by
// any test that drives consume-mode/queue logic through `App` methods like
// `save_queue_state()` -- e.g. tests that fire `PlayerEvent::Stopped` and
// assert on in-memory queue state have no reason to care where the file
// lands, so historically nobody bothered to isolate them. But
// `XDG_STATE_HOME`/`MBV_SYSTEM` are process-global env vars: an unguarded
// test's call to `state_dir()` observes whatever value another, *properly
// locked* test happens to have set at that exact moment (env vars have no
// per-thread scoping), so it can transiently read -- and write into --
// a locked test's private tempdir mid-race, corrupting it. A thread-local
// override sidesteps the whole problem for these incidental callers: it's
// only visible on the thread that set it, so two tests running on different
// threads can never observe (or clobber) each other's override, no lock
// required. See `TestStateDirGuard` and issue #106.
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_STATE_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

// `config_dir()` (config.toml, via `save_config_settings`) gets the exact
// same treatment as `state_dir()` above, for the exact same reason: some
// App-level settings toggles (`set_view_mode`, `cycle_subtitle_mode`,
// closing a multiselect settings popup) write to disk synchronously rather
// than through the debounced `settings_save_at` path, so any unguarded test
// that reaches one of them writes straight into the developer's real
// config.toml. `TestStateDirGuard` sets this override alongside its own so
// every test that already uses it (including, automatically, every `App`
// built in a test binary via `_test_state_dir_guard`) gets both for free.
#[cfg(any(test, feature = "test-support"))]
thread_local! {
    static TEST_CONFIG_DIR_OVERRIDE: std::cell::RefCell<Option<PathBuf>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
static TEST_DEFAULT_STATE_DIR: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();

#[cfg(any(test, feature = "test-support"))]
pub struct TestStateDirGuard;

#[cfg(any(test, feature = "test-support"))]
impl TestStateDirGuard {
    /// Points `state_dir()` at a fresh, unique tempdir for the lifetime of
    /// this guard, visible only on the calling thread. Use this in any test
    /// that drives `App` logic which might incidentally call
    /// `save_queue_state`/`restore_queue_state` (e.g. via consume-mode event
    /// handling) but isn't itself testing path resolution -- so it never
    /// touches a real on-disk path or races a sibling test.
    pub fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        Self::new_at(dir)
    }

    /// Points `state_dir()` (and `config_dir()`) at `dir` for the lifetime
    /// of this guard. Both point at the same directory -- their file names
    /// never collide (`config.toml` vs. `prefs.json`/`token.json`/
    /// `queue_state.json`) -- so one guard, one tempdir, one cleanup.
    pub fn new_at(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        TEST_STATE_DIR_OVERRIDE.with(|c| *c.borrow_mut() = Some(dir.clone()));
        TEST_CONFIG_DIR_OVERRIDE.with(|c| *c.borrow_mut() = Some(dir));
        TestStateDirGuard
    }

    /// Installs a fresh override only when this thread does not already have
    /// one. This lets broad app-test fixtures isolate incidental queue-state
    /// writes without shadowing tests that explicitly seeded queue state first.
    pub fn new_if_unset() -> Option<Self> {
        if TEST_STATE_DIR_OVERRIDE.with(|c| c.borrow().is_some()) {
            None
        } else {
            Some(Self::new())
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Default for TestStateDirGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(test, feature = "test-support"))]
impl Drop for TestStateDirGuard {
    fn drop(&mut self) {
        // Both overrides point at the same physical directory (see
        // `new_at`) and are always set/cleared together, so only one
        // `take()` needs to delete the directory -- but both thread-locals
        // must be cleared regardless, or the config override would keep
        // pointing at a directory this guard is about to delete.
        let dir = TEST_STATE_DIR_OVERRIDE.with(|c| c.borrow_mut().take());
        TEST_CONFIG_DIR_OVERRIDE.with(|c| c.borrow_mut().take());
        if let Some(dir) = dir {
            let _ = std::fs::remove_dir_all(&dir);
        }
    }
}

fn state_dir() -> PathBuf {
    #[cfg(any(test, feature = "test-support"))]
    if let Some(dir) = TEST_STATE_DIR_OVERRIDE.with(|c| c.borrow().clone()) {
        return dir;
    }
    #[cfg(test)]
    {
        if env::var_os("XDG_STATE_HOME").is_none() && env::var_os("MBV_SYSTEM").is_none() {
            return TEST_DEFAULT_STATE_DIR
                .get_or_init(|| {
                    std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()))
                })
                .clone();
        }
    }
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_STATE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("state")
        });
    base.join("mbv")
}

pub fn data_dir_system_or_local() -> PathBuf {
    if is_system_instance() {
        return PathBuf::from("/var/lib/mbv");
    }
    let base = env::var("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = env::var("HOME").unwrap_or_else(|_| "/root".to_string());
            PathBuf::from(home).join(".local").join("share")
        });
    base.join("mbv")
}

fn queue_state_path() -> PathBuf {
    state_dir().join("queue_state.json")
}

fn library_position_state_path() -> PathBuf {
    state_dir().join("library_position_state.json")
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum QueueSource {
    Playlist {
        id: Option<String>,
        name: String,
    },
    Album,
    Series,
    Shuffle,
    Remote,
    Collection {
        collection_type: String,
    },
    #[default]
    Unknown,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct QueueState {
    #[serde(default)]
    pub source: QueueSource,
    // Full items, not just IDs: restoring the queue must be instant and local
    // (no network round-trip), so everything needed to display and play it
    // has to already be on disk. A separate best-effort background fetch
    // refreshes played/position state from the server afterward.
    #[serde(default)]
    pub items: Vec<MediaItem>,
    #[serde(default)]
    pub cursor: usize,
    pub last_played_item_id: Option<String>,
    pub last_played_completed: bool,
    // Per-item resume positions saved at quit time. Used on restore to override stale Emby
    // UserData when a fresh launch races with Emby's async position write.
    #[serde(default)]
    pub positions: std::collections::HashMap<String, i64>,
}

pub fn save_queue_state(state: &QueueState) {
    let path = queue_state_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(state) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_queue_state() -> Option<QueueState> {
    let text = std::fs::read_to_string(queue_state_path()).ok()?;
    match serde_json::from_str(&text) {
        Ok(state) => Some(state),
        Err(e) => {
            log::warn!(target: "queue", "queue_state.json failed to parse, queue not restored: {e}");
            None
        }
    }
}

pub fn clear_queue_state() {
    let _ = std::fs::remove_file(queue_state_path());
}

/// Which remote connection (if any) was active when mbv last exited
/// (issue #236). `App::teardown` writes this; `App::new` reads it back at
/// the next launch when `Config.auto_reconnect` is true. The two
/// variants mirror `App`'s own separate `active_route` (#223 library
/// routing) and `connected_session_id`/`connected_session_state`
/// (Sessions-panel direct-remote/attached) fields -- #222 and #223 were
/// distinct features and stay distinct here, even though both are
/// restored under the same on/off switch.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind")]
pub enum LastRemoteConnection {
    /// A #223 library route, keyed by the library name that was resolved
    /// active (`App.active_route`). Re-resolved fresh against current
    /// `library_routes` at startup, not replayed verbatim -- if the config
    /// changed since the last exit, the new config wins.
    LibraryRoute { library: String },
    /// A Sessions-panel direct-remote or attached session, keyed by the
    /// other device's name (`SessionInfo.device_name`), not its session id
    /// -- Emby session ids are ephemeral per-connection and would not
    /// still identify the same device at the next launch.
    DirectSession { device_name: String },
}

fn last_remote_connection_path() -> PathBuf {
    state_dir().join("last_remote_connection.json")
}

/// Persists (or, given `None`, clears) the connection active at exit.
/// Called from `App::teardown` only when `auto_reconnect` is
/// enabled -- when the feature is off, this file is never written or
/// read, by design (Task 1's `Global Constraints`).
fn save_last_remote_connection_at(
    path: &std::path::Path,
    conn: Option<&LastRemoteConnection>,
) -> Result<(), String> {
    let Some(conn) = conn else {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("remove {}: {e}", path.display())),
        };
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create directory {}: {e}", dir.display()))?;
    }
    let json =
        serde_json::to_string(conn).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub fn save_last_remote_connection(conn: Option<&LastRemoteConnection>) -> Result<(), String> {
    save_last_remote_connection_at(&last_remote_connection_path(), conn)
}

fn load_last_remote_connection_at(
    path: &std::path::Path,
) -> Result<Option<LastRemoteConnection>, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    match serde_json::from_str(&text) {
        Ok(conn) => Ok(Some(conn)),
        Err(e) => {
            std::fs::remove_file(path).map_err(|remove_error| {
                format!(
                    "parse {}: {e}; remove corrupt {}: {remove_error}",
                    path.display(),
                    path.display()
                )
            })?;
            Err(format!(
                "parse {}: {e}; corrupt file removed",
                path.display()
            ))
        }
    }
}

pub fn load_last_remote_connection() -> Result<Option<LastRemoteConnection>, String> {
    load_last_remote_connection_at(&last_remote_connection_path())
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPositionState {
    #[serde(default)]
    pub libraries: std::collections::HashMap<String, LibraryViewPositions>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryViewPositions {
    #[serde(default)]
    pub default: Option<LibraryPosition>,
    #[serde(default)]
    pub power: Option<LibraryPosition>,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPosition {
    #[serde(default)]
    pub levels: Vec<LibraryPositionLevel>,
    #[serde(default)]
    pub feed_selected_group: usize,
    #[serde(default)]
    pub feed_video_cursor: usize,
    #[serde(default)]
    pub feed_video_scroll: usize,
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPositionLevel {
    pub parent_id: String,
    pub title: String,
    #[serde(default)]
    pub focused_item_id: Option<String>,
    #[serde(default)]
    pub cursor_index: usize,
    #[serde(default)]
    pub item_types: Option<String>,
    #[serde(default)]
    pub unplayed_only: bool,
    #[serde(default)]
    pub sort_by: String,
    #[serde(default)]
    pub sort_order: String,
    /// Index of the active letter-range pill (see `app::render::power::LetterFilter`),
    /// for the top level of a large library. `None` = unfiltered / not applicable.
    #[serde(default)]
    pub letter_filter_index: Option<usize>,
    /// The library's TRUE unfiltered item count, captured for the top level
    /// of a library so a restored session doesn't need an extra unfiltered
    /// fetch just to re-derive whether the letter pill row applies.
    /// `None` for non-root levels (or when never captured).
    #[serde(default)]
    pub library_total: Option<usize>,
}

pub fn save_library_position_state(state: &LibraryPositionState) {
    let path = library_position_state_path();
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(json) = serde_json::to_string(state) {
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, &json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

pub fn load_library_position_state() -> LibraryPositionState {
    let text = match std::fs::read_to_string(library_position_state_path()) {
        Ok(text) => text,
        Err(_) => return LibraryPositionState::default(),
    };
    match serde_json::from_str(&text) {
        Ok(state) => state,
        Err(e) => {
            log::warn!(target: "library_position", "library_position_state.json failed to parse: {e}");
            LibraryPositionState::default()
        }
    }
}

/// Visibility/size of the now-playing panel, cycled with `h` and remembered across restarts.
fn migrate_to_state(filename: &str) -> PathBuf {
    let dest = state_dir().join(filename);
    if dest.exists() {
        return dest;
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let cache = cache_dir().join(filename);
    if cache.exists() {
        let _ = std::fs::rename(&cache, &dest);
        return dest;
    }
    let old = config_dir().join(filename);
    if old.exists() {
        let _ = std::fs::rename(&old, &dest);
    }
    dest
}

pub fn osc_script_path() -> PathBuf {
    let user = data_dir_system_or_local().join("scripts").join("mbv.lua");
    if user.exists() {
        return user;
    }
    let dev = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/scripts/mbv.lua"));
    if dev.exists() {
        return dev;
    }
    PathBuf::from("/usr/share/mbv/scripts/mbv.lua")
}

pub fn prefs_path() -> PathBuf {
    migrate_to_state("prefs.json")
}

pub fn osc_fonts_dir() -> PathBuf {
    let user = data_dir_system_or_local().join("fonts");
    if user.exists() {
        return user;
    }
    PathBuf::from("/usr/share/mbv/fonts")
}

fn runtime_dir() -> String {
    if is_system_instance() {
        return "/run/mbv".to_string();
    }
    env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_string())
}

pub fn mpv_ipc_path() -> String {
    format!("{}/mbv-mpv.sock", runtime_dir())
}

pub fn mpv_config_dir() -> PathBuf {
    PathBuf::from(runtime_dir()).join("mpv-config")
}

pub fn control_socket_path() -> String {
    format!("{}/mbv-ctrl.sock", runtime_dir())
}

pub fn token_cache_path() -> PathBuf {
    migrate_to_state("token.json")
}

pub fn config_path() -> PathBuf {
    config_dir().join("config.toml")
}

pub fn load_config() -> Result<Config, String> {
    let path = config_path();
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(_) => return Ok(Config::default()),
    };
    parse_config(&text).map_err(|e| format!("Config parse error in {:?}: {e}", path))
}

pub fn parse_config(text: &str) -> Result<Config, String> {
    let doc: toml::Value = toml::from_str(text).map_err(|e| e.to_string())?;

    let server = match doc.get("server") {
        Some(s) => s,
        None => return Ok(Config::default()),
    };

    let get_str = |section: &toml::Value, key: &str| -> String {
        section
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };

    let misc = doc.get("mpv");
    let mbvd = doc.get("mbvd");
    let session = doc.get("session");
    let library = doc.get("library");
    let display = doc.get("display");
    let playback = doc.get("playback");
    let queue = doc.get("queue");
    let music = doc.get("library").and_then(|l| l.get("music"));

    let hidden_libraries: Vec<String> = library
        .and_then(|m| m.get("hidden_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_else(|| vec!["live tv".into()]);

    let hidden_latest: Vec<String> = library
        .and_then(|m| m.get("hidden_latest"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let show_audio_window = misc
        .and_then(|m| m.get("show_audio_window"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let use_mpv_config = misc
        .and_then(|m| m.get("use_mpv_config"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let audio_pipe_enabled = misc
        .and_then(|m| m.get("audio_pipe_enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let audio_pipe_path = misc
        .and_then(|m| m.get("audio_pipe_path"))
        .and_then(|v| v.as_str())
        .unwrap_or("/tmp/mbv-pipe")
        .to_string();

    let audio_pipe_samplerate = misc
        .and_then(|m| m.get("audio_pipe_samplerate"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u32)
        .unwrap_or(192_000);
    let audio_pipe_bitdepth = misc
        .and_then(|m| m.get("audio_pipe_bitdepth"))
        .and_then(|v| v.as_integer())
        .map(|v| match v {
            16 | 24 | 32 => v as u8,
            _ => 32,
        })
        .unwrap_or(32);

    let always_play_next = queue
        .and_then(|q| q.get("always_play_next"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let consume_videos = queue
        .and_then(|q| q.get("consume_videos"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let consume_audio = queue
        .and_then(|q| q.get("consume_audio"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let start_on_queue = queue
        .and_then(|q| q.get("start_on_queue"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let always_skip_intro = session
        .and_then(|m| m.get("always_skip_intro"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let show_systray_icon = playback
        .and_then(|d| d.get("show_systray_icon"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let stay_alive = session
        .and_then(|m| m.get("stay_alive"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let auto_reconnect = session
        .and_then(|m| m.get("auto_reconnect"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let view_mode = display
        .and_then(|d| d.get("view_mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("standard")
        .to_string();

    let save_playlist_on_quit = session
        .and_then(|m| m.get("save_playlist_on_quit"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);

    let no_scripts = misc
        .and_then(|m| m.get("no_scripts"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let autoload = misc
        .and_then(|m| m.get("autoload"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let music_levels: Vec<String> = music
        .and_then(|m| m.get("levels"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    let system_notifications = display
        .and_then(|m| m.get("system_notifications"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let save_playlist_on_consume = queue
        .and_then(|q| q.get("save_playlist_on_consume"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let save_playlist_on_consume_audio = queue
        .and_then(|q| q.get("save_playlist_on_consume_audio"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let subtitle_mode = playback
        .and_then(|p| p.get("subtitle_mode"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let subtitle_lang = playback
        .and_then(|p| p.get("subtitle_lang"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let audio_lang = playback
        .and_then(|p| p.get("audio_lang"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let my_languages: Vec<String> = playback
        .and_then(|p| p.get("my_languages"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let progress_interval_secs = session
        .and_then(|m| m.get("progress_interval_secs"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u64)
        .unwrap_or(10);

    let quit_timeout_secs = session
        .and_then(|m| m.get("quit_timeout_secs"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(1) as u64)
        .unwrap_or(5);

    let daemon_broadcast_ms = mbvd
        .and_then(|d| d.get("broadcast_ms"))
        .and_then(|v| v.as_integer())
        .map(|v| v.max(100) as u64)
        .unwrap_or(500);

    let daemon_client_endpoint = mbvd
        .and_then(|d| d.get("client"))
        .and_then(|c| c.get("endpoint"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let daemon_server_tcp_listen = mbvd
        .and_then(|d| d.get("server"))
        .and_then(|s| s.get("tcp_listen"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .unwrap_or_else(default_daemon_server_tcp_listen);

    let feed_view_libraries: Vec<String> = library
        .and_then(|m| m.get("feed_view_libraries"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_lowercase())
                .collect()
        })
        .unwrap_or_default();

    let library_routes: std::collections::HashMap<String, String> = doc
        .get("library_routes")
        .and_then(|v| v.as_table())
        .map(|table| {
            table
                .iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.to_lowercase(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    Ok(Config {
        server_url: get_str(server, "url").trim_end_matches('/').to_string(),
        username: String::new(),
        password: String::new(),
        api_key: String::new(),
        hidden_libraries,
        hidden_latest,
        show_audio_window,
        use_mpv_config,
        audio_pipe_enabled,
        audio_pipe_path,
        audio_pipe_samplerate,
        audio_pipe_bitdepth,
        always_play_next,
        consume_videos,
        consume_audio,
        always_skip_intro,
        show_systray_icon,
        no_scripts,
        start_on_queue,
        stay_alive,
        save_playlist_on_quit,
        autoload,
        music_levels,
        system_notifications,
        save_playlist_on_consume,
        save_playlist_on_consume_audio,
        subtitle_mode,
        subtitle_lang,
        audio_lang,
        my_languages,
        feed_view_libraries,
        library_routes,
        progress_interval_secs,
        quit_timeout_secs,
        daemon_broadcast_ms,
        daemon_client_endpoint,
        daemon_server_tcp_listen,
        auto_reconnect,
        view_mode,
    })
}

fn save_config_settings_at(cfg: &Config, path: &std::path::Path) -> Result<(), String> {
    let mut doc: toml::Value = match std::fs::read_to_string(path) {
        Ok(text) => toml::from_str(&text).map_err(|e| format!("parse {}: {e}", path.display()))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            toml::Value::Table(toml::map::Map::new())
        }
        Err(e) => return Err(format!("read {}: {e}", path.display())),
    };
    let table = match doc.as_table_mut() {
        Some(t) => t,
        None => return Err(format!("update {}: root is not a table", path.display())),
    };

    macro_rules! section {
        ($name:literal) => {
            table
                .entry($name.to_string())
                .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
                .as_table_mut()
                .unwrap()
        };
    }

    if !cfg.server_url.is_empty() {
        let server = section!("server");
        server.insert(
            "url".to_string(),
            toml::Value::String(cfg.server_url.clone()),
        );
    }

    let session = section!("session");
    session.insert(
        "stay_alive".to_string(),
        toml::Value::Boolean(cfg.stay_alive),
    );
    session.insert(
        "auto_reconnect".to_string(),
        toml::Value::Boolean(cfg.auto_reconnect),
    );
    session.insert(
        "save_playlist_on_quit".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_quit),
    );
    session.insert(
        "always_skip_intro".to_string(),
        toml::Value::Boolean(cfg.always_skip_intro),
    );
    session.insert(
        "quit_timeout_secs".to_string(),
        toml::Value::Integer(cfg.quit_timeout_secs as i64),
    );
    session.insert(
        "progress_interval_secs".to_string(),
        toml::Value::Integer(cfg.progress_interval_secs as i64),
    );

    let library = section!("library");
    library.insert(
        "hidden_libraries".to_string(),
        toml::Value::Array(
            cfg.hidden_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    library.insert(
        "hidden_latest".to_string(),
        toml::Value::Array(
            cfg.hidden_latest
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );
    library.insert(
        "feed_view_libraries".to_string(),
        toml::Value::Array(
            cfg.feed_view_libraries
                .iter()
                .map(|s| toml::Value::String(s.clone()))
                .collect(),
        ),
    );

    let display = section!("display");
    display.insert(
        "system_notifications".to_string(),
        toml::Value::Boolean(cfg.system_notifications),
    );
    display.insert(
        "view_mode".to_string(),
        toml::Value::String(cfg.view_mode.clone()),
    );

    if !cfg.music_levels.is_empty() {
        let library = section!("library");
        let music = library
            .entry("music".to_string())
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
            .as_table_mut()
            .unwrap();
        music.insert(
            "levels".to_string(),
            toml::Value::Array(
                cfg.music_levels
                    .iter()
                    .map(|s| toml::Value::String(s.clone()))
                    .collect(),
            ),
        );
    }

    if cfg.library_routes.is_empty() {
        table.remove("library_routes");
    } else {
        let mut routes_table = toml::map::Map::new();
        for (library, device) in &cfg.library_routes {
            routes_table.insert(library.clone(), toml::Value::String(device.clone()));
        }
        table.insert(
            "library_routes".to_string(),
            toml::Value::Table(routes_table),
        );
    }

    let queue = section!("queue");
    queue.insert(
        "always_play_next".to_string(),
        toml::Value::Boolean(cfg.always_play_next),
    );
    queue.insert(
        "consume_videos".to_string(),
        toml::Value::Boolean(cfg.consume_videos),
    );
    queue.insert(
        "consume_audio".to_string(),
        toml::Value::Boolean(cfg.consume_audio),
    );
    queue.insert(
        "start_on_queue".to_string(),
        toml::Value::Boolean(cfg.start_on_queue),
    );
    queue.insert(
        "save_playlist_on_consume".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_consume),
    );
    queue.insert(
        "save_playlist_on_consume_audio".to_string(),
        toml::Value::Boolean(cfg.save_playlist_on_consume_audio),
    );

    let mpv = section!("mpv");
    mpv.insert(
        "show_audio_window".to_string(),
        toml::Value::Boolean(cfg.show_audio_window),
    );
    mpv.insert(
        "use_mpv_config".to_string(),
        toml::Value::Boolean(cfg.use_mpv_config),
    );
    mpv.insert(
        "no_scripts".to_string(),
        toml::Value::Boolean(cfg.no_scripts),
    );
    mpv.insert("autoload".to_string(), toml::Value::Boolean(cfg.autoload));

    let mbvd = section!("mbvd");
    mbvd.insert(
        "broadcast_ms".to_string(),
        toml::Value::Integer(cfg.daemon_broadcast_ms as i64),
    );
    mbvd.insert(
        "audio_pipe_enabled".to_string(),
        toml::Value::Boolean(cfg.audio_pipe_enabled),
    );
    mbvd.insert(
        "audio_pipe_path".to_string(),
        toml::Value::String(cfg.audio_pipe_path.clone()),
    );
    mbvd.insert(
        "audio_pipe_samplerate".to_string(),
        toml::Value::Integer(cfg.audio_pipe_samplerate as i64),
    );
    mbvd.insert(
        "audio_pipe_bitdepth".to_string(),
        toml::Value::Integer(cfg.audio_pipe_bitdepth as i64),
    );
    let mbvd_client = mbvd
        .entry("client".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if cfg.daemon_client_endpoint.trim().is_empty() {
        mbvd_client.remove("endpoint");
    } else {
        mbvd_client.insert(
            "endpoint".to_string(),
            toml::Value::String(cfg.daemon_client_endpoint.clone()),
        );
    }
    let mbvd_server = mbvd
        .entry("server".to_string())
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()))
        .as_table_mut()
        .unwrap();
    if cfg.daemon_server_tcp_listen.trim().is_empty() {
        mbvd_server.remove("tcp_listen");
    } else {
        mbvd_server.insert(
            "tcp_listen".to_string(),
            toml::Value::String(cfg.daemon_server_tcp_listen.clone()),
        );
    }

    let playback = section!("playback");
    playback.insert(
        "show_systray_icon".to_string(),
        toml::Value::Boolean(cfg.show_systray_icon),
    );
    if cfg.subtitle_mode.is_empty() {
        playback.remove("subtitle_mode");
    } else {
        playback.insert(
            "subtitle_mode".to_string(),
            toml::Value::String(cfg.subtitle_mode.clone()),
        );
    }
    if cfg.subtitle_lang.is_empty() {
        playback.remove("subtitle_lang");
    } else {
        playback.insert(
            "subtitle_lang".to_string(),
            toml::Value::String(cfg.subtitle_lang.clone()),
        );
    }
    if cfg.audio_lang.is_empty() {
        playback.remove("audio_lang");
    } else {
        playback.insert(
            "audio_lang".to_string(),
            toml::Value::String(cfg.audio_lang.clone()),
        );
    }
    if cfg.my_languages.is_empty() {
        playback.remove("my_languages");
    } else {
        playback.insert(
            "my_languages".to_string(),
            toml::Value::Array(
                cfg.my_languages
                    .iter()
                    .map(|s| toml::Value::String(s.clone()))
                    .collect(),
            ),
        );
    }
    let s = toml::to_string(&doc).map_err(|e| format!("serialize {}: {e}", path.display()))?;
    write_config_text_at(path, &s)
}

fn write_config_text_at(path: &std::path::Path, text: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create directory {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub fn save_config_settings(cfg: &Config) -> Result<(), String> {
    save_config_settings_at(cfg, &config_path())
}

#[cfg(any(test, feature = "test-support"))]
pub mod tests {
    #[cfg(test)]
    use super::*;
    #[cfg(test)]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[cfg(test)]
    #[test]
    fn parse_full_config() {
        let toml = r#"
[server]
url = "http://localhost:8096/"
[library]
hidden_libraries = ["Live TV", "Podcasts", "Music"]
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.server_url, "http://localhost:8096"); // trailing slash stripped
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "podcasts", "music"]);
    }

    #[cfg(test)]
    #[test]
    fn parse_missing_server_section_returns_default() {
        let cfg = parse_config("[mpv]\nshow_audio_window = false").unwrap();
        assert_eq!(cfg.server_url, "");
        assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
    }

    #[cfg(test)]
    #[test]
    fn parse_empty_string_returns_default() {
        let cfg = parse_config("").unwrap();
        assert_eq!(cfg.server_url, "");
    }

    #[cfg(test)]
    #[test]
    fn parse_audio_pipe_settings() {
        let toml = r#"
[server]
url = "http://localhost:8096"
[mpv]
audio_pipe_enabled = true
        audio_pipe_path = "/tmp/custom-pipe"
audio_pipe_samplerate = 96000
audio_pipe_bitdepth = 16
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.audio_pipe_enabled);
        assert_eq!(cfg.audio_pipe_path, "/tmp/custom-pipe");
        assert_eq!(cfg.audio_pipe_samplerate, 96000);
        assert_eq!(cfg.audio_pipe_bitdepth, 16);
    }

    #[cfg(test)]
    #[test]
    fn parse_audio_pipe_defaults() {
        let cfg = parse_config("").unwrap();
        assert!(!cfg.audio_pipe_enabled);
        assert_eq!(cfg.audio_pipe_path, "/tmp/mbv-pipe");
        assert_eq!(cfg.audio_pipe_samplerate, 192_000);
        assert_eq!(cfg.audio_pipe_bitdepth, 32);
    }

    #[cfg(test)]
    #[test]
    fn parse_daemon_client_endpoint() {
        let toml = r#"
[server]
url = "http://localhost:8096"
[mbvd.client]
endpoint = "unix:///tmp/mbv.sock"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.daemon_client_endpoint, "unix:///tmp/mbv.sock");
    }

    #[cfg(test)]
    #[test]
    fn parse_daemon_server_tcp_listen() {
        let toml = r#"
[server]
url = "http://localhost:8096"
[mbvd.server]
tcp_listen = "0.0.0.0:8890"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.daemon_server_tcp_listen, "0.0.0.0:8890");
    }

    #[cfg(test)]
    #[test]
    fn parse_quit_timeout_defaults_and_clamps() {
        let cfg = parse_config("[server]\nurl = \"http://localhost:8096\"").unwrap();
        assert_eq!(cfg.quit_timeout_secs, 5);

        let cfg = parse_config(
            r#"
[server]
url = "http://localhost:8096"
[session]
quit_timeout_secs = 0
"#,
        )
        .unwrap();
        assert_eq!(cfg.quit_timeout_secs, 1);

        let cfg = parse_config(
            r#"
[server]
url = "http://localhost:8096"
[session]
quit_timeout_secs = -10
"#,
        )
        .unwrap();
        assert_eq!(cfg.quit_timeout_secs, 1);
    }

    #[cfg(test)]
    #[test]
    fn parse_consume_audio_and_autosave_default_to_false() {
        let cfg = parse_config("").unwrap();
        assert!(!cfg.consume_audio);
        assert!(!cfg.save_playlist_on_consume_audio);
    }

    #[cfg(test)]
    #[test]
    fn parse_consume_audio_and_autosave_flags() {
        let toml = r#"
[server]
url = "http://localhost:8096"
[queue]
consume_audio = true
save_playlist_on_consume_audio = true
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.consume_audio);
        assert!(cfg.save_playlist_on_consume_audio);
    }

    #[cfg(test)]
    #[test]
    fn save_config_settings_round_trips_consume_audio_flags() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "mbv-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("mbv")).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("MBV_SYSTEM");

        let mut cfg = Config {
            server_url: "http://localhost:8096".into(),
            ..Default::default()
        };
        cfg.consume_audio = true;
        cfg.save_playlist_on_consume_audio = true;
        cfg.quit_timeout_secs = 7;
        save_config_settings(&cfg).unwrap();

        let saved = std::fs::read_to_string(config_path()).unwrap();
        let reparsed = parse_config(&saved).unwrap();
        assert!(reparsed.consume_audio);
        assert!(reparsed.save_playlist_on_consume_audio);
        assert_eq!(reparsed.quit_timeout_secs, 7);

        std::env::remove_var("XDG_CONFIG_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_hidden_libraries_lowercased() {
        let toml = r#"
[server]
url = "http://host"
[library]
hidden_libraries = ["Live TV", "MOVIES"]
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_libraries, vec!["live tv", "movies"]);
    }

    #[test]
    fn parse_default_hidden_libraries_when_absent() {
        let toml = "[server]\nurl = \"http://host\"";
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_libraries, vec!["live tv"]);
    }

    #[test]
    fn parse_hidden_latest_lowercased() {
        let toml = r#"
[server]
url = "http://host"
[library]
hidden_latest = ["Movies", "TV SHOWS"]
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.hidden_latest, vec!["movies", "tv shows"]);
    }

    #[test]
    fn parse_default_hidden_latest_when_absent() {
        let toml = "[server]\nurl = \"http://host\"";
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.hidden_latest.is_empty());
    }

    #[test]
    fn parse_library_routes_lowercased_keys() {
        let toml = r#"
[server]
url = "http://host"
[library_routes]
Music = "tcp://192.168.0.104:47788"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.library_routes.get("music").map(String::as_str),
            Some("tcp://192.168.0.104:47788")
        );
    }

    #[test]
    fn parse_library_routes_ignores_legacy_wildcard_key() {
        // "*" is no longer a wildcard -- it's just an (unusable) library
        // name like any other, since #239 dropped the catch-all.
        let toml = r#"
[server]
url = "http://host"
[library_routes]
"*" = "tcp://192.168.0.104:47788"
"#;
        let cfg = parse_config(toml).unwrap();
        assert_eq!(
            cfg.library_routes.get("*").map(String::as_str),
            Some("tcp://192.168.0.104:47788")
        );
        assert_eq!(resolve_library_route(&cfg.library_routes, "movies"), None);
    }

    #[test]
    fn parse_auto_reconnect_true() {
        let toml = r#"
[server]
url = "http://x"

[session]
auto_reconnect = true
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.auto_reconnect);
    }

    #[test]
    fn parse_auto_reconnect_defaults_false_when_absent() {
        let toml = r#"
[server]
url = "http://x"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(!cfg.auto_reconnect);
    }

    #[test]
    fn save_config_settings_round_trips_auto_reconnect_values() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "mbv-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("mbv")).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("MBV_SYSTEM");

        for auto_reconnect in [true, false] {
            let cfg = Config {
                server_url: "http://localhost:8096".into(),
                auto_reconnect,
                ..Default::default()
            };
            save_config_settings(&cfg).unwrap();

            let saved = std::fs::read_to_string(config_path()).unwrap();
            let reparsed = parse_config(&saved).unwrap();
            assert_eq!(reparsed.auto_reconnect, auto_reconnect);
        }

        std::env::remove_var("XDG_CONFIG_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn save_config_settings_preserves_general_feed_view_when_auto_reconnect_exists() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "mbv-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(dir.join("mbv")).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("MBV_SYSTEM");
        std::fs::write(
            config_path(),
            r#"
[server]
url = "http://localhost:8096"

[session]
auto_reconnect = true

[library]
feed_view_libraries = ["YouTube"]
"#,
        )
        .unwrap();

        let cfg = load_config().unwrap();
        assert_eq!(cfg.feed_view_libraries, vec!["youtube"]);

        save_config_settings(&cfg).unwrap();

        let saved = std::fs::read_to_string(config_path()).unwrap();
        let reparsed = parse_config(&saved).unwrap();
        assert_eq!(reparsed.feed_view_libraries, vec!["youtube"]);
        assert!(
            !saved.contains("feed_view_libraries = []"),
            "saved config should not overwrite the feed view selection with none:\n{saved}"
        );

        std::env::remove_var("XDG_CONFIG_HOME");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn parse_default_library_routes_when_absent() {
        let toml = r#"
[server]
url = "http://host"
"#;
        let cfg = parse_config(toml).unwrap();
        assert!(cfg.library_routes.is_empty());
    }

    #[test]
    fn resolve_library_route_has_no_wildcard_fallback() {
        let mut routes = std::collections::HashMap::new();
        routes.insert("music".to_string(), "tcp://192.168.0.104:47788".to_string());
        assert_eq!(
            resolve_library_route(&routes, "Music"),
            Some(crate::remote_player::DaemonEndpoint::Tcp(
                "192.168.0.104:47788".parse().unwrap()
            ))
        );
        assert_eq!(resolve_library_route(&routes, "movies"), None);
    }

    #[test]
    fn resolve_library_route_rejects_a_bare_device_name_as_malformed() {
        // A stale pre-#256 config entry (device name, no scheme) must
        // NOT silently resolve -- DaemonEndpoint::parse would otherwise
        // accept it as a bogus Unix(PathBuf) socket path. Library routing
        // is tcp://-only (#239 addendum), so anything that doesn't parse
        // to Tcp(_) is treated as malformed: logged and skipped.
        let mut routes = std::collections::HashMap::new();
        routes.insert("music".to_string(), "living-room-pc".to_string());
        assert_eq!(resolve_library_route(&routes, "music"), None);
    }

    #[test]
    fn resolve_library_route_rejects_unix_and_local_endpoints() {
        // Library routing is remote-only -- a unix:// or bare "local"
        // value is well-formed as a DaemonEndpoint but not a valid
        // library route, so it must still resolve to None.
        let mut routes = std::collections::HashMap::new();
        routes.insert("music".to_string(), "unix:///run/mbvd.sock".to_string());
        routes.insert("movies".to_string(), "local".to_string());
        assert_eq!(resolve_library_route(&routes, "music"), None);
        assert_eq!(resolve_library_route(&routes, "movies"), None);
    }

    #[test]
    fn parse_invalid_toml_errors() {
        assert!(parse_config("not [ valid toml !!!").is_err());
    }

    #[test]
    fn parse_music_levels_group_album() {
        let toml =
            "[server]\nurl = \"http://host\"\n[library.music]\nlevels = [\"group\", \"album\"]";
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.music_levels, vec!["group", "album"]);
    }

    #[test]
    fn parse_music_levels_album_only() {
        let toml = "[server]\nurl = \"http://host\"\n[library.music]\nlevels = [\"album\"]";
        let cfg = parse_config(toml).unwrap();
        assert_eq!(cfg.music_levels, vec!["album"]);
    }

    #[test]
    fn parse_music_levels_missing_defaults_empty() {
        let toml = "[server]\nurl = \"http://host\"";
        assert!(parse_config(toml).unwrap().music_levels.is_empty());
    }

    // always_play_next and start_on_queue live in [queue].
    #[test]
    fn parse_always_play_next_in_wrong_section_is_ignored() {
        let toml = "[server]\nurl = \"http://host\"\nalways_play_next = true";
        assert!(
            !parse_config(toml).unwrap().always_play_next,
            "always_play_next must be in [queue], not [server]"
        );
    }

    #[test]
    fn load_queue_state_backfills_missing_cursor() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        let temp = std::env::temp_dir().join(format!(
            "mbv-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state_dir = temp.join("mbv");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::env::set_var("XDG_STATE_HOME", &temp);
        std::fs::write(
            state_dir.join("queue_state.json"),
            r#"{"source":{"type":"unknown"},"last_played_item_id":null,"last_played_completed":false,"positions":{}}"#,
        )
        .unwrap();

        // Pre-"full items" on-disk files have no `items` field at all; it must
        // default to empty rather than fail to load.
        let state = load_queue_state().expect("queue state missing newer fields should still load");
        assert!(state.items.is_empty());
        assert_eq!(state.cursor, 0);

        std::env::remove_var("XDG_STATE_HOME");
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn library_position_state_round_trips_by_library_and_view() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        let temp = std::env::temp_dir().join(format!(
            "mbv-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::env::set_var("XDG_STATE_HOME", &temp);

        let mut state = LibraryPositionState::default();
        let views = state.libraries.entry("lib-movies".into()).or_default();
        views.default = Some(LibraryPosition {
            levels: vec![LibraryPositionLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                focused_item_id: Some("movie-2".into()),
                cursor_index: 7,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            }],
            feed_selected_group: 0,
            feed_video_cursor: 0,
            feed_video_scroll: 0,
        });
        views.power = Some(LibraryPosition {
            levels: vec![LibraryPositionLevel {
                parent_id: "genre-action".into(),
                title: "Action".into(),
                focused_item_id: Some("movie-9".into()),
                cursor_index: 2,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                letter_filter_index: None,
                library_total: None,
            }],
            feed_selected_group: 0,
            feed_video_cursor: 0,
            feed_video_scroll: 0,
        });

        save_library_position_state(&state);

        assert_eq!(load_library_position_state(), state);

        std::env::remove_var("XDG_STATE_HOME");
        let _ = std::fs::remove_dir_all(temp);
    }

    #[test]
    fn load_library_position_state_defaults_for_missing_or_invalid_file() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        let temp = std::env::temp_dir().join(format!(
            "mbv-config-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let state_dir = temp.join("mbv");
        std::fs::create_dir_all(&state_dir).unwrap();
        std::env::set_var("XDG_STATE_HOME", &temp);

        assert_eq!(
            load_library_position_state(),
            LibraryPositionState::default()
        );

        std::fs::write(state_dir.join("library_position_state.json"), "{not json").unwrap();

        assert_eq!(
            load_library_position_state(),
            LibraryPositionState::default()
        );

        std::env::remove_var("XDG_STATE_HOME");
        let _ = std::fs::remove_dir_all(temp);
    }

    // ── System-instance path routing ─────────────────────────────────────────
    //
    // `std::env::set_var`/`var` read and write the process's single, global
    // `environ` table with no synchronization of their own — mutating *any*
    // env var on one thread can race with a read of a *different* env var on
    // another thread (the underlying C `environ` array can be reallocated
    // out from under a concurrent reader). So every test anywhere in the
    // crate that touches ANY env var via these functions must serialize on
    // one shared lock, not just tests that happen to touch the same variable
    // name. This is THE single shared lock for that: src/app/action.rs,
    // src/app/actions.rs, and src/api.rs all reference this same
    // `SYS_ENV_LOCK` (via `crate::config::tests::SYS_ENV_LOCK`) rather than
    // defining their own — independent per-file mutexes don't exclude each
    // other and previously caused flaky cross-test env-var races (e.g. one
    // test's queue_state.json read intermittently coming back empty because
    // an unrelated, unguarded HOSTNAME mutation in api.rs raced it).
    use std::sync::Mutex;
    pub static SYS_ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn is_system_instance_false_without_env_var() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        assert!(!is_system_instance());
    }

    #[test]
    fn is_system_instance_true_with_env_var() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let result = is_system_instance();
        std::env::remove_var("MBV_SYSTEM");
        assert!(result);
    }

    #[test]
    fn is_system_instance_false_with_env_set_to_zero() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "0");
        let result = is_system_instance();
        std::env::remove_var("MBV_SYSTEM");
        assert!(!result);
    }

    #[test]
    fn is_system_instance_false_with_empty_env_var() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "");
        let result = is_system_instance();
        std::env::remove_var("MBV_SYSTEM");
        assert!(!result);
    }

    #[test]
    fn cache_dir_uses_system_path_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = cache_dir();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, std::path::PathBuf::from("/var/cache/mbv"));
    }

    #[test]
    fn cache_dir_uses_xdg_when_not_system() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        std::env::set_var("XDG_CACHE_HOME", "/tmp/xdg-test-cache");
        let path = cache_dir();
        std::env::remove_var("XDG_CACHE_HOME");
        assert_eq!(path, std::path::PathBuf::from("/tmp/xdg-test-cache/mbv"));
    }

    #[test]
    fn data_dir_system_or_local_uses_system_path_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = data_dir_system_or_local();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, std::path::PathBuf::from("/var/lib/mbv"));
    }

    #[test]
    fn config_path_uses_system_path_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = config_path();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, std::path::PathBuf::from("/etc/mbv/config.toml"));
    }

    #[test]
    fn mpv_ipc_path_uses_run_dir_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = mpv_ipc_path();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, "/run/mbv/mbv-mpv.sock");
    }

    #[test]
    fn control_socket_path_uses_run_dir_when_mbv_system_set() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let path = control_socket_path();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(path, "/run/mbv/mbv-ctrl.sock");
    }

    #[test]
    fn daemon_server_tcp_listen_defaults_for_system_instance() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::set_var("MBV_SYSTEM", "1");
        let cfg = parse_config("[server]\nurl = \"http://host\"").unwrap();
        std::env::remove_var("MBV_SYSTEM");
        assert_eq!(
            cfg.daemon_server_tcp_listen,
            DEFAULT_SYSTEM_DAEMON_TCP_LISTEN
        );
    }

    #[test]
    fn mpv_ipc_path_uses_xdg_runtime_dir_when_not_system() {
        let _g = SYS_ENV_LOCK.lock().unwrap();
        std::env::remove_var("MBV_SYSTEM");
        std::env::set_var("XDG_RUNTIME_DIR", "/run/user/1000");
        let path = mpv_ipc_path();
        std::env::remove_var("XDG_RUNTIME_DIR");
        assert_eq!(path, "/run/user/1000/mbv-mpv.sock");
    }

    #[test]
    fn save_and_load_last_remote_connection_round_trips_library_route() {
        let _guard = TestStateDirGuard::new();
        let conn = LastRemoteConnection::LibraryRoute {
            library: "music".to_string(),
        };

        assert!(save_last_remote_connection(Some(&conn)).is_ok());

        assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
    }

    #[test]
    fn save_and_load_last_remote_connection_round_trips_direct_session() {
        let _guard = TestStateDirGuard::new();
        let conn = LastRemoteConnection::DirectSession {
            device_name: "living-room-mbv".to_string(),
        };

        assert!(save_last_remote_connection(Some(&conn)).is_ok());

        assert_eq!(load_last_remote_connection().unwrap(), Some(conn));
    }

    #[test]
    fn save_last_remote_connection_none_clears_a_previously_saved_record() {
        let _guard = TestStateDirGuard::new();
        assert!(
            save_last_remote_connection(Some(&LastRemoteConnection::LibraryRoute {
                library: "music".to_string(),
            }))
            .is_ok()
        );

        assert!(save_last_remote_connection(None).is_ok());

        assert_eq!(load_last_remote_connection().unwrap(), None);
    }

    #[test]
    fn load_last_remote_connection_returns_none_when_no_file_exists() {
        let _guard = TestStateDirGuard::new();
        assert_eq!(load_last_remote_connection().unwrap(), None);
    }

    #[test]
    fn save_last_remote_connection_reports_remove_failure_with_path() {
        let dir =
            std::env::temp_dir().join(format!("mbv-save-state-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let error = save_last_remote_connection_at(&dir, None).unwrap_err();
        assert!(error.contains("remove"));
        assert!(error.contains(dir.to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn load_last_remote_connection_reports_read_failure_with_path() {
        let dir =
            std::env::temp_dir().join(format!("mbv-load-state-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let error = load_last_remote_connection_at(&dir).unwrap_err();
        assert!(error.starts_with("read "));
        assert!(error.contains(dir.to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_config_settings_reports_rename_failure_with_path() {
        let dir =
            std::env::temp_dir().join(format!("mbv-save-config-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let error = write_config_text_at(&dir, "").unwrap_err();
        assert!(error.contains("rename"));
        assert!(error.contains(dir.to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_config_settings_reports_read_failure_with_path() {
        let dir =
            std::env::temp_dir().join(format!("mbv-read-config-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let error = save_config_settings_at(&Config::default(), &dir).unwrap_err();
        assert!(error.contains("read"));
        assert!(error.contains(dir.to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_config_settings_reports_parse_failure_with_path() {
        let dir =
            std::env::temp_dir().join(format!("mbv-parse-config-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "this = [is malformed").unwrap();
        let error = save_config_settings_at(&Config::default(), &path).unwrap_err();
        assert!(error.contains("parse"));
        assert!(error.contains(path.to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn save_config_settings_reports_write_failure_with_path() {
        let dir =
            std::env::temp_dir().join(format!("mbv-write-config-error-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        std::fs::write(&path, "").unwrap();
        std::fs::create_dir(path.with_extension("toml.tmp")).unwrap();
        let error = save_config_settings_at(&Config::default(), &path).unwrap_err();
        assert!(error.contains("write"));
        assert!(error.contains(path.with_extension("toml.tmp").to_str().unwrap()));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
