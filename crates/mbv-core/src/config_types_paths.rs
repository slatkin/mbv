use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Config {
    /// Singleton Emby setup. `server_url` remains only as a legacy projection
    /// for callers that still construct provider clients.
    pub emby_setup: Option<EmbySetup>,
    /// Singleton Audiobookshelf setup. Its API key lives only in the
    /// Audiobookshelf Service secret file.
    pub audiobookshelf_setup: Option<AudiobookshelfSetup>,
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
    /// Optional user-calibrated estimate of buffering after mbv starts
    /// writing PCM to the pipe. This is deliberately not a downstream
    /// consumer setting: mbv never queries or controls that consumer.
    pub audio_pipe_playout_delay_ms: Option<u64>,
    pub always_play_next: bool,
    pub consume_videos: bool,
    pub consume_audio: bool,
    pub always_skip_intro: bool,
    pub show_systray_icon: bool,
    pub no_scripts: bool,
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
    /// RSS feed URL displayed in the playback panel when idle.
    pub idle_feed_rss_url: String,
    /// Seconds between idle feed item rotations (minimum 1).
    pub idle_feed_rotation_secs: u64,
    /// ── Feed subscriptions (#471) ─────────────────────────────────
    /// User-configured RSS/Atom subscriptions shown in the Feeds tab
    /// (`[[feeds]]` array-of-tables in config.toml). The only persisted
    /// feed data: per-entry playback state is never saved.
    pub feeds: Vec<FeedSubscription>,
    /// ── Shared-data hosting (daemon) ──────────────────────────────
    /// When `true`, the daemon opens a dedicated shared-data listener and
    /// the redb database. Disabled by default: no listener or database is
    /// created unless this is explicitly enabled.
    pub shared_data_enabled: bool,
    /// Endpoint for the shared-data listener (e.g. `192.168.1.20:47789` for
    /// private TCP, or a Unix socket path). Empty = disabled. TCP endpoints
    /// are limited to loopback/private addresses; TLS is optional.
    pub shared_data_listen: String,
    /// Path to the TLS certificate file for the shared-data listener.
    /// Optional when `shared_data_listen` is a TCP endpoint; cert and key must
    /// be supplied together to enable TLS.
    pub shared_data_tls_cert_path: String,
    /// Path to the TLS private key file for the shared-data listener.
    /// Optional when `shared_data_listen` is a TCP endpoint; cert and key must
    /// be supplied together to enable TLS.
    pub shared_data_tls_key_path: String,
    /// ── Shared-data client ────────────────────────────────────────
    /// Explicit shared-data endpoint for the client. Empty = disabled
    /// (local-only behavior). Must be a loopback/private TCP or Unix endpoint;
    /// WAN endpoints are rejected at validation time.
    pub shared_data_endpoint: String,
}

/// Emby-specific setup (server URL + user ID) stored in config.toml `[server]`.
/// The corresponding API token is stored separately in a per-Service mode-0600
/// secret file, never in config.toml. The flat `server_url`/`username`/
/// `password`/`api_key` fields on `Config` serve backward compat during
/// migration and will be removed once all callers go through `EmbySetup`
/// and runtime service state.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EmbySetup {
    pub server_url: String,
    pub user_id: String,
}

impl EmbySetup {
    pub fn new(server_url: impl Into<String>, user_id: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into().trim().trim_end_matches('/').to_string(),
            user_id: user_id.into().trim().to_string(),
        }
    }
}

/// Audiobookshelf-specific setup persisted in config.toml. The API key is a
/// Service secret and is deliberately not part of this record.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudiobookshelfSetup {
    pub server_url: String,
}

impl AudiobookshelfSetup {
    pub fn new(server_url: impl Into<String>) -> Self {
        Self {
            server_url: server_url.into().trim().trim_end_matches('/').to_string(),
        }
    }
}

/// Singleton Service kind identifier. Each variant represents exactly one
/// configured instance of that Service within mbv.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ServiceKind {
    Emby,
    /// Reserved; not yet implemented. Added now to establish the pattern
    /// and the per-Service secret path. No Audiobookshelf auth, browsing,
    /// playback, or credential logic is introduced here.
    Audiobookshelf,
}

impl ServiceKind {
    /// Filesystem-safe name for this Service kind, used for per-Service
    /// secret file names.
    pub fn secret_name(self) -> &'static str {
        match self {
            ServiceKind::Emby => "emby",
            ServiceKind::Audiobookshelf => "audiobookshelf",
        }
    }
}

pub const DEFAULT_SYSTEM_DAEMON_TCP_LISTEN: &str = "0.0.0.0:47788";
pub const DEFAULT_SHARED_DATA_TCP_PORT: u16 = 47789;

impl Default for Config {
    fn default() -> Self {
        Config {
            emby_setup: None,
            audiobookshelf_setup: None,
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
            audio_pipe_playout_delay_ms: None,
            always_play_next: false,
            consume_videos: false,
            consume_audio: false,
            always_skip_intro: false,
            show_systray_icon: true,
            no_scripts: false,
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
            idle_feed_rss_url: "https://novaramedia.com/feed/".to_string(),
            idle_feed_rotation_secs: 10,
            feeds: vec![],
            shared_data_enabled: false,
            shared_data_listen: String::new(),
            shared_data_tls_cert_path: String::new(),
            shared_data_tls_key_path: String::new(),
            shared_data_endpoint: String::new(),
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

/// Resolve the shared-data listener without requiring a second address in the
/// normal configuration. System daemons follow the existing daemon TCP bind;
/// local daemons use a sibling Unix socket.
pub fn shared_data_listen(cfg: &Config) -> String {
    let configured = cfg.shared_data_listen.trim();
    if !configured.is_empty() {
        return configured.to_string();
    }
    if let Ok(mut address) = cfg
        .daemon_server_tcp_listen
        .trim()
        .parse::<std::net::SocketAddr>()
    {
        address.set_port(DEFAULT_SHARED_DATA_TCP_PORT);
        return address.to_string();
    }
    PathBuf::from(control_socket_path())
        .with_file_name("mbv-shared.sock")
        .display()
        .to_string()
}

/// Validates shared-data configuration. Returns `Err` with a human-readable
/// message if the configuration is invalid.
pub fn validate_shared_data_config(cfg: &Config) -> Result<(), String> {
    if !cfg.shared_data_enabled {
        return Ok(());
    }
    let resolved_listen = shared_data_listen(cfg);
    let listen = resolved_listen.as_str();
    if listen.starts_with('/') || listen.starts_with("unix://") {
        if !cfg.shared_data_tls_cert_path.trim().is_empty()
            || !cfg.shared_data_tls_key_path.trim().is_empty()
        {
            return Err("shared_data TLS paths require a TCP listener".to_string());
        }
        return Ok(());
    }
    validate_shared_tcp_listener(listen, "shared_data.listen")?;
    let has_cert = !cfg.shared_data_tls_cert_path.trim().is_empty();
    let has_key = !cfg.shared_data_tls_key_path.trim().is_empty();
    if has_cert != has_key {
        return Err(
            "shared_data.tls_cert_path and shared_data.tls_key_path must be supplied together"
                .to_string(),
        );
    }
    Ok(())
}

fn validate_shared_tcp_listener(address: &str, field: &str) -> Result<(), String> {
    let addr = shared_tcp_address(address);
    if addr
        .parse::<std::net::SocketAddr>()
        .is_ok_and(|address| address.ip().is_unspecified())
    {
        return Ok(());
    }
    validate_shared_tcp_address(address, field)
}

fn validate_shared_tcp_address(address: &str, field: &str) -> Result<(), String> {
    let addr = shared_tcp_address(address);
    let is_localhost = addr
        .rsplit_once(':')
        .map(|(host, _)| host.eq_ignore_ascii_case("localhost"))
        .unwrap_or(false);
    if is_localhost {
        return Ok(());
    }

    let socket_addr = addr.parse::<std::net::SocketAddr>().map_err(|_| {
        format!(
            "{field} must use localhost or a literal loopback/private IP address, not a hostname ({addr})"
        )
    })?;
    let ip = socket_addr.ip();
    let allowed = match ip {
        std::net::IpAddr::V4(ip) => ip.is_loopback() || ip.is_private() || ip.is_link_local(),
        std::net::IpAddr::V6(ip) => {
            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
        }
    };
    if allowed {
        Ok(())
    } else {
        Err(format!(
            "{field} must use a loopback/private-network address; WAN address rejected ({addr})"
        ))
    }
}

pub(crate) fn shared_tcp_address(address: &str) -> &str {
    address
        .strip_prefix("tcp://")
        .or_else(|| address.strip_prefix("tls://"))
        .unwrap_or(address)
}

/// Validates the client-side shared-data endpoint before any credentials are sent.
pub fn validate_shared_data_endpoint(endpoint: &str) -> Result<(), String> {
    let ep = endpoint.trim();
    if ep.is_empty() {
        return Ok(());
    }
    // Unix domain sockets are always accepted (permissions protect the socket).
    if ep.starts_with('/') || ep.starts_with("unix://") {
        return Ok(());
    }
    if ep.starts_with("tcp://") || ep.starts_with("tls://") {
        return validate_shared_tcp_address(ep, "shared_data.endpoint");
    }
    Err(format!(
        "shared_data.endpoint has unrecognized scheme: {ep}; \
         expected tcp://, tls://, or a Unix socket path"
    ))
}

#[cfg(test)]
mod shared_data_endpoint_tests {
    use super::{validate_shared_data_config, validate_shared_data_endpoint, Config};

    #[test]
    fn accepts_loopback_and_private_shared_endpoints_without_tls() {
        for endpoint in [
            "tcp://localhost:47789",
            "tcp://127.0.0.1:47789",
            "tcp://10.0.0.8:47789",
            "tcp://172.16.4.8:47789",
            "tcp://192.168.1.8:47789",
            "tcp://[fd12:3456::8]:47789",
        ] {
            assert!(
                validate_shared_data_endpoint(endpoint).is_ok(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn rejects_wan_shared_endpoints() {
        for endpoint in [
            "tcp://0.0.0.0:47789",
            "tcp://8.8.8.8:47789",
            "tls://8.8.8.8:47789",
            "tcp://mbvd.example.test:47789",
        ] {
            assert!(
                validate_shared_data_endpoint(endpoint).is_err(),
                "{endpoint}"
            );
        }
    }

    #[test]
    fn hosting_allows_plaintext_private_tcp_and_optional_tls() {
        let config = Config {
            shared_data_enabled: true,
            shared_data_listen: "192.168.1.8:47789".to_string(),
            ..Config::default()
        };
        assert!(validate_shared_data_config(&config).is_ok());

        let config = Config {
            shared_data_tls_cert_path: "cert.p12".to_string(),
            ..config
        };
        assert!(validate_shared_data_config(&config).is_err());
        let config = Config {
            shared_data_tls_key_path: "key.pem".to_string(),
            ..config
        };
        assert!(validate_shared_data_config(&config).is_ok());
    }

    #[test]
    fn shared_tcp_schemes_are_bindable_after_normalization() {
        for address in ["tcp://127.0.0.1:0", "tls://127.0.0.1:0"] {
            let listener = std::net::TcpListener::bind(super::shared_tcp_address(address)).unwrap();
            assert_eq!(
                listener.local_addr().unwrap().ip(),
                "127.0.0.1".parse::<std::net::IpAddr>().unwrap()
            );
        }
    }

    #[test]
    fn shared_data_uses_daemon_bind_address_with_its_own_port() {
        let config = Config {
            daemon_server_tcp_listen: "0.0.0.0:47788".to_string(),
            ..Config::default()
        };
        assert_eq!(super::shared_data_listen(&config), "0.0.0.0:47789");
    }
}

pub(crate) fn config_dir() -> PathBuf {
    // See `TEST_CONFIG_DIR_OVERRIDE` / `TestStateDirGuard`: without this,
    // `config_dir()` (and therefore `config_path()`/`save_config_settings`)
    // had no test isolation at all -- unlike `state_dir()`, *every* call in
    // a test build hit the real `$XDG_CONFIG_HOME`/`~/.config/mbv` on disk.
    // Any App-level test that reached a synchronous config-saving path
    // (e.g. `cycle_subtitle_mode`, `handle_library_routes_enter`, closing a multiselect
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
// App-level settings toggles (`cycle_subtitle_mode`, `handle_library_routes_enter`,
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

pub(crate) fn state_dir() -> PathBuf {
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

// Path helper functions (config_dir, cache_dir, state_dir) remain above.
// Path-related public functions have been extracted to config_paths.rs,
// included in this module via config.rs.

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
    pub items: Vec<crate::playback_queue::QueueItem>,
    #[serde(default)]
    pub cursor: usize,
    pub last_played_item_id: Option<String>,
    pub last_played_completed: bool,
    // Per-item resume positions saved at quit time. Used on restore to override stale Emby
    // UserData when a fresh launch races with Emby's async position write.
    #[serde(default)]
    pub positions: std::collections::HashMap<String, i64>,
}

impl QueueState {
    /// Returns only the Emby items from the queue state.
    /// Used by the UI layer (PlayerTab) and ctrl boundary, which only
    /// operate on Emby items.
    pub fn emby_items(&self) -> Vec<crate::api::EmbyItem> {
        self.items
            .iter()
            .filter_map(|qi| match qi {
                crate::playback_queue::QueueItem::Emby(e) => Some((**e).clone()),
                crate::playback_queue::QueueItem::Feed(_) => None,
            })
            .collect()
    }

    /// Consumes self and returns the inner `Vec<QueueItem>`.
    pub fn into_queue_items(self) -> Vec<crate::playback_queue::QueueItem> {
        self.items
    }

    /// Creates a `QueueState` from `Vec<EmbyItem>`, wrapping each as
    /// `QueueItem::Emby`. Convenience for tests and callers that only
    /// deal with Emby items.
    pub fn from_emby_items(
        items: Vec<crate::api::EmbyItem>,
        cursor: usize,
        source: QueueSource,
    ) -> Self {
        Self {
            items: items
                .into_iter()
                .map(|item| crate::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor,
            source,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }
    }
}

/// One library position per library (#361 collapsed the old two-scope
/// `{default, power}` split -- there is only one view now, so there is only
/// one saved position per library).
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct LibraryPositionState {
    #[serde(default)]
    pub libraries: std::collections::HashMap<String, LibraryPosition>,
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
