use serde::{Deserialize, Serialize};

/// Canonical document kinds stored per Emby user in the shared `redb` database.
/// Each kind maps to exactly one existing local state file (except roaming settings,
/// which is a new synthetic document).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SharedDocumentKind {
    /// Complete `QueueState` JSON (source, items, cursor, last-played, positions).
    QueueState,
    /// Complete `LibraryPositionState` JSON.
    LibraryPositionState,
    /// Complete `LastRemoteConnection` JSON.
    LastRemoteConnection,
    /// Roaming settings: exactly `auto_reconnect` and `library_routes`.
    RoamingSettings,
}

impl SharedDocumentKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::QueueState => "queue_state",
            Self::LibraryPositionState => "library_position_state",
            Self::LastRemoteConnection => "last_remote_connection",
            Self::RoamingSettings => "roaming_settings",
        }
    }
}

/// Snapshot of all four per-user documents.
pub type SharedDocumentTuple = (
    Option<SharedRecord>,
    Option<SharedRecord>,
    Option<SharedRecord>,
    Option<SharedRecord>,
);

/// A revisioned record stored in `redb`. The revision is an independent
/// monotonic counter per document kind. Revision zero is reserved for
/// absence; the first committed value receives revision one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedRecord {
    pub revision: u64,
    /// The document value as raw JSON bytes. Using raw bytes avoids
    /// double-serialization when the caller already holds JSON.
    #[serde(default)]
    pub value: serde_json::Value,
}

/// Minimal envelope used for CAS operations in the storage worker.
/// The caller sends the expected revision; the worker compares it against
/// the current record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedReadRequest {
    pub kind: SharedDocumentKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedReadResponse {
    pub kind: SharedDocumentKind,
    pub record: Option<SharedRecord>,
}

/// Create-if-absent: the document must not already exist.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedCreateRequest {
    pub kind: SharedDocumentKind,
    pub value: serde_json::Value,
}

/// Update with expected revision (CAS). If the expected revision does not
/// match the current record, the update is rejected.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedUpdateRequest {
    pub kind: SharedDocumentKind,
    pub expected_revision: u64,
    pub value: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SharedWriteOutcome {
    /// The write was committed; the record carries the new revision.
    Committed(SharedRecord),
    /// The write was rejected because the expected revision was stale.
    /// The payload is the current winning record.
    StaleWrite { current: SharedRecord },
    /// The document did not exist and the creation was committed.
    Created(SharedRecord),
    /// The document already exists; creation was rejected.
    /// The payload is the existing record.
    AlreadyExists { current: SharedRecord },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedWriteResponse {
    pub kind: SharedDocumentKind,
    pub outcome: SharedWriteOutcome,
}

/// Full snapshot returned to a client on initial connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedSnapshotResponse {
    pub queue_state: Option<SharedRecord>,
    pub library_position_state: Option<SharedRecord>,
    pub last_remote_connection: Option<SharedRecord>,
    pub roaming_settings: Option<SharedRecord>,
}

/// Notification broadcast to other same-user shared-data clients after a
/// committed write. Recipients compare revisions to avoid overwriting
/// newer state.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SharedDocumentNotification {
    pub kind: SharedDocumentKind,
    pub record: SharedRecord,
}

/// Roaming settings stored in the shared database and mirrored locally.
/// Contains exactly the two fields that roam across machines.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoamingSettings {
    pub auto_reconnect: bool,
    pub library_routes: std::collections::HashMap<String, String>,
}

/// Local filesystem path for the roaming-settings mirror. Stored separately
/// from `config.toml` so the original config remains an expression of
/// machine-local intent.
pub fn roaming_settings_mirror_path() -> std::path::PathBuf {
    crate::config::state_dir().join("roaming_settings.json")
}

pub fn save_roaming_settings_mirror(settings: &RoamingSettings) -> Result<(), String> {
    let path = roaming_settings_mirror_path();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("create directory {}: {e}", dir.display()))?;
    }
    let json =
        serde_json::to_string(settings).map_err(|e| format!("serialize roaming settings: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, &json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, &path)
        .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
}

pub fn load_roaming_settings_mirror() -> Option<RoamingSettings> {
    let text = std::fs::read_to_string(roaming_settings_mirror_path()).ok()?;
    serde_json::from_str(&text).ok()
}

pub fn clear_roaming_settings_mirror() {
    let _ = std::fs::remove_file(roaming_settings_mirror_path());
}

/// Apply a shared record to the corresponding local persistence document.
/// The existing persistence helpers retain the established JSON schemas and
/// atomic temp-file/rename behavior.
pub fn mirror_shared_document(
    kind: SharedDocumentKind,
    value: &serde_json::Value,
) -> Result<(), String> {
    match kind {
        SharedDocumentKind::QueueState => {
            let state: crate::config::QueueState = serde_json::from_value(value.clone())
                .map_err(|e| format!("parse shared queue state: {e}"))?;
            crate::config::save_queue_state(&state)
        }
        SharedDocumentKind::LibraryPositionState => {
            let state: crate::config::LibraryPositionState = serde_json::from_value(value.clone())
                .map_err(|e| format!("parse shared library position state: {e}"))?;
            crate::config::save_library_position_state(&state);
            Ok(())
        }
        SharedDocumentKind::LastRemoteConnection => {
            let state = if value.is_null() {
                None
            } else {
                Some(
                    serde_json::from_value::<crate::config::LastRemoteConnection>(value.clone())
                        .map_err(|e| format!("parse shared last remote connection: {e}"))?,
                )
            };
            crate::config::save_last_remote_connection(state.as_ref())
        }
        SharedDocumentKind::RoamingSettings => {
            let settings: RoamingSettings = serde_json::from_value(value.clone())
                .map_err(|e| format!("parse shared roaming settings: {e}"))?;
            save_roaming_settings_mirror(&settings)
        }
    }
}
