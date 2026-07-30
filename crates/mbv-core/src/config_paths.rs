// Path helper functions extracted from config_types_paths.rs.
// This file is included via `include!("config_paths.rs")` in config.rs,
// so all items share the same module scope. Items from config_types_paths.rs
// (types, config_dir, cache_dir, state_dir, is_system_instance) are already
// in scope.

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

pub fn queue_state_path() -> PathBuf {
    state_dir().join("queue_state.json")
}

pub fn library_position_state_path() -> PathBuf {
    state_dir().join("library_position_state.json")
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
