use crate::app::dispatch::library::stores::{ClientLibraryEffect, ClientLibraryStores};
use crate::app::state::events::{BrowseEvent, FeedHomeVideoSelectionFailed, LibEvent};
use crate::app::{App, BrowseLevel, FeedHomeVideoSelection, FeedHomeVideoState, PAGE_SIZE};
use mbv_emby_model::EmbyItem;
use mbv_ui_model::browse::{BrowseResting, ServerRows};

mod counts;
mod idle;

/// The selection a pill index names: 0 is All, `n` is the nth counted
/// folder. An index past the counted folders names nothing.
fn selection_for_group(counted: &[EmbyItem], group_idx: usize) -> Option<FeedHomeVideoSelection> {
    match group_idx.checked_sub(1) {
        None => Some(FeedHomeVideoSelection::All),
        Some(folder_idx) => counted
            .get(folder_idx)
            .map(|folder| FeedHomeVideoSelection::Folder(folder.id.clone())),
    }
}

impl App {
    pub(crate) fn log_feed_home_video_state(&self, lib_idx: usize, context: &str) {
        if !tracing::enabled!(target: "feedhv", tracing::Level::DEBUG) {
            return;
        }
        let Some(lib) = self.libs.get(lib_idx) else {
            tracing::debug!(name: "feed.home_video.state_missing", target: "feedhv", context, library_index = lib_idx, "feed home-video state missing");
            return;
        };
        let root = lib.nav_stack.first();
        let feed = lib.feed_home_video.as_ref();
        let selected_group = self.feed_home_video_selected_group_index(lib_idx);
        tracing::debug!(name: "feed.home_video.state_observed", target: "feedhv", context, library_index = lib_idx, library = %lib.library.name, navigation_length = lib.nav_stack.len(), root_parent = root.map_or("", |level| level.parent_id.as_str()), root_item_count = root.map_or(0, |level| level.items.len()), root_loading = root.is_some_and(|level| level.loading), feed_present = feed.is_some(), feed_loading = feed.is_some_and(|state| state.loading), selected_group, item_count = feed.map_or(0, FeedHomeVideoState::selected_len), video_cursor = feed.map_or(0, |state| state.video_cursor), video_scroll = feed.map_or(0, |state| state.video_scroll), group_view = self.is_feed_home_video_group_view(lib_idx), "feed home-video state observed");
    }

    /// Root-listing folders whose per-folder video count is positive: the
    /// folders the pill row offers, in listing order (design D3). The count
    /// fan-out fills `folder_counts`; until it does, no folder pill shows.
    pub(in crate::app) fn feed_home_video_counted_folders(&self, lib_idx: usize) -> Vec<EmbyItem> {
        self.counted_folder_iter(lib_idx).cloned().collect()
    }

    /// The counted folders' ids in pill order: the save boundary's input for
    /// deriving the persisted pill index from the selection.
    pub(in crate::app) fn feed_home_video_counted_folder_ids(&self, lib_idx: usize) -> Vec<String> {
        self.counted_folder_iter(lib_idx)
            .map(|folder| folder.id.clone())
            .collect()
    }

    pub fn counted_folder_iter(&self, lib_idx: usize) -> impl Iterator<Item = &EmbyItem> {
        let lib = self.libs.get(lib_idx);
        let state = lib.and_then(|lib| lib.feed_home_video.as_ref());
        let root = lib.and_then(|lib| lib.nav_stack.first());
        state.zip(root).into_iter().flat_map(|(state, root)| {
            root.items.iter().filter(move |folder| {
                state
                    .folder_counts
                    .get(&folder.id)
                    .copied()
                    .unwrap_or_default()
                    > 0
            })
        })
    }

    fn feed_home_video_visible_group_count(&self, lib_idx: usize) -> usize {
        self.counted_folder_iter(lib_idx).count()
    }

    /// The selected pill's index, derived on demand from the authoritative
    /// selection (0 = All, the folder's counted position plus one otherwise).
    /// A selection whose folder holds no counted pill — its count vanished,
    /// or the counts are not loaded — clamps to the pill row's end, so the
    /// highlight never points past the last painted pill.
    pub fn feed_home_video_selected_group_index(&self, lib_idx: usize) -> usize {
        let Some(lib) = self.libs.get(lib_idx) else {
            return 0;
        };
        let Some(state) = lib.feed_home_video.as_ref() else {
            return 0;
        };
        match &state.selection {
            FeedHomeVideoSelection::All => 0,
            FeedHomeVideoSelection::Folder(id) => {
                let count = self.feed_home_video_visible_group_count(lib_idx);
                self.counted_folder_iter(lib_idx)
                    .position(|folder| folder.id == *id)
                    .map_or(count, |pos| pos + 1)
            }
        }
    }

    pub fn feed_home_video_selected_items(&self, lib_idx: usize) -> Vec<EmbyItem> {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .and_then(|state| state.items.get(&state.selection))
            .cloned()
            .unwrap_or_default()
    }

    pub(in crate::app) fn feed_home_video_selected_parent_id(
        &self,
        lib_idx: usize,
    ) -> Option<String> {
        let selection = self.libs.get(lib_idx)?.feed_home_video.as_ref()?;
        self.feed_home_video_parent_id(lib_idx, &selection.selection)
    }

    /// The parent one target fetches (design D3): All reads the root
    /// listing's own parent, a folder its own id. The target is passed in, so
    /// a fetch the GUI's checked set starts reads its own parent while the
    /// TUI's selection stays what the TUI set.
    fn feed_home_video_parent_id(
        &self,
        lib_idx: usize,
        target: &FeedHomeVideoSelection,
    ) -> Option<String> {
        let lib = self.libs.get(lib_idx)?;
        Some(match target {
            FeedHomeVideoSelection::All => lib.nav_stack.first()?.parent_id.clone(),
            FeedHomeVideoSelection::Folder(id) => id.clone(),
        })
    }

    /// Returns the item currently under the cursor without cloning the whole
    /// selected item list (see `feed_home_video_selected_items`, which
    /// does clone the full list and remains the right choice for callers that
    /// actually need it).
    pub(in crate::app) fn selected_feed_home_video_item(&self, lib_idx: usize) -> Option<EmbyItem> {
        let state = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())?;
        let items = state.items.get(&state.selection)?;
        let idx = state.video_cursor.min(items.len().saturating_sub(1));
        items.get(idx).cloned()
    }

    pub(in crate::app) fn clamp_feed_home_video_state(&mut self, lib_idx: usize) {
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        let items_len = state.selected_len();
        if items_len == 0 {
            state.video_cursor = 0;
            state.video_scroll = 0;
        } else {
            state.video_cursor = state.video_cursor.min(items_len.saturating_sub(1));
            state.video_scroll = state.video_scroll.min(state.video_cursor);
        }
    }

    pub fn ensure_feed_home_video_group_level(&mut self, lib_idx: usize) {
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        if lib.nav_stack.len() != 1 {
            return;
        }
        let ready = lib
            .feed_home_video
            .as_ref()
            .is_some_and(|state| !state.loading);
        if !ready || !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        self.clamp_feed_home_video_state(lib_idx);
    }

    pub fn is_feed_home_video_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        // Any present state keeps the group view: a state whose count fan-out
        // has not produced pills yet (or whose folders hold no videos) must
        // still paint its pill row — the destination-latest-modes spec keeps a
        // Latest pill selectable with an empty source (user report
        // 2026-10-18: the YouTube tab lost Latest/All once everything was
        // watched). Every state writer guards on the configured feed-view
        // library check below, so presence alone cannot widen the view.
        let has_state = lib.feed_home_video.is_some();
        if !has_state {
            return false;
        }
        // Feed home-video libraries use the group view when configured.
        self.is_feed_home_video_library(lib_idx)
            && lib
                .nav_stack
                .first()
                .is_some_and(|lvl| lvl.item_types.is_none())
    }

    pub(in crate::app) fn ensure_feed_home_video_root_loaded(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let needs_reload = self.libs.get(lib_idx).is_some_and(|lib| {
            lib.nav_stack.is_empty()
                || (!lib.nav_stack[0].loading
                    && lib.nav_stack[0]
                        .items
                        .first()
                        .is_none_or(|item| !item.is_folder))
        });
        if !needs_reload {
            return;
        }
        let lib_id = self.libs[lib_idx].library.id.clone();
        let lib_name = self.libs[lib_idx].library.name.clone();
        self.libs[lib_idx].nav_stack.clear();
        self.libs[lib_idx].feed_home_video = Some(FeedHomeVideoState {
            loading: true,
            ..self.libs[lib_idx]
                .feed_home_video
                .take()
                .unwrap_or_default()
        });
        self.libs[lib_idx].nav_stack.push(BrowseLevel {
            rows: ServerRows::new(0),
            parent_id: lib_id.clone(),
            title: lib_name.clone(),
            items: vec![],
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,

            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        });
        self.spawn_browse(
            lib_idx,
            lib_id,
            lib_name,
            None,
            false,
            "SortName".into(),
            "Ascending".into(),
        );
        self.spawn_destination_latest_snapshot(lib_idx);
        self.log_feed_home_video_state(lib_idx, "root_reload");
    }

    /// Whether library `lib_idx` is the home-video library the Feeds tab
    /// shows as a feed home.
    ///
    /// # Panics
    ///
    /// Panics when `lib_idx` names no loaded library, and while reading the
    /// configuration when its lock is poisoned.
    pub fn is_feed_home_video_library(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "homevideos" {
            return false;
        }
        self.config
            .lock()
            .unwrap()
            .feed_view_libraries
            .contains(&lib.library.name.to_lowercase())
    }

    pub(in crate::app) fn select_feed_folder_group(&mut self, lib_idx: usize, group_idx: usize) {
        if self.libs[lib_idx].nav_stack.is_empty() {
            return;
        }
        // Resolve the pill's folder id from the root listing (design D3): the
        // listing, not a state-held group list, is the pill row's source. A
        // folder with no counted videos has no pill, so its index resolves to
        // nothing and the selection stays as it is.
        let counted = self.feed_home_video_counted_folders(lib_idx);
        let Some(selection) = selection_for_group(&counted, group_idx) else {
            return;
        };
        self.apply_feed_home_video_group_selection(lib_idx, &selection);
    }

    /// Apply one feed-group selection to the library's feed state: the
    /// selection write, the cursor reset, the stale-fetch drop, the lazy
    /// fetch trigger, and the state log. This is the TUI's pill press
    /// (`select_feed_folder_group`); the GUI Library never calls it, because
    /// its own checked folder set reads the shared cache without moving the
    /// TUI's selection (gui-collection-dropdown design D2, D3). The callers
    /// guard the library index; an empty nav stack leaves the selection as it
    /// is.
    pub(in crate::app) fn apply_feed_home_video_group_selection(
        &mut self,
        lib_idx: usize,
        selection: &FeedHomeVideoSelection,
    ) {
        if self.libs[lib_idx].nav_stack.is_empty() {
            return;
        }
        let mut changed = false;
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            changed = state.selection != *selection;
            // The user's pick supersedes any still-unresolved restored pill
            // index.
            state.restored_group = None;
            state.selection = selection.clone();
            if changed {
                state.video_cursor = 0;
                state.video_scroll = 0;
                // A cached selection has no fetch to wait for; an outstanding
                // fetch belonged to the previous selection and is
                // stale-dropped on arrival.
                if state.items.contains_key(selection) {
                    state.loading = false;
                }
            }
        }
        self.clamp_feed_home_video_state(lib_idx);
        // Lazy fetch (design D2): a newly selected, uncached selection
        // fetches at selection time; a cached one reuses the session cache
        // and sets no fetch. An unchanged selection leaves the outstanding
        // fetch — if any — alone; the shown pass covers the transient
        // uncached case.
        if changed {
            self.ensure_feed_home_video_selection_loaded(lib_idx, true);
        }
        self.log_feed_home_video_state(lib_idx, "select_group");
    }

    /// The lazy per-selection fetch trigger (design D2), both entry paths in
    /// one. The shown/push pass passes `prior_fetch_stale` false and fires
    /// only when the selection is uncached and nothing is fetching it: while
    /// a fetch is outstanding (`loading`) or the selection is cached, this is
    /// a no-op. The select path passes true after a changed selection and
    /// owns the hand-off — the outstanding `loading` described the previous
    /// selection, whose result is stale-dropped on arrival, so it clears
    /// before the spawn (review P1, b603f329b..9e454b62d): leaving it set
    /// would wedge the view with a spinner and no fetch for the shown
    /// selection, and switching to a cached selection must drop it too. A
    /// cached selection never spawns; without a Service client the selection
    /// stays uncached and unloaded, and a later selection or shown pass
    /// retries.
    pub fn ensure_feed_home_video_selection_loaded(
        &mut self,
        lib_idx: usize,
        prior_fetch_stale: bool,
    ) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let cached_or_loading = {
            let Some(state) = self
                .libs
                .get_mut(lib_idx)
                .and_then(|lib| lib.feed_home_video.as_mut())
            else {
                return;
            };
            if prior_fetch_stale {
                state.loading = false;
            }
            let cached_or_loading = state.items.contains_key(&state.selection) || state.loading;
            // Mark the fetch outstanding only once the gates pass; the
            // client and parent guards below must leave an unfetchable
            // selection unloaded so a later pass can retry.
            if !cached_or_loading {
                state.loading = true;
            }
            cached_or_loading
        };
        if cached_or_loading {
            return;
        }
        let selection = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.selection.clone());
        let Some(selection) = selection else {
            return;
        };
        if self.spawn_feed_home_video_fetch(lib_idx, selection) {
            return;
        }
        // The fetch could not start (no Service client yet, or no parent to
        // read): leave the selection uncached and unmarked, so a later
        // selection or shown pass retries.
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.loading = false;
        }
    }

    /// Spawn one feed-group target's items fetch (design D3): the body of the
    /// TUI's selection fetch with the target passed in instead of read from
    /// the state, so the GUI's checked folders fetch through the same call
    /// without moving the TUI's selection. The fetch marks `fetching` against
    /// its target — the listing completion's `loading` hand-off and the
    /// result arm both read that to tell a live fetch apart from the
    /// listing-pending seed — and its result lands through
    /// `FeedHomeVideoSelectionLoaded`. Returns whether the fetch started: a
    /// fetch that cannot start leaves no mark, so nothing waits on it.
    pub fn spawn_feed_home_video_fetch(
        &mut self,
        lib_idx: usize,
        target: FeedHomeVideoSelection,
    ) -> bool {
        let Some(client) = self.emby_snapshot() else {
            return false;
        };
        let Some(parent_id) = self.feed_home_video_parent_id(lib_idx, &target) else {
            return false;
        };
        // The All list reads newest-first (spec `feed-home-video-group-view`);
        // folder lists keep the flat oldest-first order.
        let sort_order = match &target {
            FeedHomeVideoSelection::All => "Descending",
            FeedHomeVideoSelection::Folder(_) => "Ascending",
        };
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return false;
        };
        state.fetching = Some(target.clone());
        self.log_feed_home_video_state(lib_idx, "selection_fetch_spawned");
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let (mut items, total_count) = match client.get_items_sorted(
                &parent_id,
                Some("Video"),
                false,
                0,
                PAGE_SIZE,
                "DateCreated",
                sort_order,
            ) {
                Ok(result) => result,
                Err(e) => {
                    // A failed fetch reports its own target (design D3), so the
                    // result arm can retire the in-flight mark and the GUI's
                    // shown pass can mark the target failed.
                    let _ = tx.send(LibEvent::FeedHomeVideoSelectionFailed(
                        FeedHomeVideoSelectionFailed {
                            lib_idx,
                            selection: target.clone(),
                            error: e.to_string(),
                        },
                    ));
                    return;
                }
            };
            // Initial page then total refill (design D2): the first page
            // arrives fast, then the remainder is appended so the parent's
            // full item set is held when the total exceeds the page.
            if total_count > items.len() {
                match client.get_items_sorted(
                    &parent_id,
                    Some("Video"),
                    false,
                    items.len(),
                    total_count - items.len(),
                    "DateCreated",
                    sort_order,
                ) {
                    Ok((rest, _)) => items.extend(rest),
                    Err(e) => {
                        let _ = tx.send(LibEvent::FeedHomeVideoSelectionFailed(
                            FeedHomeVideoSelectionFailed {
                                lib_idx,
                                selection: target.clone(),
                                error: e.to_string(),
                            },
                        ));
                        return;
                    }
                }
            }
            let _ = tx.send(LibEvent::Browse(
                BrowseEvent::FeedHomeVideoSelectionLoaded {
                    lib_idx,
                    parent_id,
                    selection: target,
                    items,
                },
            ));
        });
        true
    }

    /// The `FeedHomeVideoSelectionLoaded` arm. Stale guard: a result serves
    /// the library only while its selection is still wanted (the TUI's own
    /// selection, or a target the GUI's checked set still holds — design D3)
    /// and its root parent still matches (an All fetch's source listing). A
    /// selection left before its fetch returned is dropped, and the cache
    /// fills only through a wanted selection; only the TUI's own selection
    /// ends its loading mark.
    pub(in crate::app) fn handle_feed_home_video_selection_loaded(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        selection: FeedHomeVideoSelection,
        items: Vec<EmbyItem>,
        stores: Option<&mut dyn ClientLibraryStores>,
    ) {
        let current = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .is_some_and(|state| state.selection == selection);
        let wanted = self.feed_home_video_selection_wanted(lib_idx, &selection, stores.as_deref());
        let stale = !wanted
            || matches!(selection, FeedHomeVideoSelection::All)
                && !self.feed_home_video_root_matches(lib_idx, parent_id);
        // The event's store work keeps the order of the direct calls this
        // arm made: the target's terminal failure mark retires first (the
        // fetch returned, so a later cache miss can fetch it again), then the
        // items land. The retirement is applied through `stores` here, at the
        // position it held in the core before the split; the arm asked a
        // Client for nothing else, so no row re-queue follows.
        self.apply_library_effects(
            stores,
            &[ClientLibraryEffect::FeedTargetLanded {
                lib_idx,
                selection: selection.clone(),
            }],
        );
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            // The fetch has returned, whatever the disposition: retire its
            // outstanding-fetch mark when this result is the one it named.
            if state.fetching.as_ref() == Some(&selection) {
                state.fetching = None;
            }
            if !stale {
                state.items.insert(selection, items);
                // Only the TUI's own selection ends its loading mark: a
                // checked folder's list the GUI fetched beside it answers no
                // spinner the TUI paints.
                if current {
                    state.loading = false;
                }
            }
        }
        if stale {
            self.log_feed_home_video_state(lib_idx, "selection_loaded_dropped_stale");
            return;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "selection_loaded");
    }

    /// Whether a landed target's list still serves the library (design D3):
    /// the TUI's rule (its selection is still the state's current one) or the
    /// GUI's checked set still holds the target, so a fetch the GUI started
    /// lands while the TUI's selection stays what the TUI set.
    fn feed_home_video_selection_wanted(
        &self,
        lib_idx: usize,
        target: &FeedHomeVideoSelection,
        stores: Option<&dyn ClientLibraryStores>,
    ) -> bool {
        let current = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .is_some_and(|state| state.selection == *target);
        current || stores.is_some_and(|stores| stores.wants_feed_target(self, lib_idx, target))
    }

    /// Rule 5: when the folder listing completes (open, restore), the
    /// open/restore seed's `loading` hand-off ends and the count fan-out
    /// re-queries every folder's count. The completion guard makes this the
    /// single "root listing became fully loaded" transition: page appends
    /// only occur while the listing is incomplete, so exactly the final
    /// append fires here per full pagination, and a root `Loaded` arrives
    /// only on a (re)build. That is what keeps a refresh's counts honest
    /// beyond its first page (design D2): the partial refresh re-queries the
    /// folders it can see, and the re-appended later pages' stale entries are
    /// re-queried here instead of short-circuiting on full coverage. A
    /// pending restored pill index resolves only against a final counted row:
    /// either the counts already cover every root folder (no fan-out is
    /// owed), or the fan-out this completion spawns resolves it when its last
    /// report lands — a partial arrival must never misbind it.
    pub(in crate::app) fn maybe_complete_feed_home_video_listing(&mut self, lib_idx: usize) {
        if !self.feed_home_video_root_listing_completed(lib_idx) {
            return;
        }
        // The listing-pending phase is over: the open/restore seed's
        // `loading` hand-off ends here, and the fetch phase owns the flag
        // from the next shown pass on. A selection fetch already in flight
        // keeps the flag — its own result clears it on arrival (spec
        // `feed-home-video-group-view`, "An unloaded selection SHALL show
        // its loading state until its fetch returns"); clearing it here
        // would drop the spinner while the fetch is still running and let
        // the shown pass double-spawn the same selection.
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
            && state.fetching.is_none()
        {
            state.loading = false;
        }
        if self.feed_home_video_folder_counts_cover_root(lib_idx) {
            self.resolve_feed_home_video_pending_selection(lib_idx);
        }
        self.spawn_feed_home_video_folder_counts(lib_idx);
        self.log_feed_home_video_state(lib_idx, "listing_completed");
    }

    /// The refresh-completion hook for a refreshed hidden root listing
    /// (design D2): returns whether the refreshed level was the feed-view
    /// library's single root listing with the given parent — the caller's
    /// pagination re-arm consumes that disposition. Inside, the refreshed
    /// folder listing invalidates the whole session cache — the current
    /// selection refetches through the shown projection's lazy-fetch trigger,
    /// and unloaded pills stay unloaded. The counts refresh with the listing
    /// (design D2): a fully paginated refresh clears the count map, so the
    /// fan-out below re-queries every folder; a partial one (the library
    /// total grew since the last full pagination, so `spawn_refresh` fetched
    /// only the first page) keeps the map and commits the fan-out for the
    /// folders it can see — per-folder inserts update those entries without
    /// shrinking the row (review P2, b603f329b..9e454b62d: page-1 folders
    /// must not decide the whole row), and the re-appended later pages'
    /// counts re-query at the listing-completion transition, whose fan-out
    /// covers every folder even when the map already looks fully covered.
    /// The pill-index resolution stays gated on the completed listing, so
    /// neither fan-out can misbind it.
    pub(in crate::app) fn complete_feed_home_video_refresh(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
    ) -> bool {
        // One guard site for the refreshed hidden root listing: the library
        // must be a feed-view library and the refreshed level must still be
        // its single root level with the same parent — a deeper level's
        // refresh or a foreign parent is not the hidden listing. The guard
        // lives here alone, so a caller-side predicate cannot drift from it.
        let is_feed_root_refresh = self.is_feed_home_video_library(lib_idx)
            && self.libs.get(lib_idx).is_some_and(|lib| {
                lib.nav_stack.len() == 1 && lib.nav_stack[0].parent_id == parent_id
            });
        if !is_feed_root_refresh {
            return false;
        }
        let fully_loaded = self.feed_home_video_root_listing_completed(lib_idx);
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.items.clear();
            // An outstanding fetch for the still-current selection survives
            // the refresh — the guard above pinned the root parent, so its
            // result stays valid — and keeps the loading flag until it lands
            // (spec `feed-home-video-group-view`, "An unloaded selection
            // SHALL show its loading state until its fetch returns"); any
            // other `loading` here is the refresh seed and ends now, leaving
            // the shown pass to respawn the unloaded selection.
            state.loading = state.fetching.as_ref() == Some(&state.selection);
            if fully_loaded {
                state.folder_counts.clear();
            }
        }
        // Both refresh shapes re-query the counts: the cleared map re-queries
        // everything after a full pagination; the partial one re-queries its
        // folders here, and its final page append re-queries whatever the
        // partial listing could not see.
        self.spawn_feed_home_video_folder_counts(lib_idx);
        self.log_feed_home_video_state(lib_idx, "refresh_completed");
        true
    }

    /// The root folder listing has fully paginated: every counted folder is
    /// known, so a pending pill index can resolve against it.
    fn feed_home_video_root_listing_completed(&self, lib_idx: usize) -> bool {
        self.is_feed_home_video_library(lib_idx)
            && self.libs.get(lib_idx).is_some_and(|lib| {
                lib.nav_stack.len() == 1
                    && lib.nav_stack[0].item_types.is_none()
                    && !lib.nav_stack[0].loading
                    && lib.nav_stack[0].is_fully_loaded()
            })
    }

    /// Whether `parent_id` still names the current root listing's parent: the
    /// shared stale-guard base for the fan-out's result arms. A library
    /// switch or a replaced root drops a result; each arm adds only its own
    /// extra condition on top.
    fn feed_home_video_root_matches(&self, lib_idx: usize, parent_id: &str) -> bool {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.nav_stack.first())
            .is_some_and(|root| root.parent_id == parent_id)
    }

    fn resolve_feed_home_video_pending_selection(&mut self, lib_idx: usize) {
        let counted = self.feed_home_video_counted_folders(lib_idx);
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        let Some(group_idx) = state.restored_group else {
            return;
        };
        // The callers resolve only once the counted row is final (the
        // fan-out has quiesced over a completed listing, or the counts
        // already cover every root folder), so the index consumes here
        // whether or not it names a pill: a vanished folder's index is
        // spent, the selection stays as it is, and the derived highlight
        // clamps into the row.
        state.restored_group = None;
        if let Some(selection) = selection_for_group(&counted, group_idx)
            && state.selection != selection
        {
            state.selection = selection;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
    }
}
