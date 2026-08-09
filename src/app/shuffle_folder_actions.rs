use super::notify_actions::ToastSeverity;
use super::ui_util::natural_sort_key;
use super::{App, PanelFocus};
use rand::seq::SliceRandom;

impl App {
    pub(super) fn shuffle_play(&mut self) {
        if self.tab.is_home() {
            return;
        }
        if self.play_selected_artist_header(true) {
            return;
        }
        let lib_idx = self.tab.library_index().unwrap();
        let parent_id = {
            let lib = &self.libs[lib_idx];
            let item = lib
                .nav_stack
                .last()
                .and_then(|lvl| lvl.items.get(lvl.cursor));
            item.filter(|i| i.is_folder)
                .map(|i| i.id.clone())
                .or_else(|| lib.nav_stack.last().map(|l| l.parent_id.clone()))
                .unwrap_or_else(|| lib.library.id.clone())
        };
        // Delegate to the same fetch the context menu's Shuffle action uses
        // (`ContextAction::ShuffleFolder` -> `shuffle_folder`), rather than
        // duplicating this logic against `get_all_videos_recursive`, which
        // only requests Episode/Movie/Video types and so silently excludes
        // Audio -- Ctrl+S on a music album (all-Audio contents) always
        // fetched zero items and reported "Nothing to shuffle" even though
        // the album had playable tracks, while the context menu (already on
        // `get_all_playable_recursive`, which includes Audio) worked fine.
        self.shuffle_folder(&parent_id);
    }

    pub(super) fn play_folder(&mut self, folder_id: &str) {
        let client = self.client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                if items.is_empty() {
                    drop(client);
                    self.flash("Nothing to play".into(), ToastSeverity::Error);
                    return;
                }
                let count = items.len();
                drop(client);
                self.replace_playback_queue(items.clone(), 0);
                self.set_panel_focus(PanelFocus::Queue);
                self.flash(format!("Playing {count} items"), ToastSeverity::Success);
                self.play_items_routed(items, 0);
            }
            Err(e) => {
                drop(client);
                self.flash_error(e);
            }
        }
    }

    pub(crate) fn is_tvshows_library(&self, lib_idx: usize) -> bool {
        self.libs[lib_idx].library.collection_type == "tvshows"
    }

    /// Whether the currently focused library tab is a tvshows library.
    /// Same bounds-check-then-delegate shape as `is_in_podcast_library`.
    ///
    /// Caveat: this reads the *active tab*, not the folder actually being
    /// shuffled -- `shuffle_folder`'s `folder_id` argument is not consulted
    /// here. That's fine for its two current callers (`shuffle_play`, only
    /// reachable once the left panel is already on a library tab; and the
    /// context menu's Shuffle action, only offered for a folder while
    /// browsing a library tab), but it would silently pick the wrong fetch
    /// for a folder reached some other way -- e.g. a future caller
    /// shuffling a folder surfaced by the global search overlay, or a
    /// Home-tab aggregate (Continue Watching/Latest), while a *different*
    /// library tab happens to be focused underneath. A robust fix for that
    /// case would resolve `folder_id`'s owning library via
    /// `get_ancestors`, the way `route_for_item_via_ancestors` in
    /// `library_route.rs` already does for the analogous "which library
    /// actually owns this item" problem in route resolution.
    pub(super) fn active_lib_is_tvshows(&self) -> bool {
        let Some(lib_idx) = self.tab.library_index() else {
            return false;
        };
        lib_idx < self.libs.len() && self.is_tvshows_library(lib_idx)
    }

    pub(super) fn shuffle_folder(&mut self, folder_id: &str) {
        // TV libraries shuffle from a video-only fetch (Episode/Movie/Video)
        // so a season/series shuffle can't pull in stray Audio items (e.g.
        // theme songs); every other library type keeps the broader
        // playable-items fetch used for enqueue/play-all, which does
        // include Audio (needed for music libraries -- see the bug this
        // replaced).
        let is_tvshows = self.active_lib_is_tvshows();
        let client = self.client.lock().unwrap();
        let fetch = if is_tvshows {
            client.get_all_videos_recursive(folder_id)
        } else {
            client.get_all_playable_recursive(folder_id)
        };
        match fetch {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                if items.is_empty() {
                    drop(client);
                    self.flash("Nothing to shuffle".into(), ToastSeverity::Error);
                    return;
                }
                items.shuffle(&mut rand::rng());
                let count = items.len();
                drop(client);
                self.replace_playback_queue(items.clone(), 0);
                self.set_panel_focus(PanelFocus::Queue);
                self.flash(format!("Shuffling {count} items"), ToastSeverity::Success);
                self.queue_source = crate::config::QueueSource::Shuffle;
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
                self.play_items_routed(items, 0);
            }
            Err(e) => {
                drop(client);
                self.flash_error(e);
            }
        }
    }
}
