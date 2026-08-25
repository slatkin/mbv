use super::notify_actions::ToastSeverity;
use super::types_context_menu::ContextMenu;
use super::types_overlay::OverlayRequest;
use super::{App, ContextAction, ContextMenuAnchor, ContextMenuEntry, PanelFocus};
use mbv_core::api::EmbyItem;

impl App {
    pub(super) fn execute_context_action(&mut self, action: Option<ContextAction>) {
        // The shell dismisses the mounted ContextMenu component; this only
        // dispatches the chosen action (task 5.3c).
        // The menu can only have opened on a matched Emby library, Home, or
        // the queue; `context_menu_lib_idx()` resolves the explicitly matched
        // Emby library (positive match, `None` on Home/queue) that every
        // Emby-only callee below must receive.
        let lib_idx = self.context_menu_lib_idx();
        match action {
            Some(ContextAction::Play) => {
                if matches!(self.effective_panel_focus(), PanelFocus::Library) && self.tab.is_home()
                {
                    self.cw_play();
                } else if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
                    // Was its own third copy of queue-cursor activation, with
                    // a subtly narrower `else` branch than the keyboard/mouse
                    // paths (no seek-to-start for an already-playing audio
                    // item) -- now the same seam as Enter on the queue tab
                    // and a queue-row double-click (see #134's follow-up).
                    self.dispatch(super::action::Command::QueuePlayCursor);
                } else if let Some(lib_idx) = lib_idx {
                    self.select(lib_idx);
                }
            }
            Some(ContextAction::PlayFolder(id)) => {
                let ct = if let Some(lib_idx) = lib_idx {
                    self.libs[lib_idx].library.collection_type.clone()
                } else {
                    String::new()
                };
                self.queue_source = crate::config::QueueSource::Collection {
                    collection_type: ct,
                };
                self.play_folder(&id);
                self.save_queue_state();
            }
            Some(ContextAction::ShuffleFolder(id)) => {
                if let Some(lib_idx) = lib_idx {
                    self.shuffle_folder(lib_idx, &id);
                }
            }
            Some(ContextAction::Enqueue) => {
                if matches!(self.effective_panel_focus(), PanelFocus::Library) && self.tab.is_home()
                {
                    self.cw_enqueue();
                } else {
                    self.enqueue_selected(lib_idx);
                }
            }
            Some(ContextAction::EnqueueFolder(item)) => self.do_enqueue_folder((*item).clone()),
            Some(ContextAction::MarkPlayed(id)) => self.context_set_played(&id, true, lib_idx),
            Some(ContextAction::MarkItemsPlayed(ids)) => {
                self.context_set_many_played(&ids, lib_idx)
            }
            Some(ContextAction::MarkUnplayed(id)) => self.context_set_played(&id, false, lib_idx),
            Some(ContextAction::MarkItemsUnplayed(ids)) => {
                self.context_set_many_unplayed(&ids, lib_idx)
            }
            Some(ContextAction::RemoveFromContinueWatching) => self.remove_from_continue_watching(),
            Some(ContextAction::RemoveFromQueue(pos)) => self.remove_from_queue(pos),
            Some(ContextAction::GoToLibrary(item_id, item_type)) => {
                let libs: Vec<(usize, String, String)> = self
                    .libs
                    .iter()
                    .enumerate()
                    .map(|(i, lib)| {
                        (
                            i,
                            lib.library.id.clone(),
                            lib.library.collection_type.clone(),
                        )
                    })
                    .collect();
                self.spawn_navigate_to_item(item_id, item_type, libs);
            }
            None => {}
        }
    }

    fn context_set_many_played(&mut self, item_ids: &[String], lib_idx: Option<usize>) {
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = item_ids
            .iter()
            .try_for_each(|item_id| client.mark_played(item_id));
        drop(client);
        match result {
            Ok(()) => {
                if let Some(lib_idx) = lib_idx {
                    self.refresh_lib(lib_idx);
                }
            }
            Err(e) => self.flash(
                format!("Couldn't mark items as played: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    fn context_set_many_unplayed(&mut self, item_ids: &[String], lib_idx: Option<usize>) {
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = item_ids
            .iter()
            .try_for_each(|item_id| client.mark_unplayed(item_id));
        drop(client);
        match result {
            Ok(()) => {
                if let Some(lib_idx) = lib_idx {
                    self.refresh_lib(lib_idx);
                }
            }
            Err(e) => self.flash(
                format!("Couldn't mark items as unplayed: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    fn context_set_played(&mut self, item_id: &str, played: bool, lib_idx: Option<usize>) {
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = if played {
            client.mark_played(item_id)
        } else {
            client.mark_unplayed(item_id)
        };
        drop(client);
        match result {
            Ok(()) => {
                if played {
                    // `lib_idx` is the explicitly matched Emby library from
                    // the action dispatch (`None` on Home/queue). If guard:
                    // no feed/video cleanup when there is no Emby library.
                    if let Some(lib_idx) = lib_idx {
                        if self.is_feed_home_video_group_view(lib_idx) {
                            if let Some(state) = self
                                .libs
                                .get_mut(lib_idx)
                                .and_then(|lib| lib.feed_home_video.as_mut())
                            {
                                state.loading = true;
                            }
                            self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
                            self.log_feed_home_video_state(lib_idx, "context_set_played_feed");
                        } else if let Some(lvl) = self
                            .libs
                            .get_mut(lib_idx)
                            .and_then(|l| l.nav_stack.last_mut())
                        {
                            if lvl.unplayed_only {
                                let id = item_id.to_string();
                                lvl.items.retain(|i| i.id != id);
                                lvl.total_count = lvl.total_count.saturating_sub(1);
                                lvl.cursor = lvl.cursor.min(lvl.items.len().saturating_sub(1));
                            }
                        }
                    }
                }
                if self.tab.is_home() {
                    let _ = self.fetch_home();
                } else if let Some(lib_idx) = lib_idx {
                    self.refresh_lib(lib_idx);
                }
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    pub(super) fn remove_from_continue_watching(&mut self) {
        let Some(item) = self
            .home
            .continue_items
            .get(self.home.continue_cursor)
            .cloned()
        else {
            return;
        };
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = client.hide_from_resume(&item.id);
        drop(client);
        match result {
            Ok(()) => {
                let _ = self.fetch_home();
            }
            Err(e) => self.flash(
                format!("Couldn't remove from continue watching: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    pub(super) fn toggle_watched_home(&mut self) {
        let Some(item) = self.current_home_item().and_then(|item| match item {
            mbv_core::playback_queue::QueueItem::Emby(item) => Some(*item),
            _ => None,
        }) else {
            return;
        };
        if item.is_folder || item.is_audio() {
            return;
        }
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = if item.played {
            client.mark_unplayed(&item.id)
        } else {
            client.mark_played(&item.id)
        };
        drop(client);
        match result {
            Ok(()) => {
                let _ = self.fetch_home();
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    pub(super) fn toggle_watched(&mut self, lib_idx: usize) {
        let Some(item) = self.current_lib_item(lib_idx) else {
            return;
        };
        if item.is_folder || item.is_audio() {
            return;
        }
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = if item.played {
            client.mark_unplayed(&item.id)
        } else {
            client.mark_played(&item.id)
        };
        drop(client);
        match result {
            Ok(()) => {
                if !item.played {
                    if self.is_feed_home_video_group_view(lib_idx) {
                        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                            state.loading = true;
                        }
                        self.remove_item_from_feed_home_video_cache(lib_idx, &item.id);
                        self.log_feed_home_video_state(lib_idx, "toggle_watched_feed");
                    } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                        if lvl.unplayed_only {
                            lvl.items.remove(lvl.cursor);
                            lvl.total_count = lvl.total_count.saturating_sub(1);
                            lvl.cursor = lvl.cursor.min(lvl.items.len().saturating_sub(1));
                        }
                    }
                }
                self.refresh_lib(lib_idx);
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    // --- Context menu framing (formerly `input_context_menu.rs`) -----------
    //
    // Builds the menu content and raises it through `pending_overlay`; the
    // shell mounts the `ContextMenuComponent` and owns placement (task 5.3c).
    // `App::context_menu` and `layout.context_menu_rect` are gone.

    fn push_context_action(
        entries: &mut Vec<ContextMenuEntry>,
        label: &'static str,
        action: ContextAction,
    ) {
        entries.push(ContextMenuEntry {
            label,
            action: Some(action),
        });
    }

    fn push_context_separator(entries: &mut Vec<ContextMenuEntry>) {
        entries.push(ContextMenuEntry {
            label: "────────",
            action: None,
        });
    }

    /// Build the context menu for the current panel/destination, or `None`
    /// when no menu applies or it would be empty.
    fn build_context_menu(&mut self) -> Option<ContextMenu> {
        self.build_context_menu_for(None)
    }

    /// `build_context_menu` with an explicitly resolved Emby-library item
    /// (task 5.3d, Album track focus): while an inline album track is
    /// focused, the shell resolves the track (the component owns the cursor)
    /// and passes it here so '.' targets the focused track instead of the
    /// album row. All other arms resolve exactly as `build_context_menu`.
    fn build_context_menu_for(&mut self, tracked_item: Option<EmbyItem>) -> Option<ContextMenu> {
        let mut entries: Vec<ContextMenuEntry> = vec![];

        let cw_focused = matches!(
            self.effective_panel_focus(),
            crate::app::PanelFocus::Library
        ) && self.tab.is_home();
        let lib_idx = self.context_menu_lib_idx();
        let in_podcast =
            lib_idx.is_some_and(|idx| self.is_podcast_library(idx)) || self.is_in_podcast_library();
        let podcast_bulk_ids = lib_idx.and_then(|idx| {
            if in_podcast && self.is_feed_home_video_group_view(idx) {
                Some((
                    self.podcast_mark_all_ids(idx),
                    self.podcast_mark_all_unplayed_ids(idx),
                ))
            } else {
                None
            }
        });
        // Exhaustive dispatch by panel and destination (design §5): a context
        // menu opens only for Home (library focus), an explicitly selected
        // Emby library, or an Emby queue item. Audiobookshelf and Feeds browse
        // rows, non-Emby queue items, and absent or stale targets produce no
        // Emby menu. `cw_focused` / `lib_idx` above drive the Emby-menu content
        // that follows; this match only resolves which target (if any) exists.
        let current_item = match (self.effective_panel_focus(), self.tab) {
            (crate::app::PanelFocus::Library, crate::app::TabSelection::Home) => self
                .home
                .continue_items
                .get(self.home.continue_cursor)
                .cloned(),
            (crate::app::PanelFocus::Library, crate::app::TabSelection::EmbyLibrary(lib_idx)) => {
                tracked_item.or_else(|| self.current_lib_item(lib_idx))
            }
            (
                crate::app::PanelFocus::Library,
                crate::app::TabSelection::AudiobookshelfLibrary(_),
            )
            | (crate::app::PanelFocus::Library, crate::app::TabSelection::Feeds) => return None,
            (crate::app::PanelFocus::Queue, _) => {
                let queue = self.displayed_queue();
                queue.clone_emby_item_at(queue.queue_cursor)
            }
        };

        if let Some(ref item) = current_item {
            if item.is_folder {
                Self::push_context_action(
                    &mut entries,
                    "Play All",
                    ContextAction::PlayFolder(item.id.clone()),
                );
                Self::push_context_action(
                    &mut entries,
                    "Shuffle",
                    ContextAction::ShuffleFolder(item.id.clone()),
                );
                Self::push_context_action(
                    &mut entries,
                    "Add to Queue",
                    ContextAction::EnqueueFolder(Box::new(item.clone())),
                );
                let (played_label, unplayed_label) = if in_podcast {
                    ("Mark Played", "Mark Unplayed")
                } else {
                    ("Mark Watched", "Mark Unwatched")
                };
                if self.context_menu_play_state(item) {
                    Self::push_context_action(
                        &mut entries,
                        unplayed_label,
                        ContextAction::MarkUnplayed(item.id.clone()),
                    );
                } else {
                    Self::push_context_action(
                        &mut entries,
                        played_label,
                        ContextAction::MarkPlayed(item.id.clone()),
                    );
                }
            } else {
                Self::push_context_action(&mut entries, "Play", ContextAction::Play);
                if cw_focused
                    || lib_idx.is_some()
                    || !matches!(self.effective_panel_focus(), crate::app::PanelFocus::Queue)
                {
                    Self::push_context_action(&mut entries, "Add to Queue", ContextAction::Enqueue);
                }
                // Audio items (music tracks) don't get mark-played, but podcast
                // episodes (Audio inside a Channel library) do.
                let is_music_audio =
                    (item.media_type == "Audio" || item.item_type == "Audio") && !in_podcast;
                if !is_music_audio {
                    let (played_label, unplayed_label) = if in_podcast {
                        ("Mark Played", "Mark Unplayed")
                    } else {
                        ("Mark Watched", "Mark Unwatched")
                    };
                    if self.context_menu_play_state(item) {
                        Self::push_context_action(
                            &mut entries,
                            unplayed_label,
                            ContextAction::MarkUnplayed(item.id.clone()),
                        );
                    } else {
                        Self::push_context_action(
                            &mut entries,
                            played_label,
                            ContextAction::MarkPlayed(item.id.clone()),
                        );
                    }
                }
                if cw_focused || (self.tab.is_home() && self.home.section == 0) {
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Continue Watching",
                        ContextAction::RemoveFromContinueWatching,
                    );
                }
                if !cw_focused
                    && matches!(self.effective_panel_focus(), crate::app::PanelFocus::Queue)
                {
                    let pos = self.displayed_queue().queue_cursor;
                    Self::push_context_action(
                        &mut entries,
                        "Remove from Queue",
                        ContextAction::RemoveFromQueue(pos),
                    );
                }
                if matches!(self.effective_panel_focus(), crate::app::PanelFocus::Queue) {
                    Self::push_context_action(
                        &mut entries,
                        "Go to Library",
                        ContextAction::GoToLibrary(item.id.clone(), item.item_type.clone()),
                    );
                }
            }
        }

        if let Some((played_ids, unplayed_ids)) = podcast_bulk_ids {
            if !played_ids.is_empty() || !unplayed_ids.is_empty() {
                Self::push_context_separator(&mut entries);
                Self::push_context_action(
                    &mut entries,
                    "Mark All Played",
                    ContextAction::MarkItemsPlayed(played_ids),
                );
                Self::push_context_action(
                    &mut entries,
                    "Mark All Unplayed",
                    ContextAction::MarkItemsUnplayed(unplayed_ids),
                );
            }
        }

        if entries.iter().all(|entry| entry.action.is_none()) {
            return None;
        }

        let anchor = match self.effective_panel_focus() {
            crate::app::PanelFocus::Library => {
                ContextMenuAnchor::SelectedItem(crate::app::PanelFocus::Library)
            }
            crate::app::PanelFocus::Queue => {
                ContextMenuAnchor::SelectedItem(crate::app::PanelFocus::Queue)
            }
        };
        Some(ContextMenu {
            anchor,
            cursor: ContextMenu::first_selectable(&entries),
            entries,
        })
    }

    pub(super) fn open_context_menu(&mut self) {
        if let Some(menu) = self.build_context_menu() {
            self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
        }
    }

    /// Open the context menu targeted at an explicitly resolved item (a
    /// focused inline album track reached through the shell boundary).
    pub(super) fn open_context_menu_for(&mut self, item: EmbyItem) {
        if let Some(menu) = self.build_context_menu_for(Some(item)) {
            self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
        }
    }

    pub(super) fn open_context_menu_at(&mut self, x: u16, y: u16) {
        let Some(mut menu) = self.build_context_menu() else {
            return;
        };
        menu.anchor = ContextMenuAnchor::Pointer { x, y };
        self.pending_overlay = Some(OverlayRequest::ContextMenu(menu));
    }
}
