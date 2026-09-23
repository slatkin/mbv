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
    pub show_audio_window: bool,
    pub use_mpv_config: bool,
    pub video_cache_forward_mb: u32,
    pub video_cache_back_mb: u32,
    pub audio_pipe_enabled: bool,
    pub audio_pipe_path: String,
    pub audio_pipe_samplerate: u32, // fixed output rate forced on the pipe (Hz); mpv resamples everything to this
    pub audio_pipe_bitdepth: u8,    // fixed PCM bit depth for the pipe (16|24|32)
    /// Optional user-calibrated estimate of buffering after mbv starts
    /// writing PCM to the pipe. This is deliberately not a downstream
    /// consumer setting: mbv never queries or controls that consumer.
    pub audio_pipe_playout_delay_ms: Option<u64>,
    /// mpv's complete ALSA device identifier for packaged-daemon clocked
    /// output: `alsa` for the default endpoint, or `alsa/<device>` for an
    /// exact one. Restart-required and owner-local; bare mode and the Local
    /// daemon never apply it. An absent value resolves to `alsa`.
    pub audio_device: String,
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
    /// Gates terminal mouse capture (ADR 0024): when off, capture is never
    /// enabled and no mouse events are delivered. Editable via the F2
    /// Settings panel's Display section; default on.
    pub mouse_support: bool,
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
    /// Compiled `[keys]` configuration (change `add-configurable-keybinds`,
    /// D3): the optional prefix chord plus per-section router overrides and
    /// prefix-namespace assignments, validated against the keybind registry
    /// at parse time. Absent section resolves to the all-defaults value.
    pub keybinds: crate::keybinds::Keybinds,
}

pub const DEFAULT_SYSTEM_DAEMON_TCP_LISTEN: &str = "0.0.0.0:47788";

pub const DEFAULT_VIDEO_CACHE_FORWARD_MB: u32 = 50;
pub const DEFAULT_VIDEO_CACHE_BACK_MB: u32 = 100;

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
            show_audio_window: false,
            use_mpv_config: false,
            video_cache_forward_mb: DEFAULT_VIDEO_CACHE_FORWARD_MB,
            video_cache_back_mb: DEFAULT_VIDEO_CACHE_BACK_MB,
            audio_pipe_enabled: false,
            audio_pipe_path: "/tmp/mbv-pipe".to_string(),
            audio_pipe_samplerate: 192_000,
            audio_pipe_bitdepth: 32,
            audio_pipe_playout_delay_ms: None,
            audio_device: "alsa".to_string(),
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
            mouse_support: true,
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
            keybinds: crate::keybinds::Keybinds::default(),
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

/// True when `value` is a valid `audio_device` identifier: exactly `alsa`,
/// or `alsa/<device>` naming an exact ALSA endpoint.
pub fn is_valid_audio_device(value: &str) -> bool {
    value == "alsa" || value.starts_with("alsa/")
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
