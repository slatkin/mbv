use redb::Database;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

use crate::shared_state::{SharedDocumentKind, SharedDocumentTuple, SharedRecord};
use crate::shared_store::FeedEntryState;

/// Requests sent to the storage worker.
pub enum SharedStoreRequest {
    /// Read a single document.
    Read {
        user_id: String,
        kind: SharedDocumentKind,
        reply: mpsc::Sender<Result<Option<SharedRecord>, String>>,
    },
    /// Read all four documents for a user (snapshot).
    ReadAll {
        user_id: String,
        reply: mpsc::Sender<Result<SharedDocumentTuple, String>>,
    },
    /// Create-if-absent.
    Create {
        user_id: String,
        kind: SharedDocumentKind,
        value: serde_json::Value,
        reply: mpsc::Sender<Result<SharedRecord, String>>,
    },
    /// Update with expected revision (CAS).
    Update {
        user_id: String,
        kind: SharedDocumentKind,
        expected_revision: u64,
        value: serde_json::Value,
        reply: mpsc::Sender<Result<SharedRecord, String>>,
    },
    GetFeedEntry {
        user_id: String,
        feed_id: String,
        entry_guid: String,
        reply: mpsc::Sender<Result<Option<FeedEntryState>, String>>,
    },
    PutFeedEntry {
        user_id: String,
        feed_id: String,
        entry_guid: String,
        value: FeedEntryState,
        reply: mpsc::Sender<Result<(), String>>,
    },
    ScanFeedEntries {
        user_id: String,
        feed_id: String,
        reply: mpsc::Sender<Result<Vec<(String, FeedEntryState)>, String>>,
    },
    /// Administrative export: all documents as JSON.
    Export {
        reply: mpsc::Sender<Result<serde_json::Value, String>>,
    },
}

/// Handle to the storage worker. Callers send requests through the channel
/// and await bounded replies.
#[derive(Clone)]
pub struct SharedStoreHandle {
    tx: mpsc::Sender<SharedStoreRequest>,
}

impl SharedStoreHandle {
    pub fn read(
        &self,
        user_id: &str,
        kind: SharedDocumentKind,
    ) -> Result<Option<SharedRecord>, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::Read {
                user_id: user_id.to_string(),
                kind,
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn read_all(&self, user_id: &str) -> Result<SharedDocumentTuple, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::ReadAll {
                user_id: user_id.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn create(
        &self,
        user_id: &str,
        kind: SharedDocumentKind,
        value: serde_json::Value,
    ) -> Result<SharedRecord, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::Create {
                user_id: user_id.to_string(),
                kind,
                value,
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn update(
        &self,
        user_id: &str,
        kind: SharedDocumentKind,
        expected_revision: u64,
        value: serde_json::Value,
    ) -> Result<SharedRecord, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::Update {
                user_id: user_id.to_string(),
                kind,
                expected_revision,
                value,
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn export(&self) -> Result<serde_json::Value, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::Export { reply: reply_tx })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn get_feed_entry(
        &self,
        user_id: &str,
        feed_id: &str,
        entry_guid: &str,
    ) -> Result<Option<FeedEntryState>, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::GetFeedEntry {
                user_id: user_id.to_string(),
                feed_id: feed_id.to_string(),
                entry_guid: entry_guid.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn put_feed_entry(
        &self,
        user_id: &str,
        feed_id: &str,
        entry_guid: &str,
        value: FeedEntryState,
    ) -> Result<(), String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::PutFeedEntry {
                user_id: user_id.to_string(),
                feed_id: feed_id.to_string(),
                entry_guid: entry_guid.to_string(),
                value,
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }

    pub fn scan_feed_entries(
        &self,
        user_id: &str,
        feed_id: &str,
    ) -> Result<Vec<(String, FeedEntryState)>, String> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.tx
            .send(SharedStoreRequest::ScanFeedEntries {
                user_id: user_id.to_string(),
                feed_id: feed_id.to_string(),
                reply: reply_tx,
            })
            .map_err(|_| "storage worker shut down".to_string())?;
        reply_rx
            .recv()
            .map_err(|_| "storage worker reply channel closed".to_string())?
    }
}

/// Spawn the storage worker. It processes one request at a time through the
/// bounded channel, never holding playback, Player, queue, connection-registry,
/// or socket locks.
pub fn spawn_shared_store_worker(db: Arc<Mutex<Database>>) -> SharedStoreHandle {
    let (tx, rx) = mpsc::channel::<SharedStoreRequest>();

    std::thread::spawn(move || {
        while let Ok(req) = rx.recv() {
            let db = db.lock().unwrap();
            match req {
                SharedStoreRequest::Read {
                    user_id,
                    kind,
                    reply,
                } => {
                    let result = crate::shared_store::read_document(&db, &user_id, kind);
                    if let Err(error) = &result {
                        log::warn!(target: "shared_data", "read document failed: {error}");
                    }
                    let _ = reply.send(result);
                }
                SharedStoreRequest::ReadAll { user_id, reply } => {
                    let result = crate::shared_store::read_all_documents(&db, &user_id);
                    if let Err(error) = &result {
                        log::warn!(target: "shared_data", "read all documents failed: {error}");
                    }
                    let _ = reply.send(result);
                }
                SharedStoreRequest::Create {
                    user_id,
                    kind,
                    value,
                    reply,
                } => {
                    let result = crate::shared_store::create_document(&db, &user_id, kind, value);
                    if let Err(error) = &result {
                        log::warn!(target: "shared_data", "create document failed: {error}");
                    }
                    let _ = reply.send(result);
                }
                SharedStoreRequest::Update {
                    user_id,
                    kind,
                    expected_revision,
                    value,
                    reply,
                } => {
                    let result = crate::shared_store::update_document(
                        &db,
                        &user_id,
                        kind,
                        expected_revision,
                        value,
                    );
                    if let Err(error) = &result {
                        log::warn!(target: "shared_data", "update document failed: {error}");
                    }
                    let _ = reply.send(result);
                }
                SharedStoreRequest::GetFeedEntry {
                    user_id,
                    feed_id,
                    entry_guid,
                    reply,
                } => {
                    let _ = reply.send(crate::shared_store::get_feed_entry(
                        &db,
                        &user_id,
                        &feed_id,
                        &entry_guid,
                    ));
                }
                SharedStoreRequest::PutFeedEntry {
                    user_id,
                    feed_id,
                    entry_guid,
                    value,
                    reply,
                } => {
                    let _ = reply.send(crate::shared_store::put_feed_entry(
                        &db,
                        &user_id,
                        &feed_id,
                        &entry_guid,
                        &value,
                    ));
                }
                SharedStoreRequest::ScanFeedEntries {
                    user_id,
                    feed_id,
                    reply,
                } => {
                    let _ = reply.send(crate::shared_store::scan_feed_entries(
                        &db, &user_id, &feed_id,
                    ));
                }
                SharedStoreRequest::Export { reply } => {
                    let result = export_json(&db);
                    let _ = reply.send(result);
                }
            }
        }
    });

    SharedStoreHandle { tx }
}

/// Build a local administrative JSON export of all committed documents.
/// Contains user IDs, document kinds, revisions, and parsed values.
/// Contains no authentication tokens.
pub fn export_json(db: &Database) -> Result<serde_json::Value, String> {
    let user_ids = crate::shared_store::list_user_ids(db)?;
    let mut users = serde_json::Map::new();

    for user_id in &user_ids {
        let docs = crate::shared_store::list_documents(db, user_id)?;
        let mut doc_map = serde_json::Map::new();
        for (kind, record) in docs {
            doc_map.insert(
                kind.as_str().to_string(),
                serde_json::json!({
                    "revision": record.revision,
                    "value": record.value,
                }),
            );
        }
        users.insert(user_id.clone(), serde_json::Value::Object(doc_map));
    }

    Ok(serde_json::Value::Object(users))
}

pub fn export_json_pretty(db: &Database) -> Result<String, String> {
    serde_json::to_string_pretty(&export_json(db)?).map_err(|e| format!("serialize export: {e}"))
}
