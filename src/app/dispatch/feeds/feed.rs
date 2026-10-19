use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{App, BrowseLevel, FeedHomeVideoSelection, FeedHomeVideoState};
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

    pub(in crate::app) fn remove_item_from_feed_home_video_cache(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        for items in state.items.values_mut() {
            items.retain(|item| item.id != item_id);
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "remove_from_cache");
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
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            state.selected_group = group_idx;
            state.selection = selection;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "select_group");
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
