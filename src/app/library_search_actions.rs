use super::library_browse_actions::{
    build_album_index_with, fetch_all_album_index_items, recursive_album_search_eligible,
};
use super::types_browse::BrowseResting;
use super::{
    AlbumIndexState, AlbumSearchEntry, App, BrowseLevel, LibEvent, SidebarId, PAGE_SIZE,
    PREFETCH_AHEAD,
};
impl App {
    /// Open the global search sidebar. Sets the flag; the shell Model mounts
    /// the `SearchSidebarComponent` when it syncs after this call (task 3.2).
    pub(super) fn open_search_sidebar(&mut self) {
        self.request_sidebar_open(SidebarId::Search);
    }

    pub(super) fn recursive_album_search_enabled(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            recursive_album_search_eligible(&lib.library.collection_type, &self.music_levels)
        })
    }

    pub(super) fn library_tabs_for_nav(&self) -> Vec<(usize, String, String)> {
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

    pub(super) fn start_album_index(&mut self, lib_idx: usize, refresh: bool) {
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

    // Visibility bump: private -> `pub(super)`. Called from
    // `handle_lib_event`'s `AlbumIndexBuilt` rebuild-pending branch, which
    // stays behind in `actions.rs`.
    pub(super) fn spawn_album_index_build(&self, library_id: String) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let levels = self.music_levels.clone();
        let tx = self.lib_tx.clone();
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

    pub(super) fn activate_recursive_album(
        &mut self,
        lib_idx: usize,
        entry: AlbumSearchEntry,
    ) -> bool {
        let library_id = self.libs[lib_idx].library.id.clone();
        let library_name = self.libs[lib_idx].library.display_name();
        let Some(client) = self.emby_snapshot() else {
            return false;
        };
        let tx = self.lib_tx.clone();
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
                let Some(cursor) = items.iter().position(|item| item.id == target_id) else {
                    let _ = tx.send(LibEvent::Error(format!(
                        "Album path changed before activation: missing {target_id}"
                    )));
                    return;
                };
                nav_stack.push(BrowseLevel {
                    parent_id,
                    title,
                    items,
                    total_count,
                    resting: BrowseResting::new(cursor, 0),
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,

                    all_items: None,
                    letter_filter: None,
                    music_grouping: None,
                });
            }
            let _ = tx.send(LibEvent::RecursiveAlbumActivated {
                library_id,
                nav_stack,
            });
        });
        true
    }

    // Visibility bump: private -> `pub(super)`. Called from
    // `select_letter_pill` (moved to `music_actions.rs`) plus
    // `refresh_lib`/`maybe_capture_library_total_and_apply_default_pill`,
    // which stay behind in `actions.rs`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_refresh(
        &self,
        lib_idx: usize,
        parent_id: String,
        item_types: Option<String>,
        unplayed_only: bool,
        sort_by: String,
        sort_order: String,
        loaded_count: usize,
        letter_filter: Option<super::render::LetterFilter>,
    ) {
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let limit = loaded_count.max(PAGE_SIZE);
        let (name_ge, name_lt) = letter_filter
            .as_ref()
            .map(|f| (f.name_ge, f.name_lt))
            .unwrap_or((None, None));
        std::thread::spawn(move || {
            match client.get_items_sorted_ranged(
                &parent_id,
                item_types.as_deref(),
                unplayed_only,
                0,
                limit,
                &sort_by,
                &sort_order,
                name_ge,
                name_lt,
            ) {
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

    /// Check whether another page should be fetched for the level at the top
    /// of `lib_idx`'s nav stack, and spawn it. `cursor` is the resolved
    /// position to threshold against (the caller's live/resting cursor) —
    /// never re-read from the level, so the prefetch decision no longer
    /// depends on `BrowseLevel.cursor` (task 4.3, R7).
    pub(in crate::app) fn maybe_fetch_next_page(&mut self, lib_idx: usize, cursor: usize) {
        let lib = &self.libs[lib_idx];
        let lvl = match lib.nav_stack.last() {
            Some(l) => l,
            None => return,
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
        if !is_feed_home_video_root && cursor + PREFETCH_AHEAD < lvl.items.len() {
            return;
        }
        let start_index = lvl.items.len();
        let parent_id = lvl.parent_id.clone();
        let item_types = lvl.item_types.clone();
        let unplayed_only = lvl.unplayed_only;
        let sort_by = lvl.sort_by.clone();
        let sort_order = lvl.sort_order.clone();
        let letter_filter = lvl.letter_filter.clone();
        if let Some(last) = self.libs[lib_idx].nav_stack.last_mut() {
            last.loading = true;
        }
        self.spawn_browse_page(
            lib_idx,
            parent_id,
            start_index,
            item_types,
            unplayed_only,
            sort_by,
            sort_order,
            letter_filter,
        );
    }
}
