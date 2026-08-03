use serde::{Deserialize, Serialize};

use crate::shared_state::{SharedDocumentKind, SharedRecord};

/// Protocol version for the shared-data service. Additive changes get a
/// capability string, not a version bump (matching the ctrl protocol convention).
pub const SHARED_DATA_PROTOCOL_VERSION: u32 = 1;

/// Capability string advertised during the shared-data handshake.
pub const SHARED_DATA_CAP_V1: &str = "shared-mbv-state-v1";

/// Hello message sent by the daemon immediately after connection.
#[derive(Serialize, Deserialize)]
pub struct SharedDataHello {
    pub protocol_version: u32,
    pub capabilities: Vec<String>,
}

impl SharedDataHello {
    pub fn current() -> Self {
        Self {
            protocol_version: SHARED_DATA_PROTOCOL_VERSION,
            capabilities: vec![SHARED_DATA_CAP_V1.to_string()],
        }
    }
}

/// Commands from the client.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SharedDataCmd {
    /// Client hello with authentication token.
    Hello { auth_token: String },
    /// Request a full snapshot of all four documents.
    Snapshot,
    /// Create a document that does not yet exist.
    CreateDocument {
        kind: SharedDocumentKind,
        value: serde_json::Value,
    },
    /// Update a document with expected revision (CAS).
    UpdateDocument {
        kind: SharedDocumentKind,
        expected_revision: u64,
        value: serde_json::Value,
    },
}

/// Events from the daemon to the client.
#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SharedDataEvent {
    /// Daemon hello, sent immediately after connection.
    Hello(SharedDataHello),
    /// Authentication succeeded.
    AuthOk { user_id: String },
    /// Authentication failed.
    AuthFailed { reason: String },
    /// Full snapshot response.
    Snapshot {
        queue_state: Option<SharedRecord>,
        library_position_state: Option<SharedRecord>,
        last_remote_connection: Option<SharedRecord>,
        roaming_settings: Option<SharedRecord>,
    },
    /// Document created successfully.
    DocumentCreated {
        kind: SharedDocumentKind,
        record: SharedRecord,
    },
    /// Document already exists (create rejected).
    DocumentAlreadyExists {
        kind: SharedDocumentKind,
        current: SharedRecord,
    },
    /// Document updated successfully.
    DocumentUpdated {
        kind: SharedDocumentKind,
        record: SharedRecord,
    },
    /// Update rejected due to stale revision.
    DocumentStale {
        kind: SharedDocumentKind,
        current: SharedRecord,
    },
    /// Notification from another same-user client after a committed write.
    DocumentNotification {
        kind: SharedDocumentKind,
        record: SharedRecord,
    },
    /// Generic error.
    Error { reason: String },
}
