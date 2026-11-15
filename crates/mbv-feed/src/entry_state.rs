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

use crate::FeedError;
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
#[must_use]
pub fn feed_entry_state_path() -> PathBuf {
    mbv_config::state_dir().join("feed_entry_state.json")
}

impl FeedEntryStore {
    /// Read the state file. A missing, unreadable, or invalid file yields an
    /// empty store plus a log line: feed browsing and playback never depend on
    /// this succeeding.
    ///
    /// Rows written before this rule can hold a played row with a position.
    /// Those rows are corrected to position zero (spec `feed-queue-item`,
    /// #1005) and written back once.
    #[must_use]
    pub fn load() -> Self {
        let path = feed_entry_state_path();
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        match serde_json::from_str::<Self>(&text) {
            Ok(mut store) => {
                store.correct_played_rows();
                store
            }
            Err(error) => {
                tracing::warn!(
                    name: "feed.state.restore_failed",
                    target: "feed_state",
                    { file.path = %path.display(), error = %error },
                    "feed entry state not restored"
                );
                Self::default()
            }
        }
    }

    /// Apply the store's rule to the rows it read: a played row never keeps a
    /// position (spec `feed-queue-item`, #1005). The corrected rows are saved
    /// once; a failed save is logged and the corrected rows stay in memory.
    fn correct_played_rows(&mut self) {
        let mut corrected = 0usize;
        for row in &mut self.rows {
            if row.played && row.position_ticks > 0 {
                row.position_ticks = 0;
                corrected += 1;
            }
        }
        if corrected == 0 {
            return;
        }
        tracing::info!(name: "feed_state.rows.corrected", target: "feed_state", row_count = corrected, "played feed entry rows reset to position zero");
        if let Err(error) = self.save() {
            tracing::warn!(name: "feed_state.rows.correction_save_failed", target: "feed_state", error = %error, row_count = corrected, "corrected feed entry rows not saved");
        }
    }

    /// Atomically replace the state file (temp file plus rename). A failed write
    /// leaves the previously written file intact.
    pub fn save(&self) -> Result<(), FeedError> {
        let path = feed_entry_state_path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)
                .map_err(|e| FeedError::create_directory(dir.display().to_string(), e))?;
        }
        let json = serde_json::to_string(self).map_err(FeedError::serialize)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, &json).map_err(|e| FeedError::write(tmp.display().to_string(), e))?;
        std::fs::rename(&tmp, &path).map_err(|e| {
            FeedError::rename(tmp.display().to_string(), path.display().to_string(), e)
        })
    }

    /// Insert or replace the row for one key. Last write wins; no revision is
    /// consulted and no other row is touched. A played row stores position zero,
    /// as playback completion does (spec `feed-queue-item`, #1005).
    pub fn put(&mut self, user_id: &str, feed_id: &str, entry_guid: &str, state: FeedEntryState) {
        let row = FeedEntryRow {
            user_id: user_id.to_string(),
            feed_id: feed_id.to_string(),
            entry_guid: entry_guid.to_string(),
            position_ticks: if state.played {
                0
            } else {
                state.position_ticks
            },
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

    /// Force the played flag for one entry. Mark Played stores position zero,
    /// as playback completion does; Mark Unplayed keeps the stored position
    /// (spec `feed-queue-item`, #1005). A missing row is created with a zero
    /// resume position.
    pub fn set_played(&mut self, user_id: &str, feed_id: &str, entry_guid: &str, played: bool) {
        let position_ticks = self
            .get(user_id, feed_id, entry_guid)
            .map_or(0, |state| state.position_ticks);
        self.put(
            user_id,
            feed_id,
            entry_guid,
            FeedEntryState {
                position_ticks,
                played,
            },
        );
    }

    /// The stored state for one key, if that entry has any.
    #[must_use]
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
    #[must_use]
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
    #[must_use]
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Whether no rows are stored.
    #[must_use]
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
        let _guard = mbv_config::TestStateDirGuard::new();
        let mut store = FeedEntryStore::default();
        store.put(
            "user-1",
            "https://example.test/feed",
            "guid-1",
            state(120, false),
        );
        store.set_played("user-1", "https://example.test/feed", "guid-2", true);
        store.save().unwrap();

        let loaded = FeedEntryStore::load();
        assert_eq!(
            loaded.get("user-1", "https://example.test/feed", "guid-1"),
            Some(state(120, false))
        );
        assert_eq!(
            loaded.get("user-1", "https://example.test/feed", "guid-2"),
            Some(state(0, true))
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
            Some(state(0, true))
        );
    }

    /// Mark Played stores position zero; Mark Unplayed keeps the position
    /// (spec `feed-queue-item`, #1005).
    #[test]
    fn set_played_stores_zero_when_played_and_keeps_the_position_when_unplayed() {
        let mut store = FeedEntryStore::default();
        store.put("user", "feed", "entry", state(900, false));
        store.set_played("user", "feed", "entry", true);
        assert_eq!(store.get("user", "feed", "entry"), Some(state(0, true)));

        store.put("user", "feed", "entry", state(900, false));
        store.set_played("user", "feed", "entry", false);
        assert_eq!(store.get("user", "feed", "entry"), Some(state(900, false)));

        // A row that never had playback history is created at zero.
        store.set_played("user", "feed", "other", true);
        assert_eq!(store.get("user", "feed", "other"), Some(state(0, true)));
    }

    #[test]
    fn scan_returns_only_that_users_feed() {
        let _guard = mbv_config::TestStateDirGuard::new();
        let mut store = FeedEntryStore::default();
        store.put("user-1", "feed-1", "guid-1", state(10, false));
        store.put("user-1", "feed-1", "guid-2", state(20, false));
        store.put("user-1", "feed-2", "guid-3", state(30, false));
        store.put("user-2", "feed-1", "guid-1", state(40, false));
        store.save().unwrap();

        let mut scanned = store.scan("user-1", "feed-1");
        scanned.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(
            scanned,
            vec![
                ("guid-1".to_string(), state(10, false)),
                ("guid-2".to_string(), state(20, false)),
            ]
        );

        // The same scan after a reload, so this is the file's behavior too.
        let reloaded = FeedEntryStore::load();
        assert_eq!(reloaded.scan("user-1", "feed-1").len(), 2);
        assert_eq!(
            reloaded.scan("user-2", "feed-1"),
            vec![("guid-1".to_string(), state(40, false))]
        );
    }

    /// Write the store file's raw JSON, including rows an older build wrote.
    fn write_state_file(json: &str) {
        std::fs::write(feed_entry_state_path(), json).unwrap();
    }

    fn read_state_file() -> FeedEntryStore {
        serde_json::from_str(&std::fs::read_to_string(feed_entry_state_path()).unwrap()).unwrap()
    }

    /// A stored played row with a position loads corrected, and the file is
    /// rewritten with the corrected row (spec `feed-queue-item`, #1005).
    #[test]
    fn load_corrects_an_old_played_row_and_rewrites_the_file() {
        let _guard = mbv_config::TestStateDirGuard::new();
        write_state_file(
            r#"{"rows":[{"user_id":"user-1","feed_id":"feed-1","entry_guid":"guid-1","position_ticks":4000,"played":true},{"user_id":"user-1","feed_id":"feed-1","entry_guid":"guid-2","position_ticks":900,"played":false}]}"#,
        );

        let loaded = FeedEntryStore::load();
        assert_eq!(
            loaded.get("user-1", "feed-1", "guid-1"),
            Some(state(0, true))
        );
        assert_eq!(
            loaded.get("user-1", "feed-1", "guid-2"),
            Some(state(900, false))
        );

        let on_disk = read_state_file();
        assert_eq!(
            on_disk.get("user-1", "feed-1", "guid-1"),
            Some(state(0, true))
        );
        assert_eq!(
            on_disk.get("user-1", "feed-1", "guid-2"),
            Some(state(900, false))
        );
    }

    /// A file with nothing to correct is not written (spec `feed-queue-item`,
    /// #1005).
    #[test]
    fn load_does_not_rewrite_a_clean_file() {
        let _guard = mbv_config::TestStateDirGuard::new();
        let clean = "{\n  \"rows\": [\n    {\"user_id\": \"user-1\", \"feed_id\": \"feed-1\", \"entry_guid\": \"guid-1\", \"position_ticks\": 0, \"played\": true}\n  ]\n}";
        write_state_file(clean);

        let loaded = FeedEntryStore::load();
        assert_eq!(
            loaded.get("user-1", "feed-1", "guid-1"),
            Some(state(0, true))
        );
        assert_eq!(
            std::fs::read_to_string(feed_entry_state_path()).unwrap(),
            clean
        );
    }

    #[test]
    fn unreadable_state_loads_empty() {
        let _guard = mbv_config::TestStateDirGuard::new();

        // Absent file.
        assert!(FeedEntryStore::load().is_empty());

        // Present but invalid.
        std::fs::write(feed_entry_state_path(), "{\"rows\": not json").unwrap();
        assert!(FeedEntryStore::load().is_empty());
    }

    #[test]
    fn failed_save_preserves_the_previous_file() {
        let _guard = mbv_config::TestStateDirGuard::new();
        let mut original = FeedEntryStore::default();
        original.put("user-1", "feed-1", "guid-1", state(120, false));
        original.save().unwrap();

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
