use super::notify_actions::ToastSeverity;
use super::{App, ContextAction, PanelFocus};

impl App {
    pub(super) fn execute_context_action(&mut self, action: Option<ContextAction>) {
        self.context_menu = None;
        self.layout.context_menu_rect = None;
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
}
