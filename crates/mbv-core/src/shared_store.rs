use redb::{Database, ReadableTable, TableDefinition};
use std::path::{Path, PathBuf};

use crate::shared_state::{SharedDocumentKind, SharedDocumentTuple, SharedRecord};

const DB_TABLE: TableDefinition<(&str, &str), &str> = TableDefinition::new("shared_documents");

/// Path to the shared-data `redb` database file. Lives alongside the other
/// state files under `state_dir()`.
pub fn shared_db_path() -> PathBuf {
    crate::config::state_dir().join("shared.mbvd")
}

/// Open or create the shared-data database with owner-only filesystem
/// permissions. Returns `Err` if the database cannot be opened safely (corrupt,
/// permissions, etc.) — callers should log the error and disable shared-data
/// hosting without touching playback.
pub fn open_shared_db() -> Result<Database, String> {
    let path = shared_db_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("create shared db directory {}: {e}", parent.display()))?;
    }
    // Only a database newly created by this call may be cleaned up on failure;
    // a pre-existing database file must never be deleted, even if the init
    // transaction cannot complete (e.g. a migration or commit failure).
    let existed = path.is_file();
    let db = Database::create(&path)
        .map_err(|e| format!("open shared database {}: {e}", path.display()))?;

    // Ensure the table exists.
    let txn = db.begin_write().map_err(|e| {
        remove_created_db(&path, existed);
        format!("begin shared db init transaction: {e}")
    })?;
    {
        let _ = txn.open_table(DB_TABLE);
    }
    txn.commit().map_err(|e| {
        remove_created_db(&path, existed);
        format!("commit shared db init transaction: {e}")
    })?;

    // Best-effort owner-only permissions on the database file and parent dir.
    set_owner_only_permissions(&path);
    if let Some(parent) = path.parent() {
        set_owner_only_permissions(parent);
    }

    Ok(db)
}

/// Open an existing shared database without creating one. Used by the local
/// administrative export command so disabled hosting remains side-effect free.
pub fn open_existing_shared_db() -> Result<Database, String> {
    let path = shared_db_path();
    if !path.is_file() {
        return Err(format!(
            "shared database does not exist: {}",
            path.display()
        ));
    }
    Database::open(&path).map_err(|e| format!("open shared database {}: {e}", path.display()))
}

/// Remove the database file only if `open_shared_db` created it during this
/// call (i.e. it did not exist before). A pre-existing database is left in
/// place so a failed initialization never deletes committed data.
fn remove_created_db(path: &Path, existed: bool) {
    if !existed {
        let _ = std::fs::remove_file(path);
    }
}

/// Best-effort `chmod 0o700` (directory) or `chmod 0o600` (file).
fn set_owner_only_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if path.is_dir() { 0o700 } else { 0o600 };
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode));
    }
    let _ = path; // suppress unused warning on non-unix
}

fn table_key(user_id: &str, kind: SharedDocumentKind) -> (String, String) {
    (user_id.to_string(), kind.as_str().to_string())
}

/// Read a single document. Returns `None` if absent.
pub fn read_document(
    db: &Database,
    user_id: &str,
    kind: SharedDocumentKind,
) -> Result<Option<SharedRecord>, String> {
    let txn = db
        .begin_read()
        .map_err(|e| format!("begin read transaction: {e}"))?;
    let table = txn
        .open_table(DB_TABLE)
        .map_err(|e| format!("open table for read: {e}"))?;
    let (uk, kk) = table_key(user_id, kind);
    let key: (&str, &str) = (&uk, &kk);
    match table.get(key).map_err(|e| format!("read document: {e}"))? {
        Some(guard) => {
            let raw = guard.value();
            let record: SharedRecord =
                serde_json::from_str(raw).map_err(|e| format!("parse shared record: {e}"))?;
            Ok(Some(record))
        }
        None => Ok(None),
    }
}

/// Read all four documents for a user in one read transaction.
pub fn read_all_documents(db: &Database, user_id: &str) -> Result<SharedDocumentTuple, String> {
    let txn = db
        .begin_read()
        .map_err(|e| format!("begin read transaction: {e}"))?;
    let table = txn
        .open_table(DB_TABLE)
        .map_err(|e| format!("open table for read: {e}"))?;

    let mut queue_state = None;
    let mut library_position_state = None;
    let mut last_remote_connection = None;
    let mut roaming_settings = None;

    for (kind, dest) in [
        (SharedDocumentKind::QueueState, &mut queue_state),
        (
            SharedDocumentKind::LibraryPositionState,
            &mut library_position_state,
        ),
        (
            SharedDocumentKind::LastRemoteConnection,
            &mut last_remote_connection,
        ),
        (SharedDocumentKind::RoamingSettings, &mut roaming_settings),
    ] {
        let (uk, kk) = table_key(user_id, kind);
        let key: (&str, &str) = (&uk, &kk);
        if let Some(guard) = table.get(key).map_err(|e| format!("read document: {e}"))? {
            let raw = guard.value();
            let record: SharedRecord =
                serde_json::from_str(raw).map_err(|e| format!("parse shared record: {e}"))?;
            *dest = Some(record);
        }
    }

    Ok((
        queue_state,
        library_position_state,
        last_remote_connection,
        roaming_settings,
    ))
}

/// Create-if-absent: the document must not already exist. Returns the
/// committed record (with revision 1) on success.
pub fn create_document(
    db: &Database,
    user_id: &str,
    kind: SharedDocumentKind,
    value: serde_json::Value,
) -> Result<SharedRecord, String> {
    // Phase 1: check existence in a read transaction.
    let existing = {
        let txn = db
            .begin_read()
            .map_err(|e| format!("begin read transaction: {e}"))?;
        let table = txn
            .open_table(DB_TABLE)
            .map_err(|e| format!("open table for read: {e}"))?;
        let (uk, kk) = table_key(user_id, kind);
        let key: (&str, &str) = (&uk, &kk);
        match table
            .get(key)
            .map_err(|e| format!("check existence: {e}"))?
        {
            Some(guard) => {
                let raw = guard.value().to_string();
                let record: SharedRecord = serde_json::from_str(&raw)
                    .map_err(|e| format!("parse existing record: {e}"))?;
                Some(record)
            }
            None => None,
        }
    };

    if let Some(record) = existing {
        return Err(format!(
            "already_exists:{}",
            serde_json::to_string(&record).unwrap_or_default()
        ));
    }

    // Phase 2: insert the new record.
    let record = SharedRecord { revision: 1, value };
    let json = serde_json::to_string(&record).map_err(|e| format!("serialize record: {e}"))?;
    let txn = db
        .begin_write()
        .map_err(|e| format!("begin write transaction: {e}"))?;
    let mut table = txn
        .open_table(DB_TABLE)
        .map_err(|e| format!("open table for write: {e}"))?;
    let (uk, kk) = table_key(user_id, kind);
    let key: (&str, &str) = (&uk, &kk);
    table
        .insert(key, json.as_str())
        .map_err(|e| format!("insert record: {e}"))?;
    drop(table);
    txn.commit().map_err(|e| format!("commit create: {e}"))?;
    Ok(record)
}

/// Update with expected revision (CAS). Returns `Ok(record)` on success,
/// or `Err("stale:<current_json>")` if the expected revision is stale.
pub fn update_document(
    db: &Database,
    user_id: &str,
    kind: SharedDocumentKind,
    expected_revision: u64,
    value: serde_json::Value,
) -> Result<SharedRecord, String> {
    // Phase 1: read current record and check CAS in a read transaction.
    let current_json = {
        let txn = db
            .begin_read()
            .map_err(|e| format!("begin read transaction: {e}"))?;
        let table = txn
            .open_table(DB_TABLE)
            .map_err(|e| format!("open table for read: {e}"))?;
        let (uk, kk) = table_key(user_id, kind);
        let key: (&str, &str) = (&uk, &kk);
        match table.get(key).map_err(|e| format!("read for CAS: {e}"))? {
            Some(guard) => {
                let raw = guard.value().to_string();
                let record: SharedRecord =
                    serde_json::from_str(&raw).map_err(|e| format!("parse current record: {e}"))?;
                if record.revision != expected_revision {
                    return Err(format!("stale:{}", raw));
                }
                Some(raw)
            }
            None => {
                if expected_revision != 0 {
                    return Err("stale:document absent but expected non-zero revision".to_string());
                }
                None
            }
        }
    };

    // Phase 2: write the new record.
    let new_revision = match &current_json {
        Some(raw) => {
            let current: SharedRecord =
                serde_json::from_str(raw).map_err(|e| format!("parse current for rev: {e}"))?;
            current.revision + 1
        }
        None => 1,
    };

    let record = SharedRecord {
        revision: new_revision,
        value,
    };
    let json = serde_json::to_string(&record).map_err(|e| format!("serialize record: {e}"))?;
    let txn = db
        .begin_write()
        .map_err(|e| format!("begin write transaction: {e}"))?;
    let mut table = txn
        .open_table(DB_TABLE)
        .map_err(|e| format!("open table for write: {e}"))?;
    let (uk, kk) = table_key(user_id, kind);
    let key: (&str, &str) = (&uk, &kk);
    table
        .insert(key, json.as_str())
        .map_err(|e| format!("insert record: {e}"))?;
    drop(table);
    txn.commit().map_err(|e| format!("commit update: {e}"))?;
    Ok(record)
}

/// List all user IDs present in the database (for admin export).
pub fn list_user_ids(db: &Database) -> Result<Vec<String>, String> {
    let txn = db
        .begin_read()
        .map_err(|e| format!("begin read transaction: {e}"))?;
    let table = txn
        .open_table(DB_TABLE)
        .map_err(|e| format!("open table for list: {e}"))?;
    let mut user_ids = std::collections::HashSet::new();
    let iter = table.iter().map_err(|e| format!("iterate table: {e}"))?;
    for entry in iter {
        let (key, _) = entry.map_err(|e| format!("read entry: {e}"))?;
        let (user_id, _kind) = key.value();
        user_ids.insert(user_id.to_string());
    }
    Ok(user_ids.into_iter().collect())
}

/// List all document kinds for a user (for admin export).
pub fn list_documents(
    db: &Database,
    user_id: &str,
) -> Result<Vec<(SharedDocumentKind, SharedRecord)>, String> {
    let txn = db
        .begin_read()
        .map_err(|e| format!("begin read transaction: {e}"))?;
    let table = txn
        .open_table(DB_TABLE)
        .map_err(|e| format!("open table for list: {e}"))?;
    let mut docs = Vec::new();
    let iter = table.iter().map_err(|e| format!("iterate table: {e}"))?;
    for entry in iter {
        let (key, value) = entry.map_err(|e| format!("read entry: {e}"))?;
        let (uid, kind_str) = key.value();
        if uid != user_id {
            continue;
        }
        let kind = match kind_str {
            "queue_state" => SharedDocumentKind::QueueState,
            "library_position_state" => SharedDocumentKind::LibraryPositionState,
            "last_remote_connection" => SharedDocumentKind::LastRemoteConnection,
            "roaming_settings" => SharedDocumentKind::RoamingSettings,
            _ => continue,
        };
        let record: SharedRecord =
            serde_json::from_str(value.value()).map_err(|e| format!("parse record: {e}"))?;
        docs.push((kind, record));
    }
    Ok(docs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_db() -> Database {
        let dir = std::env::temp_dir().join(format!("mbv-shared-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.mbvd");
        let db = Database::create(&path).unwrap();
        let txn = db.begin_write().unwrap();
        let _ = txn.open_table(DB_TABLE);
        txn.commit().unwrap();
        db
    }

    #[test]
    fn create_and_read() {
        let db = test_db();
        let val = serde_json::json!({"items": [], "cursor": 0});
        let rec =
            create_document(&db, "user1", SharedDocumentKind::QueueState, val.clone()).unwrap();
        assert_eq!(rec.revision, 1);
        let loaded = read_document(&db, "user1", SharedDocumentKind::QueueState)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.revision, 1);
        assert_eq!(loaded.value, val);
    }

    #[test]
    fn create_already_exists() {
        let db = test_db();
        let val = serde_json::json!({});
        create_document(&db, "u", SharedDocumentKind::QueueState, val.clone()).unwrap();
        let result = create_document(&db, "u", SharedDocumentKind::QueueState, val);
        assert!(result.is_err());
        assert!(result.unwrap_err().starts_with("already_exists:"));
    }

    #[test]
    fn update_cas_success() {
        let db = test_db();
        let val = serde_json::json!({"a": 1});
        let rec = create_document(&db, "u", SharedDocumentKind::QueueState, val).unwrap();
        assert_eq!(rec.revision, 1);
        let val2 = serde_json::json!({"a": 2});
        let rec2 =
            update_document(&db, "u", SharedDocumentKind::QueueState, 1, val2.clone()).unwrap();
        assert_eq!(rec2.revision, 2);
        assert_eq!(rec2.value, val2);
    }

    #[test]
    fn update_stale_rejection() {
        let db = test_db();
        let val = serde_json::json!({});
        create_document(&db, "u", SharedDocumentKind::QueueState, val).unwrap();
        let result = update_document(
            &db,
            "u",
            SharedDocumentKind::QueueState,
            0,
            serde_json::json!({"x": 1}),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().starts_with("stale:"));
    }

    #[test]
    fn read_absent() {
        let db = test_db();
        let result = read_document(&db, "nobody", SharedDocumentKind::QueueState).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn user_isolation() {
        let db = test_db();
        let val = serde_json::json!({"user": "a"});
        create_document(&db, "user_a", SharedDocumentKind::QueueState, val).unwrap();
        let val_b = serde_json::json!({"user": "b"});
        create_document(&db, "user_b", SharedDocumentKind::QueueState, val_b.clone()).unwrap();
        let rec_a = read_document(&db, "user_a", SharedDocumentKind::QueueState)
            .unwrap()
            .unwrap();
        let rec_b = read_document(&db, "user_b", SharedDocumentKind::QueueState)
            .unwrap()
            .unwrap();
        assert_eq!(rec_a.value["user"], "a");
        assert_eq!(rec_b.value, val_b);
    }

    #[test]
    fn failed_commit_preserves_data() {
        let db = test_db();
        let val = serde_json::json!({"keep": true});
        let rec = create_document(&db, "u", SharedDocumentKind::QueueState, val.clone()).unwrap();
        assert_eq!(rec.revision, 1);

        // Stale update should not affect the stored record.
        let _ = update_document(
            &db,
            "u",
            SharedDocumentKind::QueueState,
            0,
            serde_json::json!({"overwrite": true}),
        );
        let loaded = read_document(&db, "u", SharedDocumentKind::QueueState)
            .unwrap()
            .unwrap();
        assert_eq!(loaded.revision, 1);
        assert_eq!(loaded.value, val);
    }

    #[test]
    fn read_all_documents_populated() {
        let db = test_db();
        let val_q = serde_json::json!({"queue": true});
        let val_l = serde_json::json!({"lib": true});
        create_document(&db, "u", SharedDocumentKind::QueueState, val_q.clone()).unwrap();
        create_document(
            &db,
            "u",
            SharedDocumentKind::LibraryPositionState,
            val_l.clone(),
        )
        .unwrap();
        let (q, l, r, s) = read_all_documents(&db, "u").unwrap();
        assert_eq!(q.unwrap().value, val_q);
        assert_eq!(l.unwrap().value, val_l);
        assert!(r.is_none());
        assert!(s.is_none());
    }

    #[test]
    fn read_all_documents_empty() {
        let db = test_db();
        let (q, l, r, s) = read_all_documents(&db, "nobody").unwrap();
        assert!(q.is_none());
        assert!(l.is_none());
        assert!(r.is_none());
        assert!(s.is_none());
    }
}
