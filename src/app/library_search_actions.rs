use super::library_browse_actions::{
    build_album_index_with, fetch_all_album_index_items, recursive_album_search_eligible,
};
use super::search_sidebar::SearchSidebar;
use super::{
    AlbumIndexState, AlbumSearchEntry, App, BrowseLevel, LibEvent, PAGE_SIZE, PREFETCH_AHEAD,
};
impl App {
    /// Open the global search sidebar. Does not touch `panel_focus` --
    /// see `design.md` Decision 4: the sidebar locks input via its
    /// `CONTEXT_STACK` position, not a saved/restored focus.
    pub(super) fn open_search_sidebar(&mut self) {
        self.search_sidebar = Some(SearchSidebar::new());
    }

    /// Close the search sidebar without navigating.
    pub(super) fn dismiss_search_sidebar(&mut self) {
        self.search_sidebar = None;
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
        if refresh {
            self.sync_recursive_album_search(lib_idx);
        }
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

    pub(super) fn open_recursive_album_search(&mut self, lib_idx: usize) -> bool {
        if !self.recursive_album_search_enabled(lib_idx) {
            return false;
        }
        self.libs[lib_idx].search = Some(super::LibSearch {
            query: String::new(),
            items: Vec::new(),
            results: Vec::new(),
            cursor: 0,
            scroll: 0,
            loading: false,
        });
        self.sync_recursive_album_search(lib_idx);
        true
    }

    // Visibility bump: private -> `pub(super)`. Called from
    // `handle_lib_event`'s `AlbumIndexBuilt` handler, which stays behind in
    // `actions.rs`.
    pub(super) fn sync_recursive_album_search(&mut self, lib_idx: usize) {
        if !self.recursive_album_search_enabled(lib_idx) || self.libs[lib_idx].search.is_none() {
            return;
        }
        let library_id = self.libs[lib_idx].library.id.clone();
        let (items, loading) = match self.album_indexes.get(&library_id) {
            Some(AlbumIndexState::Ready(entries)) => (
                entries.iter().map(|entry| entry.album.clone()).collect(),
                false,
            ),
            Some(AlbumIndexState::Loading { .. }) => (Vec::new(), true),
            _ => (Vec::new(), false),
        };
        if let Some(search) = self.libs[lib_idx].search.as_mut() {
            search.items = items;
            search.loading = loading;
        }
        self.update_lib_search(lib_idx);
    }

    pub(super) fn recursive_album_search_entry(&self, lib_idx: usize) -> Option<AlbumSearchEntry> {
        if !self.recursive_album_search_enabled(lib_idx) {
            return None;
        }
        let lib = self.libs.get(lib_idx)?;
        let search = lib.search.as_ref()?;
        let item_idx = *search.results.get(search.cursor)?;
        let entries = match self.album_indexes.get(&lib.library.id)? {
            AlbumIndexState::Ready(entries) => entries,
            _ => return None,
        };
        entries.get(item_idx).cloned()
    }

    pub(super) fn activate_recursive_album(&mut self, lib_idx: usize) -> bool {
        let Some(entry) = self.recursive_album_search_entry(lib_idx) else {
            return false;
        };
        let library_id = self.libs[lib_idx].library.id.clone();
        let library_name = self.libs[lib_idx].library.display_name();
        let client = self.client.lock().unwrap().clone();
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
                    cursor,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    scroll: 0,
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
        if lib.search.is_some() {
            return;
        }
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
