//! Local, machine-scoped feed-entry playback state.
//!
//! One row per `(user_id, feed_id, entry_guid)` holding resume position and
//! watched flag, with last-write-wins writes and no revisions. The whole set is
//! held in memory and persisted as one JSON document at
//! `state_dir()/feed_entry_state.json`.
//!
//! This replaces the daemon-hosted feed-entry table of the shared-data
//! capability (`openspec/changes/remove-shared-central-storage`): same keying,
//! same last-write-wins semantics, same prefix scan, but no transport and no
//! cross-machine roaming.
//!
//! The interactive client is the only writer, and it is single-instance
//! (ADR 0006), so the file needs no locking and no compare-and-swap. Writes
//! rewrite the whole document; rows are bounded by the entries a user has
//! actually played, and writes happen on playback lifecycle events rather than
//! per frame.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Resume position and watched flag for one feed entry.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct FeedEntryState {
    pub position_ticks: i64,
    pub played: bool,
}

/// One stored row: the three-part key plus its state.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct FeedEntryRow {
    user_id: String,
    feed_id: String,
    entry_guid: String,
    position_ticks: i64,
    played: bool,
}

/// Every feed-entry row stored on this machine.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub struct FeedEntryStore {
    #[serde(default)]
    rows: Vec<FeedEntryRow>,
}

/// Path to the local feed-entry state file, alongside the other state files
/// under `state_dir()`.
pub fn feed_entry_state_path() -> PathBuf {
    crate::config::state_dir().join("feed_entry_state.json")
}

impl FeedEntryStore {
    /// Read the state file. A missing, unreadable, or invalid file yields an
    /// empty store plus a log line: feed browsing and playback never depend on
    /// this succeeding.
    pub fn load() -> Self {
        let path = feed_entry_state_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str(&text) {
            Ok(store) => store,
            Err(error) => {
                log::warn!(
                    target: "feed_state",
                    "{} failed to parse, feed entry state not restored: {error}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Atomically replace the state file (temp file plus rename). A failed write
    /// leaves the previously written file intact.
    pub fn save(&self) -> Result<(), String> {
        let path = feed_entry_state_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("create directory {}: {e}", dir.display()))?;
        }
        let json =
            serde_json::to_string(self).map_err(|e| format!("serialize feed entry state: {e}"))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json).map_err(|e| format!("write {}: {e}", tmp.display()))?;
        std::fs::rename(&tmp, &path)
            .map_err(|e| format!("rename {} to {}: {e}", tmp.display(), path.display()))
    }

    /// Insert or replace the row for one key. Last write wins; no revision is
    /// consulted and no other row is touched.
    pub fn put(&mut self, user_id: &str, feed_id: &str, entry_guid: &str, state: FeedEntryState) {
        let row = FeedEntryRow {
            user_id: user_id.to_string(),
            feed_id: feed_id.to_string(),
            entry_guid: entry_guid.to_string(),
            position_ticks: state.position_ticks,
            played: state.played,
        };
        match self.rows.iter_mut().find(|existing| {
            existing.user_id == user_id
                && existing.feed_id == feed_id
                && existing.entry_guid == entry_guid
        }) {
            Some(existing) => *existing = row,
            None => self.rows.push(row),
        }
    }

    /// The stored state for one key, if that entry has any.
    pub fn get(&self, user_id: &str, feed_id: &str, entry_guid: &str) -> Option<FeedEntryState> {
        self.rows
            .iter()
            .find(|row| {
                row.user_id == user_id && row.feed_id == feed_id && row.entry_guid == entry_guid
            })
            .map(|row| FeedEntryState {
                position_ticks: row.position_ticks,
                played: row.played,
            })
    }

    /// Every row under `(user_id, feed_id)`, as `(entry_guid, state)`. Rows of
    /// other feeds and other users are never returned.
    pub fn scan(&self, user_id: &str, feed_id: &str) -> Vec<(String, FeedEntryState)> {
        self.rows
            .iter()
            .filter(|row| row.user_id == user_id && row.feed_id == feed_id)
            .map(|row| {
                (
                    row.entry_guid.clone(),
                    FeedEntryState {
                        position_ticks: row.position_ticks,
                        played: row.played,
                    },
                )
            })
            .collect()
    }

    /// Number of stored rows, for tests and diagnostics.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no rows are stored.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(position_ticks: i64, played: bool) -> FeedEntryState {
        FeedEntryState {
            position_ticks,
            played,
        }
    }

    #[test]
    fn round_trip_through_the_file() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut store = FeedEntryStore::default();
        store.put(
            "user-1",
            "https://example.test/feed",
            "guid-1",
            state(120, true),
        );
        assert!(store.save().is_ok());

        let loaded = FeedEntryStore::load();
        assert_eq!(
            loaded.get("user-1", "https://example.test/feed", "guid-1"),
            Some(state(120, true))
        );
        assert!(feed_entry_state_path().is_file());
    }

    #[test]
    fn put_replaces_the_row_for_one_key() {
        let mut store = FeedEntryStore::default();
        store.put("user-1", "feed-1", "guid-1", state(10, false));
        store.put("user-1", "feed-1", "guid-1", state(900, true));

        assert_eq!(store.len(), 1);
        assert_eq!(
            store.get("user-1", "feed-1", "guid-1"),
            Some(state(900, true))
        );
    }

    #[test]
    fn scan_returns_only_that_users_feed() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut store = FeedEntryStore::default();
        store.put("user-1", "feed-1", "guid-1", state(10, false));
        store.put("user-1", "feed-1", "guid-2", state(20, true));
        store.put("user-1", "feed-2", "guid-3", state(30, false));
        store.put("user-2", "feed-1", "guid-1", state(40, true));
        assert!(store.save().is_ok());

        let mut scanned = store.scan("user-1", "feed-1");
        scanned.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            scanned,
            vec![
                ("guid-1".to_string(), state(10, false)),
                ("guid-2".to_string(), state(20, true)),
            ]
        );

        // The same scan after a reload, so this is the file's behavior too.
        let reloaded = FeedEntryStore::load();
        assert_eq!(reloaded.scan("user-1", "feed-1").len(), 2);
        assert_eq!(
            reloaded.scan("user-2", "feed-1"),
            vec![("guid-1".to_string(), state(40, true))]
        );
    }

    #[test]
    fn unreadable_state_loads_empty() {
        let _guard = crate::config::TestStateDirGuard::new();

        // Absent file.
        assert!(FeedEntryStore::load().is_empty());

        // Present but invalid.
        std::fs::write(feed_entry_state_path(), "{\"rows\": not json").unwrap();
        assert!(FeedEntryStore::load().is_empty());
    }

    #[test]
    fn failed_save_preserves_the_previous_file() {
        let _guard = crate::config::TestStateDirGuard::new();
        let mut original = FeedEntryStore::default();
        original.put("user-1", "feed-1", "guid-1", state(120, false));
        assert!(original.save().is_ok());

        // Make the temp path a directory so the write fails.
        let tmp = feed_entry_state_path().with_extension("json.tmp");
        std::fs::create_dir(&tmp).unwrap();

        let mut modified = FeedEntryStore::default();
        modified.put("user-1", "feed-1", "guid-1", state(9999, true));
        assert!(modified.save().is_err());

        assert_eq!(
            FeedEntryStore::load().get("user-1", "feed-1", "guid-1"),
            Some(state(120, false))
        );
    }
}
