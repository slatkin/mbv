use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::types::context_menu::BulkRemoveTarget;
use crate::app::{
    App, ContextAction, LibEvent, PanelFocus, PendingQueueAction, ReplacementExecutor,
    RoutedReplacementPrep,
};
use mbv_core::api::EmbyItem;
use rand::seq::SliceRandom;

impl App {
    pub(in crate::app) fn execute_context_action(
        &mut self,
        action: Option<ContextAction>,
        cw_item: Option<EmbyItem>,
    ) {
        // The shell dismisses the mounted ContextMenu component; this only
        // dispatches the chosen action (task 5.3c).
        let lib_idx = self.context_menu_lib_idx();
        let action = self.execute_context_selection_action(action, lib_idx);
        let action = self.execute_context_library_action(action, cw_item, lib_idx);
        let action = self.execute_context_queue_and_feed_action(action);
        self.execute_context_navigation_action(action);
    }

    fn execute_context_selection_action(
        &mut self,
        action: Option<ContextAction>,
        lib_idx: Option<usize>,
    ) -> Option<ContextAction> {
        match action {
            Some(ContextAction::PlaySelection(items)) => {
                // Queue rebuild is deferred to the gated confirmed path, so
                // cancelling leaves the queue untouched (design D4).
                self.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx: 0,
                        source: crate::config::QueueSource::Unknown,
                        autostart: true,
                    },
                    ReplacementExecutor::Routed(RoutedReplacementPrep::Selection),
                );
                None
            }
            Some(ContextAction::ShuffleSelection(mut items)) => {
                items.shuffle(&mut rand::rng());
                self.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx: 0,
                        source: crate::config::QueueSource::Shuffle,
                        autostart: true,
                    },
                    ReplacementExecutor::Routed(RoutedReplacementPrep::Selection),
                );
                None
            }
            Some(ContextAction::EnqueueSelection(items)) => {
                if let Some(lib_idx) = lib_idx {
                    for item in items {
                        self.enqueue_lib_item(lib_idx, item);
                    }
                } else {
                    for item in items
                        .into_iter()
                        .filter(|item| !item.is_folder && crate::app::ui_util::is_playable(item))
                    {
                        self.submit_queue_item(
                            mbv_core::playback_queue::QueueItem::Emby(Box::new(item)),
                            false,
                        );
                    }
                }
                None
            }
            Some(ContextAction::RemoveSelection(targets)) => {
                // Queue targets are batched; Continue Watching targets are
                // independent Service writes.
                let mut queue_slot_ids = Vec::new();
                for target in targets {
                    match target {
                        BulkRemoveTarget::ContinueWatching(item) => {
                            self.remove_from_continue_watching(&item);
                        }
                        BulkRemoveTarget::Queue(slot_id) => queue_slot_ids.push(slot_id),
                    }
                }
                if !queue_slot_ids.is_empty() {
                    let scope = self.viewed_queue_scope();
                    self.remove_slots_from_queue(scope, &queue_slot_ids);
                }
                None
            }
            Some(ContextAction::MarkPlayedSelection(ids)) => {
                for id in ids {
                    self.context_set_played(&id, true, lib_idx);
                }
                None
            }
            Some(ContextAction::MarkUnplayedSelection(ids)) => {
                for id in ids {
                    self.context_set_played(&id, false, lib_idx);
                }
                None
            }
            action => action,
        }
    }

    fn execute_context_library_action(
        &mut self,
        action: Option<ContextAction>,
        cw_item: Option<EmbyItem>,
        lib_idx: Option<usize>,
    ) -> Option<ContextAction> {
        match action {
            Some(ContextAction::Play) => {
                self.execute_play_action(cw_item, lib_idx);
                None
            }
            Some(ContextAction::PlayFolder(id)) => {
                let collection_type = lib_idx
                    .map(|index| self.libs[index].library.collection_type.clone())
                    .unwrap_or_default();
                // Replacement and its save remain gated by confirmation (D4).
                self.play_folder(&id, collection_type);
                None
            }
            Some(ContextAction::ShuffleFolder(id)) => {
                if let Some(lib_idx) = lib_idx {
                    self.shuffle_folder(lib_idx, &id);
                }
                None
            }
            Some(ContextAction::Enqueue) => {
                if matches!(self.effective_panel_focus(), PanelFocus::Library) && self.tab.is_home()
                {
                    if let Some(item) = cw_item {
                        self.cw_enqueue(item);
                    }
                } else if let Some(lib_idx) = lib_idx {
                    let cursor = self
                        .libs
                        .get(lib_idx)
                        .and_then(|lib| lib.nav_stack.last())
                        .map_or(0, |level| level.resting().cursor());
                    if let Some(item) = self.current_lib_item(lib_idx, cursor) {
                        self.enqueue_lib_item(lib_idx, item);
                    }
                }
                None
            }
            Some(ContextAction::EnqueueFolder(item)) => {
                self.do_enqueue_folder(&item);
                None
            }
            Some(ContextAction::MarkPlayed(id)) => {
                self.context_set_played(&id, true, lib_idx);
                None
            }
            Some(ContextAction::MarkUnplayed(id)) => {
                self.context_set_played(&id, false, lib_idx);
                None
            }
            Some(ContextAction::RemoveFromContinueWatching) => {
                if let Some(item) = cw_item {
                    self.remove_from_continue_watching(&item);
                }
                None
            }
            action => action,
        }
    }

    fn execute_context_queue_and_feed_action(
        &mut self,
        action: Option<ContextAction>,
    ) -> Option<ContextAction> {
        match action {
            Some(ContextAction::PlayQueue(index)) => {
                self.dispatch(&crate::app::dispatch::action::Command::QueuePlayCursor(
                    index,
                ));
                None
            }
            Some(ContextAction::RemoveFromQueue(pos)) => {
                self.remove_from_queue(pos);
                None
            }
            Some(ContextAction::FeedsPlay(entries)) => {
                self.play_feed_entries(entries);
                None
            }
            Some(ContextAction::FeedsEnqueue(entries)) => {
                self.enqueue_feed_entries(entries);
                None
            }
            Some(ContextAction::FeedsMarkPlayed(entries)) => {
                self.set_feed_entries_played(&entries, true);
                None
            }
            Some(ContextAction::FeedsMarkUnplayed(entries)) => {
                self.set_feed_entries_played(&entries, false);
                None
            }
            action => action,
        }
    }

    fn execute_context_navigation_action(&mut self, action: Option<ContextAction>) {
        match action {
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
            None
            | Some(
                ContextAction::Play
                | ContextAction::PlaySelection(_)
                | ContextAction::ShuffleSelection(_)
                | ContextAction::EnqueueSelection(_)
                | ContextAction::RemoveSelection(_)
                | ContextAction::MarkPlayedSelection(_)
                | ContextAction::MarkUnplayedSelection(_)
                | ContextAction::PlayQueue(_)
                | ContextAction::PlayFolder(_)
                | ContextAction::ShuffleFolder(_)
                | ContextAction::Enqueue
                | ContextAction::EnqueueFolder(_)
                | ContextAction::MarkPlayed(_)
                | ContextAction::MarkUnplayed(_)
                | ContextAction::RemoveFromContinueWatching
                | ContextAction::RemoveFromQueue(_)
                | ContextAction::FeedsPlay(_)
                | ContextAction::FeedsEnqueue(_)
                | ContextAction::FeedsMarkPlayed(_)
                | ContextAction::FeedsMarkUnplayed(_),
            ) => {}
        }
    }

    /// Execute the bare `Play` context action: Home's Continue Watching column
    /// plays its resolved item, Queue focus replays the cursor, and an Emby
    /// library plays the current row. The paths are mutually exclusive.
    fn execute_play_action(&mut self, cw_item: Option<EmbyItem>, lib_idx: Option<usize>) {
        if matches!(self.effective_panel_focus(), PanelFocus::Library) && self.tab.is_home() {
            if let Some(item) = cw_item {
                self.cw_play(item);
            }
        } else if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
            // The queue menu carries the resolved index explicitly
            // (ContextAction::PlayQueue, D2); bare Play on Queue
            // focus should not occur, but keep the legacy read for
            // defensive parity with the pre-D2 behavior.
            let index = self.displayed_queue().queue_cursor;
            self.dispatch(&crate::app::dispatch::action::Command::QueuePlayCursor(
                index,
            ));
        } else if let Some(lib_idx) = lib_idx {
            let cursor = self
                .libs
                .get(lib_idx)
                .and_then(|lib| lib.nav_stack.last())
                .map_or(0, |l| l.resting().cursor());
            if let Some(item) = self.current_lib_item(lib_idx, cursor) {
                self.select_item(lib_idx, item);
            }
        }
    }

    /// Rebuild the local canonical queue from a context-menu selection only
    /// when this process owns playback. A direct remote queue rebuilds its own
    /// queue on submission, while an attached Session must leave the local
    /// Composed queue untouched. Replayed by `run_routed_replacement` for the
    /// gated `PlaySelection`/`ShuffleSelection` sites.
    pub(in crate::app) fn rebuild_queue_for_selection(
        &mut self,
        items: &[EmbyItem],
        source: crate::config::QueueSource,
    ) {
        let rebuild_local_queue =
            !self.has_direct_remote_queue() && self.connected_session_id.is_none();
        if rebuild_local_queue {
            self.replace_playback_queue(items.to_vec(), 0);
        }
        self.set_queue_source_if_not_local_daemon(source);
        if rebuild_local_queue {
            self.save_queue_state();
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
                if let Some(lib_idx) = lib_idx.filter(|_| played) {
                    self.remove_played_item_from_library(lib_idx, item_id);
                }
                if self.tab.is_home() {
                    match self.fetch_home() {
                        Ok(content) => {
                            // Delivered to Model-owned `home_content` via the
                            // lib_tx/ lib_rx drain (task 5.3d).
                            let _ = self
                                .channels
                                .lib_tx
                                .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                        }
                        Err(e) => {
                            self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error);
                        }
                    }
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

    fn remove_played_item_from_library(&mut self, lib_idx: usize, item_id: &str) {
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
            .and_then(|lib| lib.nav_stack.last_mut())
        {
            if lvl.unplayed_only {
                let id = item_id.to_string();
                lvl.items.retain(|item| item.id != id);
                lvl.total_count = lvl.total_count.saturating_sub(1);
            }
        }
    }

    pub(in crate::app) fn remove_from_continue_watching(&mut self, item: &EmbyItem) {
        // The shell resolved the target Continue Watching item at the Model
        // boundary from Model-owned `home_content` (task 5.3d) -- the App no
        // longer holds `home.continue_items`/`continue_cursor` to re-read.
        let Some(client) = self.emby_client() else {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return;
        };
        let client = client.lock().unwrap();
        let result = client.hide_from_resume(&item.id);
        drop(client);
        match result {
            Ok(()) => {
                match self.fetch_home() {
                    Ok(content) => {
                        // Delivered to Model-owned `home_content` via the
                        // lib_tx/ lib_rx drain (task 5.3d).
                        let _ = self
                            .channels
                            .lib_tx
                            .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                    }
                    Err(e) => {
                        self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error);
                    }
                }
            }
            Err(e) => self.flash(
                format!("Couldn't remove from continue watching: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    pub(in crate::app) fn toggle_watched_home_item(&mut self, item: &EmbyItem) {
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
                match self.fetch_home() {
                    Ok(content) => {
                        // Delivered to Model-owned `home_content` via the
                        // lib_tx/ lib_rx drain (task 5.3d).
                        let _ = self
                            .channels
                            .lib_tx
                            .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                    }
                    Err(e) => {
                        self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error);
                    }
                }
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    /// `toggle_watched`'s cursor-resolving wrapper has been deleted (task
    /// 4.3, R1): the live path is the item-taking
    /// `toggle_watched_item(lib_idx, item)` the shell routes
    /// `EmbyLibraryToggleWatched` through. Folder/audio guards, mark played/
    /// unplayed API behavior, unplayed-only/feed-home-video removal, refresh,
    /// and unavailable-Service/error toasts are preserved exactly. The
    /// unplayed-only removal previously used `lvl.cursor` (the App cursor,
    /// which the legacy call always resolves to the toggled item); it now
    /// targets the supplied item's identity — identical in the legacy flow,
    /// and correct when the component-selected item differs from a parked
    /// App cursor.
    pub(in crate::app) fn toggle_watched_item(&mut self, lib_idx: usize, item: &EmbyItem) {
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
                    self.remove_watched_item_from_library(lib_idx, &item.id);
                }
                self.refresh_lib(lib_idx);
            }
            Err(e) => self.flash(
                format!("Couldn't update play status: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    fn remove_watched_item_from_library(&mut self, lib_idx: usize, item_id: &str) {
        if self.is_feed_home_video_group_view(lib_idx) {
            if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
                state.loading = true;
            }
            self.remove_item_from_feed_home_video_cache(lib_idx, item_id);
            self.log_feed_home_video_state(lib_idx, "toggle_watched_feed");
        } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            if lvl.unplayed_only {
                if let Some(pos) = lvl.items.iter().position(|item| item.id == item_id) {
                    lvl.items.remove(pos);
                    lvl.total_count = lvl.total_count.saturating_sub(1);
                }
            }
        }
    }

    fn set_feed_entries_played(
        &mut self,
        entries: &[mbv_core::playback_queue::FeedEntry],
        played: bool,
    ) {
        let user_id = self
            .config
            .lock()
            .unwrap()
            .emby_setup
            .as_ref()
            .map_or_else(String::new, |setup| setup.user_id.clone());
        for entry in entries {
            if let Some(feed_id) = entry.feed_id.as_deref() {
                self.feed_entry_state
                    .set_played(&user_id, feed_id, &entry.guid, played);
            }
        }
        let _ = self.feed_entry_state.save();
    }
}
