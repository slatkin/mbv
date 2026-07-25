use super::ui_util::{is_playable, sort_audio_tracks};
use super::{App, ArtistHeaderSelection, PanelFocus};
use mbv_core::api::MediaItem;
use rand::seq::SliceRandom;

impl App {
    pub(super) fn power_artist_header_action_lib_idx(&self) -> Option<usize> {
        if matches!(self.panel_focus, PanelFocus::Library) && self.library_tab > 0 {
            Some(self.library_tab - 1)
        } else {
            None
        }
    }

    fn selected_artist_header_action(&mut self) -> Option<(usize, ArtistHeaderSelection)> {
        let lib_idx = self.power_artist_header_action_lib_idx()?;
        self.selected_artist_header_album_items(lib_idx)
            .map(|(selection, _)| (lib_idx, selection))
    }

    fn resolve_artist_header_playable_items(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
    ) -> Result<Vec<MediaItem>, String> {
        let albums = self
            .artist_header_album_items_for_selection(lib_idx, selection)
            .unwrap_or_default();
        let client = self.client.lock().unwrap();
        let mut resolved = Vec::new();
        for album in albums {
            let mut items = client.get_all_playable_recursive(&album.id)?;
            items.retain(|item| !item.is_folder && is_playable(item));
            sort_audio_tracks(&mut items);
            resolved.extend(items);
        }
        Ok(resolved)
    }

    pub(super) fn enqueue_artist_header_selection(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
    ) -> bool {
        log::info!(target: "library_route", "user action=enqueue item_id={:?} item_name={:?} source=artist-header", selection.first_album_id, selection.artist_label);
        if self.in_non_library_thin_client_mode() {
            log::info!(target: "library_route", "route bypass action=enqueue item_id={:?} item_name={:?} source=artist-header reason=non-library thin-client owns playback", selection.first_album_id, selection.artist_label);
        }
        let resolved = self.route_for_active_library_view(lib_idx).map(|(n, _)| n);
        if self.enqueue_route_conflict(resolved) {
            return true;
        }
        let items = match self.resolve_artist_header_playable_items(lib_idx, selection) {
            Ok(items) => items,
            Err(e) => {
                self.flash_status_high(format!("Error: {e}"));
                return true;
            }
        };
        let count = items.len();
        if count == 0 {
            self.flash_status_high("Nothing to enqueue".into());
            return true;
        }

        let scope = self.visible_queue_scope();
        let appended = items.clone();
        let previous_dirty = self.queue_dirty;
        let previous_queue = self.queue_for_scope(scope).clone();
        {
            let queue = self.queue_for_scope_mut(scope);
            queue.append_items(items);
        }
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        self.flash_status(format!(
            "Enqueued {count} items from {}",
            selection.artist_label
        ));
        if self.sync_playback_queue_after_append(scope, appended) {
            self.persist_local_queue_state_if_needed(scope);
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
        true
    }

    pub(super) fn enqueue_selected_artist_header(&mut self) -> bool {
        let Some((lib_idx, selection)) = self.selected_artist_header_action() else {
            return false;
        };
        self.enqueue_artist_header_selection(lib_idx, &selection)
    }

    pub(super) fn play_artist_header_selection(
        &mut self,
        lib_idx: usize,
        selection: &ArtistHeaderSelection,
        shuffle: bool,
    ) -> bool {
        let mut items = match self.resolve_artist_header_playable_items(lib_idx, selection) {
            Ok(items) => items,
            Err(e) => {
                self.flash_status_high(format!("Error: {e}"));
                return true;
            }
        };
        let count = items.len();
        if count == 0 {
            self.flash_status_high(if shuffle {
                "Nothing to shuffle".into()
            } else {
                "Nothing to play".into()
            });
            return true;
        }
        if shuffle {
            items.shuffle(&mut rand::rng());
        }
        self.replace_playback_queue(items.clone(), 0);
        self.set_panel_focus(PanelFocus::Queue);
        self.flash_status(if shuffle {
            format!("Shuffling {count} items")
        } else {
            format!("Playing {count} items")
        });
        self.queue_source = if shuffle {
            crate::config::QueueSource::Shuffle
        } else {
            crate::config::QueueSource::Collection {
                collection_type: self.libs[lib_idx].library.collection_type.clone(),
            }
        };
        if !self.has_direct_remote_queue() {
            self.save_queue_state();
        }
        self.play_items_routed(items, 0);
        true
    }

    pub(super) fn play_selected_artist_header(&mut self, shuffle: bool) -> bool {
        let Some((lib_idx, selection)) = self.selected_artist_header_action() else {
            return false;
        };
        self.play_artist_header_selection(lib_idx, &selection, shuffle)
    }
}
