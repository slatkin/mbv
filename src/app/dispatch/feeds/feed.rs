use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::events::{BrowseEvent, LibEvent};
use crate::app::{App, BrowseLevel, FeedHomeVideoSelection, FeedHomeVideoState, PAGE_SIZE};
use mbv_emby_model::EmbyItem;
use mbv_feed::fetch_and_parse_rss;
use mbv_ui_model::browse::{BrowseResting, ServerRows};
use std::time::Instant;

impl App {
    pub(in crate::app) fn log_feed_home_video_state(&self, lib_idx: usize, context: &str) {
        let Some(lib) = self.libs.get(lib_idx) else {
            tracing::debug!(name: "feed.home_video.state_missing", target: "feedhv", context, library_index = lib_idx, "feed home-video state missing");
            return;
        };
        let root = lib.nav_stack.first();
        let feed = lib.feed_home_video.as_ref();
        let selected_group = feed.map_or(0, |state| {
            state
                .selected_group
                .min(self.feed_home_video_visible_group_count(lib_idx))
        });
        tracing::debug!(name: "feed.home_video.state_observed", target: "feedhv", context, library_index = lib_idx, library = %lib.library.name, navigation_length = lib.nav_stack.len(), root_parent = root.map_or("", |level| level.parent_id.as_str()), root_item_count = root.map_or(0, |level| level.items.len()), root_loading = root.is_some_and(|level| level.loading), feed_present = feed.is_some(), feed_loading = feed.is_some_and(|state| state.loading), selected_group, item_count = feed.map_or(0, FeedHomeVideoState::selected_len), video_cursor = feed.map_or(0, |state| state.video_cursor), video_scroll = feed.map_or(0, |state| state.video_scroll), group_view = self.is_feed_home_video_group_view(lib_idx), "feed home-video state observed");
    }

    /// Root-listing folders whose per-folder video count is positive: the
    /// folders the pill row offers, in listing order (design D3). The count
    /// fan-out fills `folder_counts`; until it does, no folder pill shows.
    pub(in crate::app) fn feed_home_video_counted_folders(&self, lib_idx: usize) -> Vec<EmbyItem> {
        let Some(lib) = self.libs.get(lib_idx) else {
            return Vec::new();
        };
        let Some(state) = lib.feed_home_video.as_ref() else {
            return Vec::new();
        };
        let Some(root) = lib.nav_stack.first() else {
            return Vec::new();
        };
        root.items
            .iter()
            .filter(|folder| {
                state
                    .folder_counts
                    .get(&folder.id)
                    .copied()
                    .unwrap_or_default()
                    > 0
            })
            .cloned()
            .collect()
    }

    fn feed_home_video_visible_group_count(&self, lib_idx: usize) -> usize {
        self.feed_home_video_counted_folders(lib_idx).len()
    }

    pub(in crate::app) fn feed_home_video_selected_group_index(&self, lib_idx: usize) -> usize {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map_or(0, |state| {
                state
                    .selected_group
                    .min(self.feed_home_video_visible_group_count(lib_idx))
            })
    }

    pub(in crate::app) fn feed_home_video_selected_items(&self, lib_idx: usize) -> Vec<EmbyItem> {
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
        let lib = self.libs.get(lib_idx)?;
        let state = lib.feed_home_video.as_ref()?;
        Some(match &state.selection {
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
        let folder_count = self.feed_home_video_visible_group_count(lib_idx);
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        state.selected_group = state.selected_group.min(folder_count);
        let items_len = state.selected_len();
        if items_len == 0 {
            state.video_cursor = 0;
            state.video_scroll = 0;
        } else {
            state.video_cursor = state.video_cursor.min(items_len.saturating_sub(1));
            state.video_scroll = state.video_scroll.min(state.video_cursor);
        }
    }

    pub(in crate::app) fn ensure_feed_home_video_group_level(&mut self, lib_idx: usize) {
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

    pub(in crate::app) fn is_feed_home_video_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        // Any present state keeps the group view: an aggregate that completed
        // empty (every video played, so the unplayed-only fetch returns none)
        // must still paint its pill row — the destination-latest-modes spec
        // keeps a Latest pill selectable with an empty source (user report
        // 2026-10-18: the YouTube tab lost Latest/All once everything was
        // watched). Every state writer guards on the configured feed-view
        // library check below, so presence alone cannot widen the view.
        let has_state = lib.feed_home_video.is_some();
        if !has_state {
            return false;
        }
        // Feed home-video libraries use the group view when configured.
        if lib.library.collection_type != "homevideos" {
            return false;
        }
        self.config
            .lock()
            .unwrap()
            .feed_view_libraries
            .contains(&lib.library.name.to_lowercase())
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

    pub(crate) fn is_feed_home_video_library(&self, lib_idx: usize) -> bool {
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
        let n = self.feed_home_video_visible_group_count(lib_idx);
        if group_idx > n {
            return;
        }
        // Resolve the pill's folder id from the root listing (design D3): the
        // listing, not a state-held group list, is the pill row's source. A
        // folder with no counted videos has no pill, so its index resolves to
        // nothing and the selection stays as it is.
        let counted = self.feed_home_video_counted_folders(lib_idx);
        let selection = if group_idx == 0 {
            Some(FeedHomeVideoSelection::All)
        } else {
            counted
                .get(group_idx - 1)
                .map(|folder| FeedHomeVideoSelection::Folder(folder.id.clone()))
        };
        let Some(selection) = selection else {
            return;
        };
        let mut changed = false;
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            changed = state.selection != selection;
            state.selected_group = group_idx;
            state.selection = selection.clone();
            if changed {
                state.video_cursor = 0;
                state.video_scroll = 0;
                // A cached selection has no fetch to wait for; an outstanding
                // fetch belonged to the previous selection and is
                // stale-dropped on arrival.
                if state.items.contains_key(&selection) {
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
            self.fetch_feed_home_video_selection_after_change(lib_idx);
        }
        self.log_feed_home_video_state(lib_idx, "select_group");
    }

    /// The shown selection's items are not cached and no fetch for it is
    /// outstanding: its lazy fetch (design D2) still has to run. The shown
    /// and Latest-mode gates live at the shell call sites (`App` cannot name
    /// component-local Latest state); tests use this predicate to observe the
    /// refetch decision a refresh leaves behind.
    pub(in crate::app) fn feed_home_video_selection_awaits_fetch(&self, lib_idx: usize) -> bool {
        if !self.is_feed_home_video_library(lib_idx) {
            return false;
        }
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .is_some_and(|state| !state.loading && !state.items.contains_key(&state.selection))
    }

    /// Shown-time trigger for the lazy per-selection fetch (design D2): fires
    /// only when the selection is uncached and nothing is fetching it. While
    /// a fetch is outstanding (`loading`) or the selection is cached, this is
    /// a no-op.
    pub(in crate::app) fn maybe_fetch_feed_home_video_selection(&mut self, lib_idx: usize) {
        if self.feed_home_video_selection_awaits_fetch(lib_idx) {
            self.fetch_feed_home_video_selection_now(lib_idx);
        }
    }

    /// Selection-change trigger for the lazy per-selection fetch (design D2):
    /// spawns whenever the new selection is uncached, even while a fetch for
    /// the previous selection is still outstanding — that result is
    /// stale-dropped on arrival. The outstanding `loading` describes the
    /// previous selection, so an uncached change clears it before the spawn
    /// (review P1, b603f329b..9e454b62d): leaving it set would wedge the view
    /// with a spinner and no fetch for the shown selection.
    fn fetch_feed_home_video_selection_after_change(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let uncached = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
            .is_some_and(|state| {
                let uncached = !state.items.contains_key(&state.selection);
                if uncached {
                    state.loading = false;
                }
                uncached
            });
        if uncached {
            self.fetch_feed_home_video_selection_now(lib_idx);
        }
    }

    /// Marks the current selection's fetch outstanding and spawns it. Callers
    /// own the cached and shown gates.
    fn fetch_feed_home_video_selection_now(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            // No Service client yet: leave the selection uncached and
            // unloaded; a later selection or shown pass retries.
            return;
        };
        let Some(lib) = self.libs.get_mut(lib_idx) else {
            return;
        };
        let Some(state) = lib.feed_home_video.as_mut() else {
            return;
        };
        if state.loading || state.items.contains_key(&state.selection) {
            return;
        }
        let Some(root) = lib.nav_stack.first() else {
            return;
        };
        let selection = state.selection.clone();
        let parent_id = match &selection {
            FeedHomeVideoSelection::All => root.parent_id.clone(),
            FeedHomeVideoSelection::Folder(id) => id.clone(),
        };
        state.loading = true;
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
                "Ascending",
            ) {
                Ok(result) => result,
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e.to_string()));
                    return;
                }
            };
            // Initial page then total refill (design D2): the same shape the
            // deleted aggregate used, minus the unplayed filter and grouping.
            if total_count > items.len() {
                match client.get_items_sorted(
                    &parent_id,
                    Some("Video"),
                    false,
                    0,
                    total_count,
                    "DateCreated",
                    "Ascending",
                ) {
                    Ok((refetched, _)) => items = refetched,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e.to_string()));
                        return;
                    }
                }
            }
            let _ = tx.send(LibEvent::Browse(
                BrowseEvent::FeedHomeVideoSelectionLoaded {
                    lib_idx,
                    parent_id,
                    selection,
                    items,
                },
            ));
        });
    }

    /// The count fan-out (design D1 (b), task 3.3): one background thread
    /// queries the per-folder videos-only count for every folder directly
    /// under the root listing, sequentially, and sends ONE result event
    /// carrying the successfully obtained pairs. A failed query omits its
    /// folder — it gets no pill until a later listing completion or refresh
    /// retries. The listing's `child_count` is not consulted (the root
    /// listing requests no count fields) and nothing here lists any videos.
    /// Without `force`, a listing whose folders all hold count entries
    /// already spawns nothing; a refresh forces a re-query of every folder
    /// because the refreshed counts replace the map — immediately when the
    /// refreshed listing is fully loaded, otherwise deferred through the
    /// state's `pending_count_refresh` marker to the listing's final page
    /// append. The marker clears exactly here, when the forced fan-out is
    /// actually requested; without a Service client it survives and the next
    /// listing completion retries.
    pub(in crate::app) fn spawn_feed_home_video_folder_counts(
        &mut self,
        lib_idx: usize,
        force: bool,
    ) {
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
        let Some(state) = lib.feed_home_video.as_ref() else {
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
        if folder_ids.is_empty()
            || (!force
                && folder_ids
                    .iter()
                    .all(|id| state.folder_counts.contains_key(id)))
        {
            return;
        }
        let parent_id = root.parent_id.clone();
        // The deferred refresh force is consumed exactly at the commit point:
        // a forced request that reached here passed the non-forced dedupe, so
        // the marker's clearing proves the fan-out was requested (review P2,
        // 9e454b62d..8e5ce710a).
        if force
            && let Some(state) = self
                .libs
                .get_mut(lib_idx)
                .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.pending_count_refresh = false;
        }
        let tx = self.channels.lib_tx.clone();
        tracing::debug!(name: "feed.home_video.counts_spawned", target: "feedhv", library_index = lib_idx, folders = folder_ids.len(), "folder video-count fan-out spawned");
        std::thread::spawn(move || {
            let mut counts = Vec::with_capacity(folder_ids.len());
            for folder_id in folder_ids {
                match client.get_folder_video_count(&folder_id) {
                    Ok(count) => counts.push((folder_id, count)),
                    Err(error) => {
                        tracing::warn!(name: "feed.home_video.folder_count_failed", target: "feedhv", folder = %folder_id, error = %error, "folder video-count query failed");
                    }
                }
            }
            let _ = tx.send(LibEvent::Browse(
                BrowseEvent::FeedHomeVideoFolderCountsLoaded {
                    lib_idx,
                    parent_id,
                    counts,
                },
            ));
        });
    }

    /// The `FeedHomeVideoFolderCountsLoaded` arm (task 3.3). Stale guard: a
    /// result serves the library only while its root parent still matches the
    /// current root listing's parent — a library switch or a replaced root
    /// drops it. The pairs replace the count map wholesale (a refresh
    /// re-queries every folder), then the pending persisted pill index
    /// resolves against the new pill row and the cursor clamps to it. The
    /// selection's own fetch stays with the shown-pass trigger (design D2);
    /// nothing here lists any videos.
    pub(in crate::app) fn handle_feed_home_video_folder_counts_loaded(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        counts: Vec<(String, usize)>,
    ) {
        let stale = self.libs.get(lib_idx).is_none_or(|lib| {
            lib.feed_home_video.is_none()
                || lib
                    .nav_stack
                    .first()
                    .is_none_or(|root| root.parent_id != parent_id)
        });
        if stale {
            self.log_feed_home_video_state(lib_idx, "folder_counts_dropped_stale");
            return;
        }
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.folder_counts = counts.into_iter().collect();
        }
        self.resolve_feed_home_video_pending_selection(lib_idx);
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "folder_counts_loaded");
    }

    /// The `FeedHomeVideoSelectionLoaded` arm. Stale guard: a result serves
    /// the library only while its root parent still matches (an All fetch's
    /// source listing) and its selection is still the state's current one; a
    /// selection left before its fetch returned is dropped, and the cache
    /// fills only through a live selection.
    pub(in crate::app) fn handle_feed_home_video_selection_loaded(
        &mut self,
        lib_idx: usize,
        parent_id: &str,
        selection: FeedHomeVideoSelection,
        items: Vec<EmbyItem>,
    ) {
        let stale = self.libs.get(lib_idx).is_none_or(|lib| {
            lib.feed_home_video.as_ref().is_none_or(|state| {
                state.selection != selection
                    || matches!(selection, FeedHomeVideoSelection::All)
                        && lib
                            .nav_stack
                            .first()
                            .is_none_or(|root| root.parent_id != parent_id)
            })
        });
        if stale {
            self.log_feed_home_video_state(lib_idx, "selection_loaded_dropped_stale");
            return;
        }
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.items.insert(selection, items);
            state.loading = false;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "selection_loaded");
    }

    /// Rule 5: when the folder listing completes (open, restore), resolve the
    /// persisted pill index into the concrete selection (design D3). A folder
    /// index past the counted folders — the count fan-out is still empty, or
    /// the folder vanished — leaves the selection as it is. Resolution that
    /// keeps the selection leaves the cursor alone; the fetch itself stays
    /// shown-gated at the shell call sites.
    pub(in crate::app) fn maybe_complete_feed_home_video_listing(&mut self, lib_idx: usize) {
        if !self.feed_home_video_root_listing_completed(lib_idx) {
            return;
        }
        // The listing-pending phase is over: the open/restore seed's
        // `loading` hand-off ends here, and the fetch phase owns the flag
        // from the next shown pass on.
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.loading = false;
        }
        // A partial refresh deferred its forced count re-query to this
        // completion (review P2, 9e454b62d..8e5ce710a): carry the marker's
        // force into the fan-out; the fan-out consumes the marker when the
        // forced request commits.
        let force_counts = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .is_some_and(|state| state.pending_count_refresh);
        self.resolve_feed_home_video_pending_selection(lib_idx);
        self.spawn_feed_home_video_folder_counts(lib_idx, force_counts);
        self.log_feed_home_video_state(lib_idx, "listing_completed");
    }

    /// The refresh-completion hook (design D2): the refreshed folder listing
    /// invalidates the whole session cache — the current selection refetches
    /// through the shown projection's lazy-fetch trigger, and unloaded pills
    /// stay unloaded. The count replacement and the pill-index re-resolution
    /// wait for the refreshed root listing to fully paginate (review P2,
    /// b603f329b..9e454b62d): a refresh that lands only part of the folders
    /// (the library total grew since the last full pagination) must not
    /// replace `folder_counts` from its page-1 folders, nor clamp or
    /// re-resolve the pill index against the shrunken pill row while
    /// `selection` keeps a later-page folder. A full refresh forces the count
    /// re-query now; a partial one arms `pending_count_refresh` so the
    /// listing's final page append runs exactly one forced fan-out — even
    /// when the folder set is unchanged and the non-forced dedupe would skip
    /// it (review P2, 9e454b62d..8e5ce710a).
    pub(in crate::app) fn complete_feed_home_video_refresh(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let fully_loaded = self.feed_home_video_root_listing_completed(lib_idx);
        if let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        {
            state.items.clear();
            state.loading = false;
            state.pending_count_refresh = !fully_loaded;
        }
        if fully_loaded {
            self.resolve_feed_home_video_pending_selection(lib_idx);
            self.spawn_feed_home_video_folder_counts(lib_idx, true);
        }
        self.log_feed_home_video_state(lib_idx, "refresh_completed");
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

    fn resolve_feed_home_video_pending_selection(&mut self, lib_idx: usize) {
        let counted = self.feed_home_video_counted_folders(lib_idx);
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        let resolved = if state.selected_group == 0 {
            Some(FeedHomeVideoSelection::All)
        } else {
            counted
                .get(state.selected_group - 1)
                .map(|folder| FeedHomeVideoSelection::Folder(folder.id.clone()))
        };
        if let Some(selection) = resolved
            && state.selection != selection
        {
            state.selection = selection;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
    }

    /// Spawn a background thread to fetch and parse the idle RSS feed.
    pub(in crate::app) fn spawn_idle_feed_fetch(&self) {
        let Some(ref idle_feed) = self.idle_feed else {
            return;
        };
        let rss_url = self.config.lock().unwrap().idle_feed_rss_url.clone();
        let tx = idle_feed.items_tx.clone();
        std::thread::spawn(move || {
            let items = match fetch_and_parse_rss(&rss_url) {
                Ok(items) => items,
                Err(e) => {
                    tracing::warn!(name: "idle_feed.fetch.failed", target: "idle_feed", error = %e, "RSS feed fetch failed");
                    Vec::new()
                }
            };
            let _ = tx.send(items);
        });
    }

    pub(in crate::app) fn idle_feed_link_available(&self) -> bool {
        self.idle_feed
            .as_ref()
            .and_then(|feed| feed.items.get(feed.current_index))
            .and_then(|item| item.link.as_deref())
            .is_some_and(|link| !link.is_empty())
    }

    /// Open the currently displayed idle-feed item in the user's browser.
    pub(in crate::app) fn open_idle_feed_link(&mut self) {
        let Some(link) = self
            .idle_feed
            .as_ref()
            .and_then(|feed| feed.items.get(feed.current_index))
            .and_then(|item| item.link.as_deref())
            .filter(|link| !link.is_empty())
            .map(str::to_owned)
        else {
            return;
        };

        if let Err(error) = crate::app::open_url(&link) {
            tracing::warn!(name: "idle_feed.link_open.failed", target: "idle_feed", { url.path = %mbv_core::applog::UrlPath(&link), error = %error }, "feed link open failed");
            self.flash(
                format!("Unable to open feed link: {error}"),
                ToastSeverity::Neutral,
            );
        }
    }

    /// Advance the idle feed rotation if enough time has elapsed.
    pub(in crate::app) fn advance_idle_feed_rotation(&mut self) {
        let Some(ref mut idle_feed) = self.idle_feed else {
            return;
        };
        if idle_feed.items.is_empty() {
            return;
        }
        let rotation_secs = self.config.lock().unwrap().idle_feed_rotation_secs;
        if idle_feed.last_rotation.elapsed() >= std::time::Duration::from_secs(rotation_secs) {
            idle_feed.current_index = (idle_feed.current_index + 1) % idle_feed.items.len();
            idle_feed.last_rotation = Instant::now();
        }
    }
}
