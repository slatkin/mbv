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
        // The menu can only have opened on a matched Emby library, Home, or
        // the queue; `context_menu_lib_idx()` resolves the explicitly matched
        // Emby library (positive match, `None` on Home/queue) that every
        // Emby-only callee below must receive. `cw_item` is the resolved
        // Continue Watching column target supplied by the Home component; it
        // feeds the Home-tab arms and the queue-menu's "Remove from Continue
        // Watching" coupling, and is ignored everywhere else.
        let lib_idx = self.context_menu_lib_idx();
        match action {
            Some(ContextAction::Play) => {
                if matches!(self.effective_panel_focus(), PanelFocus::Library) && self.tab.is_home()
                {
                    if let Some(item) = cw_item {
                        self.cw_play(item);
                    }
                } else if matches!(self.effective_panel_focus(), PanelFocus::Queue) {
                    // The queue menu carries the resolved index explicitly
                    // (ContextAction::PlayQueue, D2); bare Play on Queue
                    // focus should not occur, but keep the legacy read for
                    // defensive parity with the pre-D2 behavior.
                    let index = self.displayed_queue().queue_cursor;
                    self.dispatch(crate::app::dispatch::action::Command::QueuePlayCursor(
                        index,
                    ));
                } else if let Some(lib_idx) = lib_idx {
                    let cursor = self
                        .libs
                        .get(lib_idx)
                        .and_then(|lib| lib.nav_stack.last())
                        .map(|l| l.resting().cursor())
                        .unwrap_or(0);
                    if let Some(item) = self.current_lib_item(lib_idx, cursor) {
                        self.select_item(lib_idx, item);
                    }
                }
            }
            Some(ContextAction::PlaySelection(items)) => {
                // The selection's queue rebuild is deferred into the gated
                // confirmed path (`run_routed_replacement`), so cancelling the
                // replacement leaves the queue untouched (design D4).
                self.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx: 0,
                        source: crate::config::QueueSource::Unknown,
                        autostart: true,
                    },
                    ReplacementExecutor::Routed(RoutedReplacementPrep::Selection),
                );
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
            }
            Some(ContextAction::EnqueueSelection(items)) => {
                if let Some(lib_idx) = lib_idx {
                    for item in items {
                        self.enqueue_lib_item(lib_idx, item);
                    }
                } else {
                    for item in items {
                        if !item.is_folder && crate::app::ui_util::is_playable(&item) {
                            self.submit_queue_item(
                                mbv_core::playback_queue::QueueItem::Emby(Box::new(item)),
                                false,
                            );
                        }
                    }
                }
            }
            Some(ContextAction::RemoveSelection(targets)) => {
                // Queue targets go through one batch edit so the owner
                // publishes a single snapshot; Continue Watching targets are
                // independent Service writes.
                let mut queue_slot_ids = Vec::new();
                for target in targets {
                    match target {
                        BulkRemoveTarget::ContinueWatching(item) => {
                            self.remove_from_continue_watching(*item)
                        }
                        BulkRemoveTarget::Queue(slot_id) => queue_slot_ids.push(slot_id),
                    }
                }
                if !queue_slot_ids.is_empty() {
                    let scope = self.viewed_queue_scope();
                    self.remove_slots_from_queue(scope, &queue_slot_ids);
                }
            }
            Some(ContextAction::MarkPlayedSelection(ids)) => {
                for id in ids {
                    self.context_set_played(&id, true, lib_idx);
                }
            }
            Some(ContextAction::MarkUnplayedSelection(ids)) => {
                for id in ids {
                    self.context_set_played(&id, false, lib_idx);
                }
            }
            Some(ContextAction::PlayQueue(index)) => {
                self.dispatch(crate::app::dispatch::action::Command::QueuePlayCursor(
                    index,
                ));
            }
            Some(ContextAction::PlayFolder(id)) => {
                let ct = if let Some(lib_idx) = lib_idx {
                    self.libs[lib_idx].library.collection_type.clone()
                } else {
                    String::new()
                };
                // The folder replacement, its Collection source, and its save
                // are deferred into the gated confirmed path (design D4), so
                // cancelling leaves the queue source unchanged.
                self.play_folder(&id, ct);
            }
            Some(ContextAction::ShuffleFolder(id)) => {
                if let Some(lib_idx) = lib_idx {
                    self.shuffle_folder(lib_idx, &id);
                }
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
                        .map(|l| l.resting().cursor())
                        .unwrap_or(0);
                    if let Some(item) = self.current_lib_item(lib_idx, cursor) {
                        self.enqueue_lib_item(lib_idx, item);
                    }
                }
            }
            Some(ContextAction::EnqueueFolder(item)) => self.do_enqueue_folder((*item).clone()),
            Some(ContextAction::MarkPlayed(id)) => self.context_set_played(&id, true, lib_idx),
            Some(ContextAction::MarkUnplayed(id)) => self.context_set_played(&id, false, lib_idx),
            Some(ContextAction::RemoveFromContinueWatching) => {
                if let Some(item) = cw_item {
                    self.remove_from_continue_watching(item);
                }
            }
            Some(ContextAction::RemoveFromQueue(pos)) => self.remove_from_queue(pos),
            Some(ContextAction::FeedsPlay(entries)) => self.play_feed_entries(entries),
            Some(ContextAction::FeedsEnqueue(entries)) => self.enqueue_feed_entries(entries),
            Some(ContextAction::FeedsMarkPlayed(entries)) => {
                self.set_feed_entries_played(entries, true)
            }
            Some(ContextAction::FeedsMarkUnplayed(entries)) => {
                self.set_feed_entries_played(entries, false)
            }
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
                            }
                        }
                    }
                }
                if self.tab.is_home() {
                    match self.fetch_home() {
                        Ok(content) => {
                            // Delivered to Model-owned `home_content` via the
                            // lib_tx/ lib_rx drain (task 5.3d).
                            let _ = self
                                .lib_tx
                                .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                        }
                        Err(e) => {
                            self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error)
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

    pub(in crate::app) fn remove_from_continue_watching(&mut self, item: EmbyItem) {
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
                            .lib_tx
                            .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                    }
                    Err(e) => {
                        self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error)
                    }
                }
            }
            Err(e) => self.flash(
                format!("Couldn't remove from continue watching: {e}"),
                ToastSeverity::Error,
            ),
        }
    }

    pub(in crate::app) fn toggle_watched_home_item(&mut self, item: EmbyItem) {
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
                            .lib_tx
                            .send(LibEvent::HomeContentRefreshed(Box::new(content)));
                    }
                    Err(e) => {
                        self.flash(format!("Couldn't refresh home: {e}"), ToastSeverity::Error)
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
    pub(in crate::app) fn toggle_watched_item(&mut self, lib_idx: usize, item: EmbyItem) {
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
                            if let Some(pos) = lvl.items.iter().position(|i| i.id == item.id) {
                                lvl.items.remove(pos);
                                lvl.total_count = lvl.total_count.saturating_sub(1);
                            }
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

    fn set_feed_entries_played(
        &mut self,
        entries: Vec<mbv_core::playback_queue::FeedEntry>,
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
