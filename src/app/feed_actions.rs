use super::feed_parse::fetch_and_parse_rss;
use super::{App, BrowseLevel, FeedHomeVideoGroup, FeedHomeVideoState, LibEvent, PAGE_SIZE};
use mbv_core::api::MediaItem;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

impl App {
    pub(super) fn log_feed_home_video_state(&self, lib_idx: usize, context: &str) {
        let Some(lib) = self.libs.get(lib_idx) else {
            log::debug!(target: "feedhv", "{context}: lib_idx={lib_idx} missing");
            return;
        };
        let root = lib.nav_stack.first();
        let feed = lib.feed_home_video.as_ref();
        log::debug!(
            target: "feedhv",
            "{context}: lib_idx={lib_idx} lib={} nav_len={} root_parent={} root_items={} root_loading={} root_cursor={} feed_present={} feed_loading={} selected_group={} groups={} all_items={} video_cursor={} video_scroll={} group_view={}",
            lib.library.name,
            lib.nav_stack.len(),
            root.map(|lvl| lvl.parent_id.as_str()).unwrap_or(""),
            root.map(|lvl| lvl.items.len()).unwrap_or(0),
            root.map(|lvl| lvl.loading).unwrap_or(false),
            root.map(|lvl| lvl.cursor).unwrap_or(0),
            feed.is_some(),
            feed.map(|state| state.loading).unwrap_or(false),
            feed.map(|state| state.selected_group).unwrap_or(0),
            feed.map(|state| state.groups.len()).unwrap_or(0),
            feed.map(|state| state.all_items.len()).unwrap_or(0),
            feed.map(|state| state.video_cursor).unwrap_or(0),
            feed.map(|state| state.video_scroll).unwrap_or(0),
            self.is_feed_home_video_group_view(lib_idx),
        );
    }

    fn feed_home_video_visible_group_count(&self, lib_idx: usize) -> usize {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.groups.len())
            .unwrap_or(0)
    }

    pub(super) fn feed_home_video_selected_group_index(&self, lib_idx: usize) -> usize {
        self.libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
            .map(|state| state.selected_group_index())
            .unwrap_or(0)
    }

    pub(super) fn feed_home_video_selected_items(&self, lib_idx: usize) -> Vec<MediaItem> {
        let Some(state) = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())
        else {
            return Vec::new();
        };
        let selected_group = state.selected_group_index();
        if selected_group == 0 {
            state.all_items.clone()
        } else {
            state
                .groups
                .get(selected_group - 1)
                .map(|group| group.items.clone())
                .unwrap_or_default()
        }
    }

    pub(super) fn feed_home_video_selected_parent_id(&self, lib_idx: usize) -> Option<String> {
        let lib = self.libs.get(lib_idx)?;
        let root = lib.nav_stack.first()?;
        let state = lib.feed_home_video.as_ref()?;
        let selected_group = state.selected_group_index();
        if selected_group == 0 {
            Some(root.parent_id.clone())
        } else {
            state
                .groups
                .get(selected_group - 1)
                .map(|group| group.folder.id.clone())
        }
    }

    /// Returns the item currently under the cursor without cloning the whole
    /// selected-group item list (see `feed_home_video_selected_items`, which
    /// does clone the full list and remains the right choice for callers that
    /// actually need it).
    pub(super) fn selected_feed_home_video_item(&self, lib_idx: usize) -> Option<MediaItem> {
        let state = self
            .libs
            .get(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_ref())?;
        let idx = state
            .video_cursor
            .min(state.selected_len().saturating_sub(1));
        let group = state.selected_group_index();
        if group == 0 {
            state.all_items.get(idx).cloned()
        } else {
            state
                .groups
                .get(group - 1)
                .and_then(|g| g.items.get(idx))
                .cloned()
        }
    }

    pub(super) fn clamp_feed_home_video_state(&mut self, lib_idx: usize) {
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        state.selected_group = state.selected_group_index();
        let items_len = state.selected_len();
        if items_len == 0 {
            state.video_cursor = 0;
            state.video_scroll = 0;
        } else {
            state.video_cursor = state.video_cursor.min(items_len.saturating_sub(1));
            state.video_scroll = state.video_scroll.min(state.video_cursor);
        }
    }

    pub(super) fn remove_item_from_feed_home_video_cache(&mut self, lib_idx: usize, item_id: &str) {
        let Some(state) = self
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.feed_home_video.as_mut())
        else {
            return;
        };
        state.all_items.retain(|item| item.id != item_id);
        for group in &mut state.groups {
            group.items.retain(|item| item.id != item_id);
        }
        state.groups.retain(|group| !group.items.is_empty());
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "remove_from_cache");
    }

    pub(super) fn ensure_feed_home_video_group_level(&mut self, lib_idx: usize) {
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
        if !ready || !(self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx))
        {
            return;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "ensure_group_level");
    }

    /// Common guard for kicking off `spawn_feed_home_video_aggregate` (or the
    /// podcast equivalent) once a grouped library's root folder listing has
    /// fully paginated: mbv is showing this library's tab, it's a
    /// feed-home-video or podcast library, and its root nav level has loaded
    /// every item. `extra_ok` carries the caller-specific condition (e.g.
    /// which event/level this check is reacting to).
    fn should_aggregate_feed(
        &self,
        lib_idx: usize,
        extra_ok: impl FnOnce(&BrowseLevel) -> bool,
    ) -> bool {
        self.library_tab == lib_idx + 1
            && (self.is_feed_home_video_library(lib_idx) || self.is_podcast_library(lib_idx))
            && self
                .libs
                .get(lib_idx)
                .map(|lib| {
                    lib.nav_stack.len() == 1
                        && lib.nav_stack[0].is_fully_loaded()
                        && extra_ok(&lib.nav_stack[0])
                })
                .unwrap_or(false)
    }

    fn spawn_feed_home_video_aggregate(&self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let Some(root) = lib.nav_stack.first() else {
            return;
        };
        if root.loading {
            return;
        }
        let parent_id = root.parent_id.clone();
        let candidate_folders = root.items.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let (mut all_items, total_count) = match client.get_items_sorted(
                &parent_id,
                Some("Video"),
                true,
                0,
                PAGE_SIZE,
                "DateCreated",
                "Ascending",
            ) {
                Ok(items) => items,
                Err(e) => {
                    let _ = tx.send(LibEvent::Error(e));
                    return;
                }
            };
            if total_count > all_items.len() {
                match client.get_items_sorted(
                    &parent_id,
                    Some("Video"),
                    true,
                    0,
                    total_count,
                    "DateCreated",
                    "Ascending",
                ) {
                    Ok((items, _)) => all_items = items,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                }
            }

            let folder_ids: HashSet<String> = candidate_folders
                .iter()
                .map(|folder| folder.id.clone())
                .collect();
            let mut grouped: HashMap<String, Vec<MediaItem>> = HashMap::new();
            for video in &all_items {
                if folder_ids.is_empty() {
                    break;
                }
                let ancestors = match client.get_ancestors(&video.id) {
                    Ok(ancestors) => ancestors,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                };
                if let Some(folder) = ancestors
                    .iter()
                    .find(|ancestor| folder_ids.contains(&ancestor.id))
                {
                    grouped
                        .entry(folder.id.clone())
                        .or_default()
                        .push(video.clone());
                }
            }

            let groups = candidate_folders
                .into_iter()
                .filter_map(|folder| {
                    let items = grouped.remove(&folder.id).unwrap_or_default();
                    if items.is_empty() {
                        None
                    } else {
                        Some(FeedHomeVideoGroup { folder, items })
                    }
                })
                .collect();
            let _ = tx.send(LibEvent::FeedHomeVideoAggregated {
                lib_idx,
                parent_id,
                all_items,
                groups,
            });
        });
    }

    pub(super) fn is_feed_home_video_group_view(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        let has_state = lib.feed_home_video.as_ref().is_some_and(|state| {
            state.loading || !state.groups.is_empty() || !state.all_items.is_empty()
        });
        if !has_state {
            return false;
        }
        // Podcast channels always use the group view.
        if self.is_podcast_library(lib_idx) {
            return true;
        }
        // Feed home-video libraries use the group view when configured.
        if lib.library.collection_type != "homevideos" {
            return false;
        }
        let client = self.client.lock().unwrap();
        client
            .config
            .feed_view_libraries
            .contains(&lib.library.name.to_lowercase())
            && lib
                .nav_stack
                .first()
                .is_some_and(|lvl| lvl.item_types.is_none())
    }

    pub(super) fn ensure_feed_home_video_root_loaded(&mut self, lib_idx: usize) {
        if !self.is_feed_home_video_library(lib_idx) {
            return;
        }
        let needs_reload = self
            .libs
            .get(lib_idx)
            .map(|lib| {
                lib.nav_stack.is_empty()
                    || (!lib.nav_stack[0].loading
                        && lib.nav_stack[0]
                            .items
                            .first()
                            .map(|item| !item.is_folder)
                            .unwrap_or(true))
            })
            .unwrap_or(false);
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
            parent_id: lib_id.clone(),
            title: lib_name.clone(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
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
        self.log_feed_home_video_state(lib_idx, "root_reload");
    }

    pub(crate) fn is_feed_home_video_library(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        if lib.library.collection_type != "homevideos" {
            return false;
        }
        let client = self.client.lock().unwrap();
        client
            .config
            .feed_view_libraries
            .contains(&lib.library.name.to_lowercase())
    }

    pub(crate) fn is_podcast_library(&self, lib_idx: usize) -> bool {
        let lib = &self.libs[lib_idx];
        lib.library.item_type == "Channel"
            || lib.library.collection_type == "podcasts"
            || lib.library.name.to_lowercase().contains("podcast")
    }

    /// Whether the currently focused library tab is a podcast channel.
    pub(super) fn is_in_podcast_library(&self) -> bool {
        let Some(lib_idx) = self.library_tab.checked_sub(1) else {
            return false;
        };
        lib_idx < self.libs.len() && self.is_podcast_library(lib_idx)
    }

    pub(super) fn ensure_podcast_root_loaded(&mut self, lib_idx: usize) {
        if !self.is_podcast_library(lib_idx) {
            return;
        }
        let needs_reload = self
            .libs
            .get(lib_idx)
            .map(|lib| {
                lib.nav_stack.is_empty()
                    || (!lib.nav_stack[0].loading
                        && lib.nav_stack[0]
                            .items
                            .first()
                            .map(|item| !item.is_folder)
                            .unwrap_or(true))
            })
            .unwrap_or(false);
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
            parent_id: lib_id.clone(),
            title: lib_name.clone(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
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
    }

    /// Fetch episodes for each podcast show folder, sorted newest-first.
    /// Much simpler than feed-home-video aggregation: episodes are direct
    /// children of each show folder, no ancestor lookups needed.
    fn spawn_podcast_aggregate(&self, lib_idx: usize) {
        if !self.is_podcast_library(lib_idx) {
            return;
        }
        let Some(lib) = self.libs.get(lib_idx) else {
            return;
        };
        let Some(root) = lib.nav_stack.first() else {
            return;
        };
        if root.loading {
            return;
        }
        let parent_id = root.parent_id.clone();
        let show_folders = root.items.clone();
        let client = self.client.lock().unwrap().clone();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let mut all_items: Vec<MediaItem> = Vec::new();
            let mut groups: Vec<FeedHomeVideoGroup> = Vec::new();
            for folder in show_folders {
                let episodes = match client.get_items_sorted(
                    &folder.id,
                    None,
                    false,
                    0,
                    10000, // fetch all episodes
                    "PremiereDate",
                    "Descending",
                ) {
                    Ok((items, _)) => items,
                    Err(e) => {
                        let _ = tx.send(LibEvent::Error(e));
                        return;
                    }
                };
                all_items.extend(episodes.clone());
                if !episodes.is_empty() {
                    groups.push(FeedHomeVideoGroup {
                        folder,
                        items: episodes,
                    });
                }
            }
            // Sort the combined "All" list newest-first by premiere_date.
            all_items.sort_by(|a, b| b.premiere_date.cmp(&a.premiere_date));
            let _ = tx.send(LibEvent::FeedHomeVideoAggregated {
                lib_idx,
                parent_id,
                all_items,
                groups,
            });
        });
    }

    pub(super) fn select_feed_folder_group(&mut self, lib_idx: usize, group_idx: usize) {
        if self.libs[lib_idx].nav_stack.is_empty() {
            return;
        }
        let n = self.feed_home_video_visible_group_count(lib_idx);
        if group_idx > n {
            return;
        }
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            state.selected_group = group_idx;
            state.video_cursor = 0;
            state.video_scroll = 0;
        }
        self.clamp_feed_home_video_state(lib_idx);
        self.log_feed_home_video_state(lib_idx, "select_group");
    }

    pub(super) fn switch_feed_folder_group(&mut self, lib_idx: usize, delta: i64) {
        let n = self.feed_home_video_visible_group_count(lib_idx) + 1;
        if n == 0 {
            return;
        }
        let cur = self.feed_home_video_selected_group_index(lib_idx);
        let next = (cur as i64 + delta).rem_euclid(n as i64) as usize;
        self.select_feed_folder_group(lib_idx, next);
    }

    pub(super) fn maybe_aggregate_feed_after_loaded(&self, lib_idx: usize) {
        let should_aggregate_feed = self.should_aggregate_feed(lib_idx, |root| {
            root.item_types.is_none() && !root.unplayed_only
        });
        if should_aggregate_feed {
            self.log_feed_home_video_state(lib_idx, "loaded_before_aggregate");
            self.spawn_feed_home_video_aggregate(lib_idx);
            self.spawn_podcast_aggregate(lib_idx);
        }
    }

    pub(super) fn maybe_aggregate_feed_after_page_append(&self, lib_idx: usize, parent_id: &str) {
        let should_aggregate_feed =
            self.should_aggregate_feed(lib_idx, |root| root.parent_id == parent_id);
        if should_aggregate_feed {
            self.log_feed_home_video_state(lib_idx, "page_appended_before_aggregate");
            self.spawn_feed_home_video_aggregate(lib_idx);
            self.spawn_podcast_aggregate(lib_idx);
        }
    }

    pub(super) fn maybe_refresh_feed_groups_after_refresh(&mut self, lib_idx: usize) {
        let should_refresh_feed_groups = self
            .libs
            .get(lib_idx)
            .map(|lib| {
                self.library_tab == lib_idx + 1
                    && (self.is_feed_home_video_library(lib_idx)
                        || self.is_podcast_library(lib_idx))
                    && lib
                        .nav_stack
                        .first()
                        .is_some_and(BrowseLevel::is_fully_loaded)
            })
            .unwrap_or(false);
        if should_refresh_feed_groups {
            if let Some(lib) = self.libs.get_mut(lib_idx) {
                let state = lib
                    .feed_home_video
                    .get_or_insert_with(FeedHomeVideoState::default);
                state.loading = true;
            }
            self.log_feed_home_video_state(lib_idx, "refreshed_before_aggregate");
            self.spawn_feed_home_video_aggregate(lib_idx);
            self.spawn_podcast_aggregate(lib_idx);
        }
    }

    /// Spawn a background thread to fetch and parse the idle RSS feed.
    pub(super) fn spawn_idle_feed_fetch(&self) {
        let Some(ref idle_feed) = self.idle_feed else {
            return;
        };
        let rss_url = self.client.lock().unwrap().config.idle_feed_rss_url.clone();
        let tx = idle_feed.items_tx.clone();
        std::thread::spawn(move || {
            let items = match fetch_and_parse_rss(&rss_url) {
                Ok(items) => items,
                Err(e) => {
                    log::warn!(target: "idle_feed", "Failed to fetch RSS feed: {e}");
                    Vec::new()
                }
            };
            let _ = tx.send(items);
        });
    }

    pub(super) fn idle_feed_link_available(&self) -> bool {
        self.idle_feed
            .as_ref()
            .and_then(|feed| feed.items.get(feed.current_index))
            .and_then(|item| item.link.as_deref())
            .is_some_and(|link| !link.is_empty())
    }

    /// Open the currently displayed idle-feed item in the user's browser.
    pub(super) fn open_idle_feed_link(&mut self) {
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

        if let Err(error) = open_url(&link) {
            log::warn!(target: "idle_feed", "Failed to open feed link {link:?}: {error}");
            self.flash_status(format!("Unable to open feed link: {error}"));
        }
    }

    /// Advance the idle feed rotation if enough time has elapsed.
    pub(super) fn advance_idle_feed_rotation(&mut self) {
        let Some(ref mut idle_feed) = self.idle_feed else {
            return;
        };
        if idle_feed.items.is_empty() {
            return;
        }
        let rotation_secs = self.client.lock().unwrap().config.idle_feed_rotation_secs;
        if idle_feed.last_rotation.elapsed() >= std::time::Duration::from_secs(rotation_secs) {
            idle_feed.current_index = (idle_feed.current_index + 1) % idle_feed.items.len();
            idle_feed.last_rotation = Instant::now();
        }
    }
}

fn open_url(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map(|_| ())
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map(|_| ())
    }
}
