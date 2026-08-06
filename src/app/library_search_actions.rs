use super::library_browse_actions::{build_album_index_with, recursive_album_search_eligible};
use super::search_modal::{SearchModal, SearchMode};
use super::{AlbumIndexState, App, LibEvent, PAGE_SIZE, PREFETCH_AHEAD};
impl App {
    pub(super) fn recursive_album_search_enabled(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            recursive_album_search_eligible(&lib.library.collection_type, &self.music_levels)
        })
    }

    pub(super) fn open_search_modal_fuzzy(&mut self, lib_idx: usize) {
        self.search_modal_prior_focus = Some(self.panel_focus);
        self.search_modal = Some(SearchModal::new(SearchMode::Fuzzy));
        if self.recursive_album_search_enabled(lib_idx) {
            self.fill_search_modal_corpus_from_album_index(lib_idx);
            return;
        }
        let all_items = self.libs[lib_idx]
            .nav_stack
            .first()
            .and_then(|lvl| lvl.all_items.clone());
        match all_items {
            Some(items) => {
                if let Some(modal) = self.search_modal.as_mut() {
                    modal.corpus = items;
                    modal.loading = false;
                }
            }
            None => {
                if let Some(modal) = self.search_modal.as_mut() {
                    modal.loading = true;
                }
            }
        }
    }

    pub(super) fn open_search_modal_global(&mut self) {
        self.search_modal_prior_focus = Some(self.panel_focus);
        self.search_modal = Some(SearchModal::new(SearchMode::Global));
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

    pub(super) fn fill_search_modal_corpus_from_album_index(&mut self, lib_idx: usize) {
        let library_id = self.libs[lib_idx].library.id.clone();
        let (items, loading) = match self.album_indexes.get(&library_id) {
            Some(AlbumIndexState::Ready(entries)) => {
                (entries.iter().map(|e| e.album.clone()).collect(), false)
            }
            Some(AlbumIndexState::Loading { .. }) => (Vec::new(), true),
            _ => (Vec::new(), false),
        };
        if let Some(modal) = self.search_modal.as_mut() {
            modal.corpus = items;
            modal.loading = loading;
        }
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
        let client = self.client.lock().unwrap().clone();
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
        let client = self.client.lock().unwrap().clone();
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

    pub(in crate::app) fn maybe_fetch_next_page(&mut self, lib_idx: usize) {
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
        if !is_feed_home_video_root && lvl.cursor + PREFETCH_AHEAD < lvl.items.len() {
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
