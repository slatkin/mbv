// Queue, library-position, and last-remote-connection state persistence.

use super::{
    ConfigError, LibraryPositionState, QueueState, home_latest_launch_path,
    library_position_state_path, queue_state_path, state_dir, stay_alive_queue_state_path,
};
use std::path::PathBuf;

pub fn save_queue_state(state: &QueueState) -> Result<(), ConfigError> {
    save_json_atomic(&queue_state_path(), state, "queue state")
}

#[must_use]
pub fn load_queue_state() -> Option<QueueState> {
    load_json(&queue_state_path(), "queue_state.json")
}

pub(super) fn save_json_atomic<T: serde::Serialize>(
    path: &std::path::Path,
    state: &T,
    what: &str,
) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| ConfigError::state(format!("create directory {}: {e}", dir.display())))?;
    }
    let json = serde_json::to_string(state)
        .map_err(|e| ConfigError::state(format!("serialize {what}: {e}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)
        .map_err(|e| ConfigError::state(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        ConfigError::state(format!(
            "rename {} to {}: {e}",
            tmp.display(),
            path.display()
        ))
    })
}

pub(super) fn load_json<T: serde::de::DeserializeOwned>(
    path: &std::path::Path,
    what: &str,
) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(state) => Some(state),
        Err(e) => {
            tracing::warn!(
                name: "queue.restore.failed",
                target: "queue",
                state = what,
                error = %e,
                "queue not restored"
            );
            None
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct StayAliveQueueState {
    pub queue: QueueState,
    pub lineage: mbv_queue::QueueLineage,
}

pub fn save_stay_alive_queue_state_at(
    path: &std::path::Path,
    state: &StayAliveQueueState,
) -> Result<(), ConfigError> {
    save_json_atomic(path, state, "owner queue")
}

pub fn save_stay_alive_queue_state(state: &StayAliveQueueState) -> Result<(), ConfigError> {
    save_stay_alive_queue_state_at(&stay_alive_queue_state_path(), state)
}

#[must_use]
pub fn load_stay_alive_queue_state_at(path: &std::path::Path) -> Option<StayAliveQueueState> {
    load_json(path, "owner queue state")
}

#[must_use]
pub fn load_stay_alive_queue_state() -> Option<StayAliveQueueState> {
    load_stay_alive_queue_state_at(&stay_alive_queue_state_path())
}

#[must_use]
pub fn legacy_queue_for_owner_if_absent(
    owner_path: &std::path::Path,
    legacy: Option<QueueState>,
) -> Option<StayAliveQueueState> {
    if owner_path.exists() {
        return None;
    }
    legacy.map(|queue| StayAliveQueueState {
        queue,
        lineage: mbv_queue::QueueLineage::default(),
    })
}

pub fn clear_queue_state() -> Result<(), ConfigError> {
    let path = queue_state_path();
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(ConfigError::state(format!(
            "remove {}: {error}",
            path.display()
        ))),
    }
}

const HOME_LATEST_LAUNCH_STATE_VERSION: u8 = 1;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct HomeLatestLaunchState {
    version: u8,
    launch_secs: u64,
}

pub(super) fn save_home_latest_launch_at(
    path: &std::path::Path,
    launch_secs: u64,
) -> Result<(), ConfigError> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| {
            ConfigError::launch(format!("create directory {}: {error}", dir.display()))
        })?;
    }
    let state = HomeLatestLaunchState {
        version: HOME_LATEST_LAUNCH_STATE_VERSION,
        launch_secs,
    };
    let json = serde_json::to_string(&state)
        .map_err(|error| ConfigError::launch(format!("serialize {}: {error}", path.display())))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, json)
        .map_err(|error| ConfigError::launch(format!("write {}: {error}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|error| {
        ConfigError::launch(format!(
            "rename {} to {}: {error}",
            tmp.display(),
            path.display()
        ))
    })
}

pub fn save_home_latest_launch(launch_secs: u64) -> Result<(), ConfigError> {
    if launch_secs == 0 {
        return Err(ConfigError::launch("launch timestamp must be positive"));
    }
    save_home_latest_launch_at(&home_latest_launch_path(), launch_secs)
}

pub(super) fn load_home_latest_launch_at(path: &std::path::Path) -> Option<u64> {
    let text = std::fs::read_to_string(path).ok()?;
    let state: HomeLatestLaunchState = serde_json::from_str(&text).ok()?;
    (state.version == HOME_LATEST_LAUNCH_STATE_VERSION).then_some(state.launch_secs)
}

#[must_use]
pub fn load_home_latest_launch() -> Option<u64> {
    load_home_latest_launch_at(&home_latest_launch_path())
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

pub(super) fn last_remote_connection_path() -> PathBuf {
    state_dir().join("last_remote_connection.json")
}

/// Persists (or, given `None`, clears) the connection active at exit.
/// Called from `App::teardown` only when `auto_reconnect` is
/// enabled -- when the feature is off, this file is never written or
/// read, by design (Task 1's `Global Constraints`).
pub(super) fn save_last_remote_connection_at(
    path: &std::path::Path,
    conn: Option<&LastRemoteConnection>,
) -> Result<(), ConfigError> {
    let Some(conn) = conn else {
        return match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(ConfigError::state(format!(
                "remove {}: {e}",
                path.display()
            ))),
        };
    };
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| ConfigError::state(format!("create directory {}: {e}", dir.display())))?;
    }
    let json = serde_json::to_string(conn)
        .map_err(|e| ConfigError::state(format!("serialize {}: {e}", path.display())))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)
        .map_err(|e| ConfigError::state(format!("write {}: {e}", tmp.display())))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        ConfigError::state(format!(
            "rename {} to {}: {e}",
            tmp.display(),
            path.display()
        ))
    })
}

pub fn save_last_remote_connection(conn: Option<&LastRemoteConnection>) -> Result<(), ConfigError> {
    save_last_remote_connection_at(&last_remote_connection_path(), conn)
}

pub(super) fn load_last_remote_connection_at(
    path: &std::path::Path,
) -> Result<Option<LastRemoteConnection>, ConfigError> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(ConfigError::state(format!("read {}: {e}", path.display()))),
    };
    match serde_json::from_str(&text) {
        Ok(conn) => Ok(Some(conn)),
        Err(e) => {
            std::fs::remove_file(path).map_err(|remove_error| {
                ConfigError::state(format!(
                    "parse {}: {e}; remove corrupt {}: {remove_error}",
                    path.display(),
                    path.display()
                ))
            })?;
            Err(ConfigError::state(format!(
                "parse {}: {e}; corrupt file removed",
                path.display()
            )))
        }
    }
}

pub fn load_last_remote_connection() -> Result<Option<LastRemoteConnection>, ConfigError> {
    load_last_remote_connection_at(&last_remote_connection_path())
}

pub fn save_library_position_state(state: &LibraryPositionState) {
    let _ = save_library_position_state_result(state);
}

pub fn save_library_position_state_result(state: &LibraryPositionState) -> Result<(), ConfigError> {
    let path = library_position_state_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|error| {
            ConfigError::state(format!("create directory {}: {error}", dir.display()))
        })?;
    }
    let json = serde_json::to_string(state)
        .map_err(|error| ConfigError::state(format!("serialize positions: {error}")))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json)
        .map_err(|error| ConfigError::state(format!("write {}: {error}", tmp.display())))?;
    std::fs::rename(&tmp, &path).map_err(|error| {
        ConfigError::state(format!(
            "rename {} to {}: {error}",
            tmp.display(),
            path.display()
        ))
    })
}

#[must_use]
pub fn load_library_position_state() -> LibraryPositionState {
    let Ok(text) = std::fs::read_to_string(library_position_state_path()) else {
        return LibraryPositionState::default();
    };
    match serde_json::from_str(&text) {
        Ok(state) => state,
        Err(e) => {
            tracing::warn!(
                name: "library_position.restore.failed",
                target: "library_position",
                error = %e,
                "library position state failed to parse"
            );
            LibraryPositionState::default()
        }
    }
}
