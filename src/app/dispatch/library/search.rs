use mbv_core::api::EmbyClient;

use crate::app::App;

impl App {
    pub(in crate::app) fn spawn_search_sidebar_query(&self, client: EmbyClient, query: String) {
        let tx = self.channels.search_tx.clone();
        std::thread::spawn(move || {
            let result = client.search_items(&query, 100);
            let _ = tx.send((query, result));
        });
    }
}

use super::browse::{
    build_album_index_with, fetch_all_album_index_items, recursive_album_search_eligible,
    retain_grouped_music_level_items,
};
use crate::app::state::types::browse::BrowseResting;
use crate::app::{
    AlbumIndexState, AlbumSearchEntry, BrowseLevel, LibEvent, SidebarId, PAGE_SIZE, PREFETCH_AHEAD,
};
impl App {
    /// Open the global search sidebar. Sets the flag; the shell Model mounts
    /// the `SearchSidebarComponent` when it syncs after this call (task 3.2).
    pub(in crate::app) fn open_search_sidebar(&mut self) {
        self.request_sidebar_open(SidebarId::Search);
    }

    pub(in crate::app) fn recursive_album_search_enabled(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            recursive_album_search_eligible(&lib.library.collection_type, &self.music_levels)
        })
    }

    pub(in crate::app) fn library_tabs_for_nav(&self) -> Vec<(usize, String, String)> {
        self.libs
            .iter()
            .enumerate()
            .map(|(idx, tab)| {
                (
                    idx,
                    tab.library.id.clone(),
                    tab.library.collection_type.clone(),
                )
            })
            .collect()
    }

    pub(in crate::app) fn start_album_index(&mut self, lib_idx: usize, refresh: bool) {
        if !self.recursive_album_search_enabled(lib_idx) {
            return;
        }
        let library_id = self.libs[lib_idx].library.id.clone();
        let should_spawn = match self.album_indexes.get_mut(&library_id) {
            None => {
                self.album_indexes.insert(
                    library_id.clone(),
                    AlbumIndexState::Loading {
                        rebuild_pending: false,
                    },
                );
                true
            }
            Some(AlbumIndexState::Loading { rebuild_pending }) if refresh => {
                *rebuild_pending = true;
                false
            }
            Some(state) if refresh => {
                *state = AlbumIndexState::Loading {
                    rebuild_pending: false,
                };
                true
            }
            Some(_) => false,
        };
        if should_spawn {
            self.spawn_album_index_build(library_id);
        }
    }

    // Visibility bump: private -> `pub(in crate::app)`. Called from
    // `handle_lib_event`'s `AlbumIndexBuilt` rebuild-pending branch, which
    // stays behind in `actions.rs`.
    pub(in crate::app) fn spawn_album_index_build(&self, library_id: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let levels = self.music_levels.clone();
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let mut fetch = |parent_id: &str, start: usize, limit: usize| {
                client.get_items_sorted(
                    parent_id,
                    None,
                    false,
                    start,
                    limit,
                    "SortName",
                    "Ascending",
                )
            };
            let result = build_album_index_with(&library_id, &levels, &mut fetch);
            let _ = tx.send(LibEvent::AlbumIndexBuilt { library_id, result });
        });
    }

    pub(in crate::app) fn activate_recursive_album(
        &mut self,
        lib_idx: usize,
        entry: AlbumSearchEntry,
    ) -> bool {
        // A catalog change between resolve and drain can leave `lib_idx`
        // stale; a miss flashes (the caller's false branch) instead of
        // panicking on the raw index (U2 correction, matching the sibling
        // Series/Chain arms' `libs.get` guards).
        let Some(lib) = self.libs.get(lib_idx) else {
            return false;
        };
        let library_id = lib.library.id.clone();
        let library_name = lib.library.display_name();
        let grouped_music = self.is_grouped_music_library(lib_idx);
        let Some(client) = self.emby_snapshot() else {
            return false;
        };
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let fetch = |parent_id: &str| {
                let mut call = |id: &str, start: usize, limit: usize| {
                    client.get_items_sorted(id, None, false, start, limit, "SortName", "Ascending")
                };
                fetch_all_album_index_items(parent_id, &mut call)
            };
            let mut parents = vec![(library_id.clone(), library_name)];
            parents.extend(
                entry
                    .ancestors
                    .iter()
                    .map(|part| (part.id.clone(), part.name.clone())),
            );
            let mut targets: Vec<String> =
                entry.ancestors.iter().map(|part| part.id.clone()).collect();
            targets.push(entry.album.id.clone());
            let mut nav_stack = Vec::new();
            for ((parent_id, title), target_id) in parents.into_iter().zip(targets) {
                let items = match fetch(&parent_id) {
                    Ok(items) => items,
                    Err(error) => {
                        let _ = tx.send(LibEvent::Error(error));
                        return;
                    }
                };
                let total_count = items.len();
                let mut level = BrowseLevel {
                    parent_id,
                    title,
                    fetched_rows: items.len(),
                    items,
                    total_count,
                    resting: BrowseResting::new(0, 0),
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,

                    all_items: None,
                    letter_filter: None,
                    tv_content_mode: None,
                    music_grouping: None,
                };
                // Grouped-state construction: apply the same boundary filter
                // `handle_lib_loaded` applies to every group-view level
                // before the prepared landing is published, so the built
                // stack is what the tree would have shown.
                retain_grouped_music_level_items(&mut level, grouped_music);
                let Some(cursor) = level.items.iter().position(|item| item.id == target_id) else {
                    let _ = tx.send(LibEvent::Error(format!(
                        "Album path changed before activation: missing {target_id}"
                    )));
                    return;
                };
                level.resting = BrowseResting::new(cursor, 0);
                nav_stack.push(level);
            }
            let _ = tx.send(LibEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            });
        });
        true
    }

    // Visibility bump: private -> `pub(in crate::app)`. Called from
    // `select_letter_pill` (moved to `music_actions.rs`) plus
    // `refresh_lib`/`maybe_capture_library_total_and_apply_default_pill`,
    // which stay behind in `actions.rs`.
    pub(in crate::app) fn spawn_refresh(
        &self,
        lib_idx: usize,
        loaded_count: usize,
        key: crate::app::state::types::browse::LevelFetchKey,
    ) {
        let crate::app::state::types::browse::LevelFetchKey {
            parent_id,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            letter_filter,
        } = key;
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.channels.lib_tx.clone();
        let limit = loaded_count.max(PAGE_SIZE);
        let (name_ge, name_lt) = letter_filter.map_or((None, None), |f| (f.name_ge, f.name_lt));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(&mbv_core::api::SortedItemsParams {
                parent_id: &parent_id,
                item_types: item_types.as_deref(),
                unplayed_only,
                start_index: 0,
                limit,
                sort_by: &sort_by,
                sort_order: &sort_order,
                name_ge,
                name_lt,
            }) {
                Ok((items, total_count)) => {
                    log::info!(target: "browse", "Refreshed lib_idx={lib_idx} parent={parent_id} total={total_count} got={} first3={:?}",
                        items.len(),
                        items.iter().take(3).map(|i| format!("{}:{}", i.id, i.name)).collect::<Vec<_>>());
                    let _ = tx.send(LibEvent::Refreshed {
                        lib_idx,
                        parent_id,
                        item_types,
                        unplayed_only,
                        items,
                        total_count,
                    });
                }
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                }
            }
        });
    }

    /// Arm the next source page when a focused Grouped Music artist has a
    /// child album near the loaded edge. The tree owns artist/album selection;
    /// the shell resolves these stable album targets back to its browse level
    /// here, so no component cursor or source-row index crosses the boundary.
    pub(in crate::app) fn maybe_fetch_next_page_for_music_artist(
        &mut self,
        lib_idx: usize,
        album_targets: &[String],
    ) {
        let cursor = {
            let Some(level) = self.libs.get(lib_idx).and_then(|lib| lib.nav_stack.last()) else {
                return;
            };
            album_targets
                .iter()
                .filter_map(|target| level.items.iter().position(|item| item.id == *target))
                .max()
        };
        if let Some(cursor) = cursor {
            self.maybe_fetch_next_page(lib_idx, cursor);
        }
    }

    /// Check whether another page should be fetched for the level at the top
    /// of `lib_idx`'s nav stack, and spawn it. `cursor` is the resolved
    /// position to threshold against (the caller's live/resting cursor) —
    /// never re-read from the level, so the prefetch decision no longer
    /// depends on `BrowseLevel.cursor` (task 4.3, R7).
    pub(in crate::app) fn maybe_fetch_next_page(&mut self, lib_idx: usize, cursor: usize) {
        self.maybe_fetch_next_page_sized(lib_idx, cursor, PREFETCH_AHEAD, PAGE_SIZE);
    }

    /// `maybe_fetch_next_page` with caller-chosen near-edge margin and page
    /// size. `cursor` is the resolved position to threshold against (the
    /// caller's live/resting cursor) — never re-read from the level, so the
    /// prefetch decision no longer depends on `BrowseLevel.cursor` (task 4.3,
    /// R7).
    pub(in crate::app) fn maybe_fetch_next_page_sized(
        &mut self,
        lib_idx: usize,
        cursor: usize,
        ahead: usize,
        limit: usize,
    ) {
        let lib = &self.libs[lib_idx];
        let Some(lvl) = lib.nav_stack.last() else {
            return;
        };
        if lvl.loading {
            return;
        }
        if lvl.is_fully_loaded() {
            return;
        }
        // The root folder listing of a feed-home-video library isn't scrolled by
        // the user directly -- it's aggregated in the background into grouped
        // sections, and that aggregation can't start until every page has
        // loaded. Waiting for the cursor to approach the loaded edge (as normal
        // browse levels do) would stall pagination forever for libraries with
        // more folders than PAGE_SIZE + PREFETCH_AHEAD, since nothing moves the
        // cursor on that hidden level. Paginate it to completion unconditionally.
        let is_feed_home_video_root =
            lib.nav_stack.len() == 1 && self.is_feed_home_video_library(lib_idx);
        // The Grouped Music album level groups its flat items by artist
        // client-side and the tree's local inline-search filter only ever
        // searches what's loaded (never re-fetches on its own), so this level
        // paginates to completion unconditionally too, exactly like the feed
        // home-video root above -- there's no cursor-proximity heuristic that
        // stays correct once artists collapse/expand independently of the
        // underlying flat array position.
        let paginate_to_completion = is_feed_home_video_root || self.is_music_group_view(lib_idx);
        if !paginate_to_completion && cursor + ahead < lvl.items.len() {
            return;
        }
        let start_index = lvl.fetched_rows;
        let key = crate::app::state::types::browse::LevelFetchKey::from_level(lvl);
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.loading = true;
        }
        self.spawn_browse_page_sized(lib_idx, start_index, limit, key);
    }
}
