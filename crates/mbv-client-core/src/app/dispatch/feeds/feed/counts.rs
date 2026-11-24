//! The feed-view per-folder count fan-out (design D1 (b) of change
//! `feed-view-lazy-folder-groups`): the background queries that fill the
//! pill-membership count map, their result event, and the quiescence
//! accounting a restored pill index resolves against.

use super::App;
use crate::app::state::events::{BrowseEvent, LibEvent};
use mbv_emby::EmbyClient;
use std::sync::mpsc;

/// How many threads the per-folder count fan-out may run at once. Each
/// thread owns one contiguous slice of the root folders and queries its
/// slice sequentially, so the burst of concurrent count requests never
/// exceeds this and a very large folder count degrades to slices of M/8
/// queries instead of M threads. Eight covers a home-video library's
/// typical folder count fully in parallel without a thread-pool crate for a
/// one-shot fan-out.
const FOLDER_COUNT_THREADS: usize = 8;

/// How many attempts one folder's count query gets before the fan-out
/// reports `None` for it: the initial query plus two retries for transient
/// HTTP failures, so a single failed request does not leave the folder
/// without its pill for the whole session (spec `feed-home-video-group-view`,
/// "The pill row SHALL include one pill for every folder directly under the
/// library root that contains at least one video"). The retries run
/// back-to-back with no delay: they exist to absorb one-off request
/// failures, not sustained outages — a folder whose every attempt fails
/// still reports exactly one `None` result, keeping the fan-out's
/// accounting balanced, and the next listing completion or refresh
/// re-queries it.
const FOLDER_COUNT_ATTEMPTS: usize = 3;

/// One folder's count query inside a fan-out thread: sends the folder's own
/// result event — every queried folder reports exactly once, `None` when
/// every attempt fails, so the fan-out's quiescence accounting balances —
/// carrying the count the pill row reads on success.
fn query_folder_count(
    client: &EmbyClient,
    tx: &mpsc::Sender<LibEvent>,
    lib_idx: usize,
    parent_id: &str,
    folder_id: &str,
) {
    let mut count = None;
    let mut failure = None;
    for _ in 0..FOLDER_COUNT_ATTEMPTS {
        match client.get_folder_video_count(folder_id) {
            Ok(queried) => {
                count = Some(queried);
                failure = None;
                break;
            }
            Err(error) => failure = Some(error),
        }
    }
    if count.is_none()
        && let Some(error) = failure
    {
        tracing::warn!(name: "feed.home_video.folder_count_failed", target: "feedhv", folder = %folder_id, attempts = FOLDER_COUNT_ATTEMPTS, error = %error, "folder video-count query failed");
    }
    let _ = tx.send(LibEvent::Browse(
        BrowseEvent::FeedHomeVideoFolderCountLoaded {
            lib_idx,
            parent_id: parent_id.to_string(),
            folder_id: folder_id.to_string(),
            count,
        },
    ));
}

impl App {
    /// The count fan-out (design D1 (b), task 3.3): background threads query
    /// the per-folder videos-only count for every folder directly under the
    /// current root listing, one query per folder, and each thread sends one
    /// result event per folder as its queries return so pills fill in
    /// incrementally. At most `FOLDER_COUNT_THREADS` threads run, each over a
    /// contiguous slice of the folder list. A query whose every attempt
    /// fails reports `None` — its folder gets no pill until a later listing
    /// completion or refresh re-queries it. The listing's `child_count` is
    /// not consulted (the root
    /// listing requests no count fields) and nothing here lists any videos.
    /// Callers decide when to fan out: a listing completion spawns once its
    /// listing becomes fully loaded; a refresh spawns unconditionally (design
    /// D2). Without a Service client it spawns nothing; the next listing
    /// completion or refresh retries.
    pub(in crate::app) fn spawn_feed_home_video_folder_counts(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            // No Service client yet: the next listing completion or refresh
            // retries.
            return;
        };
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let Some(root) = lib.nav_stack.first() else {
            return;
        };
        let folder_ids: Vec<String> = root
            .items
            .iter()
            .filter(|item| item.is_folder)
            .map(|item| item.id.clone())
            .collect();
        if folder_ids.is_empty() {
            return;
        }
        let parent_id = root.parent_id.clone();
        let tx = self.channels.lib_tx.clone();
        // Quiescence accounting: one owed report per queried folder, so the
        // per-folder arm knows when the counted row is final.
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.outstanding_counts += folder_ids.len();
        }
        let thread_count = folder_ids.len().clamp(1, FOLDER_COUNT_THREADS);
        let slice_len = folder_ids.len().div_ceil(thread_count);
        tracing::debug!(name: "feed.home_video.counts_spawned", target: "feedhv", library_index = lib_idx, folders = folder_ids.len(), threads = thread_count, "folder video-count fan-out spawned");
        for slice in folder_ids.chunks(slice_len).map(<[String]>::to_vec) {
            let client = client.clone();
            let tx = tx.clone();
            let parent_id = parent_id.clone();
            std::thread::spawn(move || {
                for folder_id in slice {
                    query_folder_count(&client, &tx, lib_idx, &parent_id, &folder_id);
                }
            });
        }
    }

    /// The `FeedHomeVideoFolderCountLoaded` arm (task 3.3). Stale guard: a
    /// result records into the pill-membership map only while its root parent
    /// still matches the current root listing's parent — a library switch or
    /// a replaced root drops it (its accounting report still settles the
    /// fan-out). Every queried folder reports exactly once, success or
    /// failure, so when the last owed report lands the fan-out has quiesced:
    /// the counted row is final and a pending restored pill index resolves
    /// against it — and is consumed even when it names no pill, so a partial
    /// arrival can never misbind it and a vanished folder's index is spent.
    /// The selection's own fetch stays with the shown-pass trigger (design
    /// D2); nothing here lists any videos.
    pub(in crate::app) fn handle_feed_home_video_folder_count_loaded(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        folder_id: &str,
        count: Option<usize>,
    ) {
        let became_quiesced = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
            .is_some_and(|state| {
                if state.outstanding_counts == 0 {
                    return false;
                }
                state.outstanding_counts -= 1;
                state.outstanding_counts == 0
            });
        let stale = !self.feed_home_video_root_matches(lib_idx, parent_id)
            || self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.feed_home_video.as_ref())
                .is_none();
        if !stale
            && let Some(count) = count
            && let Some(state) = self
                .libs
                .get_mut(lib_idx)
                .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.folder_counts.insert(folder_id.to_string(), count);
        }
        if became_quiesced && self.feed_home_video_root_listing_completed(lib_idx) {
            self.resolve_feed_home_video_pending_selection(lib_idx);
        }
        self.clamp_feed_home_video_state(lib_idx);
        if stale {
            self.log_feed_home_video_state(lib_idx, "folder_counts_dropped_stale");
        } else {
            self.log_feed_home_video_state(lib_idx, "folder_counts_loaded");
        }
    }

    /// Whether every folder directly under the root listing holds a count
    /// entry: the counted row is final and a restored pill index can resolve
    /// against it.
    pub(super) fn feed_home_video_folder_counts_cover_root(&self, lib_idx: usize) -> bool {
        let Some(lib) = self.libs.get(lib_idx) else {
            return false;
        };
        let Some(state) = lib.feed_home_video.as_ref() else {
            return false;
        };
        let Some(root) = lib.nav_stack.first() else {
            return false;
        };
        root.items
            .iter()
            .filter(|item| item.is_folder)
            .all(|folder| state.folder_counts.contains_key(&folder.id))
    }
}
