use super::notify_actions::ToastSeverity;
use super::ui_util::natural_sort_key;
use super::{App, PanelFocus};
use mbv_core::api::EmbyItem;
use rand::seq::SliceRandom;

impl App {
    pub(super) fn shuffle_play(&mut self, lib_idx: usize) {
        // Defensive bounds check: the dispatch front door normalizes a stale
        // destination first, but async Service removal can invalidate the
        // matched index between normalization and this call. No-op (never
        // substitute library zero) on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }
        // Resolve the folder target at the selected browse item, delegating
        // the current-parent fallback to the shared tail below. Cloning this
        // resolution into a `shuffle_play_target` call keeps the legacy
        // `shuffle_play(lib_idx)` behaviour byte-for-byte identical while
        // letting the component-sourced browser Ctrl+S path reuse the same
        // explicit-target tail without this cursor read.
        let parent_id = {
            let lib = &self.libs[lib_idx];
            let item = lib
                .nav_stack
                .last()
                .and_then(|lvl| lvl.items.get(lvl.cursor));
            item.filter(|i| i.is_folder).map(|i| i.id.clone())
        };
        self.shuffle_play_target(lib_idx, parent_id);
    }

    /// Shuffle from the generic Emby browser's component-resolved selected
    /// item (task 5.3d, Emby browser shuffle decoupling). `BrowserComponent`
    /// resolved the item at its component-local cursor; when that item is a
    /// folder the folder itself is shuffled, otherwise the current browse
    /// level's parent is shuffled (falling back to the library id exactly as
    /// `shuffle_play` did). The folder target comes from the supplied item,
    /// never from re-reading `BrowseLevel.cursor`.
    pub(super) fn shuffle_play_selected(&mut self, lib_idx: usize, item: EmbyItem) {
        let explicit_folder = item.is_folder.then_some(item.id.clone());
        self.shuffle_play_target(lib_idx, explicit_folder);
    }

    /// Shared explicit-target tail for both shuffle entry points. `explicit_folder`
    /// is the folder id to shuffle when the caller already knows the selected
    /// item is a folder (the typed browser path supplies it from the
    /// component-resolved item); when `None`, the current browse level's
    /// parent is shuffled, falling back to the library id exactly as the
    /// legacy `shuffle_play` did.
    fn shuffle_play_target(&mut self, lib_idx: usize, explicit_folder: Option<String>) {
        // Defensive bounds check (see `shuffle_play` comment; the shell also
        // derives a fresh `lib_idx`, so a synchronous tab change can race in).
        if lib_idx >= self.libs.len() {
            return;
        }
        let parent_id = match explicit_folder {
            Some(id) => id,
            None => {
                let lib = &self.libs[lib_idx];
                lib.nav_stack
                    .last()
                    .map(|l| l.parent_id.clone())
                    .unwrap_or_else(|| lib.library.id.clone())
            }
        };
        // Delegate to the same fetch the context menu's Shuffle action uses
        // (`ContextAction::ShuffleFolder` -> `shuffle_folder`), rather than
        // duplicating this logic against `get_all_videos_recursive`, which
        // only requests Episode/Movie/Video types and so silently excludes
        // Audio -- Ctrl+S on a music album (all-Audio contents) always
        // fetched zero items and reported "Nothing to shuffle" even though
        // the album had playable tracks, while the context menu (already on
        // `get_all_playable_recursive`, which includes Audio) worked fine.
        self.shuffle_folder(lib_idx, &parent_id);
    }

    pub(super) fn play_folder(&mut self, folder_id: &str) {
        let Some(client) = self.emby_client() else {
            self.flash(
                "Emby is unavailable".into(),
                super::notify_actions::ToastSeverity::Warning,
            );
            return;
        };
        let client = client.lock().unwrap();
        match client.get_all_playable_recursive(folder_id) {
            Ok(mut items) => {
                items.retain(|i| !i.is_folder);
                items.sort_by_key(|a| natural_sort_key(a.sort_key()));
                if items.is_empty() {
                    drop(client);
                    self.flash("Nothing to play".into(), ToastSeverity::Error);
                    return;
                }
                drop(client);
                self.replace_playback_queue(items.clone(), 0);
                self.set_panel_focus(PanelFocus::Queue);
                self.play_items_routed(items, 0, crate::config::QueueSource::Unknown);
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't load folder: {e}"), ToastSeverity::Error);
            }
        }
    }

    pub(crate) fn is_tvshows_library(&self, lib_idx: usize) -> bool {
        self.libs[lib_idx].library.collection_type == "tvshows"
    }

    /// Whether the given Emby library is a tvshows library. The index
    /// arrives explicitly from the shuffle chain (`shuffle_play` /
    /// `execute_context_action` pass the library the folder was reached
    /// through), so this no longer reads the selected tab. Bounds-misses
    /// return false (defensive; never substitute library zero).
    pub(super) fn active_lib_is_tvshows(&self, lib_idx: usize) -> bool {
        lib_idx < self.libs.len() && self.is_tvshows_library(lib_idx)
    }

    pub(super) fn shuffle_folder(&mut self, lib_idx: usize, folder_id: &str) {
        // TV libraries shuffle from a video-only fetch (Episode/Movie/Video)
        // so a season/series shuffle can't pull in stray Audio items (e.g.
        // theme songs); every other library type keeps the broader
        // playable-items fetch used for enqueue/play-all, which does
        // include Audio (needed for music libraries -- see the bug this
        // replaced).
        let is_tvshows = self.active_lib_is_tvshows(lib_idx);
        let Some(client) = self.emby_client() else {
            self.flash(
                "Emby is unavailable".into(),
                super::notify_actions::ToastSeverity::Warning,
            );
            return;
        };
        let client = client.lock().unwrap();
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
                drop(client);
                self.replace_playback_queue(items.clone(), 0);
                self.set_panel_focus(PanelFocus::Queue);
                self.queue_source = crate::config::QueueSource::Shuffle;
                if !self.has_direct_remote_queue() {
                    self.save_queue_state();
                }
                self.play_items_routed(items, 0, crate::config::QueueSource::Shuffle);
            }
            Err(e) => {
                drop(client);
                self.flash(format!("Couldn't load folder: {e}"), ToastSeverity::Error);
            }
        }
    }
}
