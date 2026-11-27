pub(in crate::app) mod emby_writes;

use crate::app::dispatch::library::stores::ClientLibraryStores;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::{
    App, ContextAction, PanelFocus, PendingQueueAction, ReplacementExecutor, RoutedReplacementPrep,
};
use mbv_emby_model::EmbyItem;
use mbv_queue::QueueItem;
use mbv_ui_model::context_menu::{AudiobookshelfMenuTarget, BulkRemoveTarget};
use rand::seq::SliceRandom;

impl App {
    /// Relay an accepted manual played-state mark into the Player owner's
    /// queue through the shared `QueueOp::ApplyProgress` path — the same
    /// propagation the Audiobookshelf accepted-mark local apply uses
    /// (standard-media-context-menus design D6). Every non-active slot whose
    /// content matches takes the new state, and the owner's broadcast
    /// repaints the queue rows; the actively owned session is left out on the
    /// owner side. A send failure surfaces as a warning, never silently: the
    /// Service write already succeeded, so the queue row would otherwise stay
    /// stale without a word.
    pub(in crate::app) fn propagate_played_to_queue(
        &mut self,
        updates: Vec<mbv_ctrl::ProgressUpdate>,
    ) {
        if updates.is_empty() {
            return;
        }
        let result = {
            let (player, _) = self.queue_link(crate::app::QueueLink::Local);
            player
                .remote()
                .send_queue_op(mbv_remote_player::QueueOp::ApplyProgress { updates }, None)
        };
        if let Err(error) = result {
            self.flash(error.to_string(), ToastSeverity::Warning);
        }
    }

    /// Play the resolved queue items in list order: the first replaces the
    /// queue and starts, the rest append (standard-media-context-menus task
    /// 5.3, design D4). Shuffle callers randomize before calling.
    fn play_audiobookshelf_menu_targets_in_order(&mut self, targets: &[AudiobookshelfMenuTarget]) {
        let items = self.audiobookshelf_menu_targets_queue_items(targets);
        self.submit_audiobookshelf_queue_items(items);
    }

    /// `submit_queue_item(first, true)` runs, then each remaining item
    /// appends; a target that no longer resolves was already skipped.
    fn submit_audiobookshelf_queue_items(&mut self, items: Vec<QueueItem>) {
        let mut items = items.into_iter();
        let Some(first) = items.next() else {
            return;
        };
        self.submit_queue_item(first, true);
        for item in items {
            self.submit_queue_item(item, false);
        }
    }

    pub fn execute_context_action(
        &mut self,
        action: Option<ContextAction>,
        cw_item: Option<EmbyItem>,
    ) {
        self.execute_context_action_with_stores(action, cw_item, None);
    }

    /// The same dispatch with the Client's own library stores (emby-favourites
    /// design D4): a write whose landing changes what those stores hold raises
    /// its [`ClientLibraryEffect`] through them, so a Client that keeps its own
    /// copies of the item takes the new value. A Client that keeps none (the
    /// TUI) passes `None`, exactly as it does for a library event.
    pub fn execute_context_action_with_stores(
        &mut self,
        action: Option<ContextAction>,
        cw_item: Option<EmbyItem>,
        stores: Option<&mut dyn ClientLibraryStores>,
    ) {
        // The shell dismisses the mounted ContextMenu component; this only
        // dispatches the chosen action (task 5.3c).
        let lib_idx = self.context_menu_lib_idx();
        let action = self.execute_context_visual_action(action);
        let action = self.execute_context_selection_action(action, lib_idx);
        let action = self.execute_context_library_action(action, cw_item, lib_idx, stores);
        let action = self.execute_context_queue_and_feed_action(action);
        self.execute_context_navigation_action(action);
    }

    /// The queue visual slot's chrome-menu actions (the now-playing image
    /// menu): select the artwork or the visualizer, or hide the slot. They
    /// carry no row targets, so they run ahead of the selection and library
    /// chains, which pass the action through unchanged.
    fn execute_context_visual_action(
        &mut self,
        action: Option<ContextAction>,
    ) -> Option<ContextAction> {
        match action {
            Some(ContextAction::ShowVisualizer) => {
                self.set_visualizer(true);
                None
            }
            Some(ContextAction::ShowMediaArt) => {
                self.set_visualizer(false);
                None
            }
            Some(ContextAction::HideVisualSlot) => {
                self.set_visual_slot_hidden(true);
                None
            }
            other => other,
        }
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
                        source: mbv_queue::QueueSource::Unknown,
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
                        source: mbv_queue::QueueSource::Shuffle,
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
                        .filter(|item| !item.is_folder && mbv_ui_model::ui_util::is_playable(item))
                    {
                        self.submit_queue_item(mbv_queue::QueueItem::Emby(Box::new(item)), false);
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
                    self.remove_slots_from_queue(self.queue_edit_target(), &queue_slot_ids);
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
            // Home mixed-Service selection actions (spec `home-latest-sections`,
            // "Home actions resolve the row's own section and Service
            // identity"): the first resolved item plays — replacing the queue and
            // starting — and the rest append in display order, through the
            // same per-item effects `HomePlay`/`HomeEnqueue` use.
            Some(ContextAction::HomePlaySelection(items)) => {
                let mut items = items.into_iter();
                if let Some(first) = items.next() {
                    self.home_play_target(first, false);
                }
                for item in items {
                    self.home_enqueue_target(item, false);
                }
                None
            }
            Some(ContextAction::HomeEnqueueSelection(items)) => {
                for item in items {
                    self.home_enqueue_target(item, false);
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
        stores: Option<&mut dyn ClientLibraryStores>,
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
            Some(ContextAction::Favourite(id)) => {
                self.context_set_favourite(&id, true, stores);
                None
            }
            Some(ContextAction::Unfavourite(id)) => {
                self.context_set_favourite(&id, false, stores);
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
            Some(ContextAction::ShuffleQueue) => {
                // queue-content-shuffle design D6: the menu only offers the
                // entry when the owner advertises the `queue-shuffle`
                // capability, so this is a plain send; the next queue
                // snapshot reports the new order. The shuffle names the shown
                // queue (design D7 of `gui-named-queues`).
                self.shown_queue_owner_player()
                    .shuffle_queue(self.shown_queue_edit());
                None
            }
            Some(ContextAction::PlayQueue(index)) => {
                self.dispatch(&crate::app::dispatch::action::AppCommand::QueuePlayCursor(
                    index,
                ));
                None
            }
            Some(ContextAction::RemoveFromQueue(pos)) => {
                self.remove_from_queue(pos);
                None
            }
            Some(ContextAction::ClearQueue) => {
                self.request_clear_queue();
                None
            }
            Some(ContextAction::FeedsPlay(entries)) => {
                self.play_feed_entries(entries);
                None
            }
            Some(ContextAction::FeedsShuffle(mut entries)) => {
                entries.shuffle(&mut rand::rng());
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
            Some(ContextAction::AudiobookshelfPlay(targets)) => {
                if self.player.can_admit_audiobookshelf() {
                    self.play_audiobookshelf_menu_targets_in_order(&targets);
                } else {
                    self.flash(
                        "Audiobookshelf playback owner is unavailable".into(),
                        ToastSeverity::Error,
                    );
                }
                None
            }
            Some(ContextAction::AudiobookshelfShuffle(targets)) => {
                if self.player.can_admit_audiobookshelf() {
                    let mut items = self.audiobookshelf_menu_targets_queue_items(&targets);
                    items.shuffle(&mut rand::rng());
                    self.submit_audiobookshelf_queue_items(items);
                } else {
                    self.flash(
                        "Audiobookshelf playback owner is unavailable".into(),
                        ToastSeverity::Error,
                    );
                }
                None
            }
            Some(ContextAction::AudiobookshelfEnqueue(targets)) => {
                // The owner capability gates the enqueue: the shown queue is
                // always a Player owner's queue (ADR 0034), so the check
                // applies unconditionally.
                if !self.player.can_admit_audiobookshelf() {
                    self.flash(
                        "Audiobookshelf playback owner is unavailable".into(),
                        ToastSeverity::Error,
                    );
                    return None;
                }
                for item in self.audiobookshelf_menu_targets_queue_items(&targets) {
                    self.submit_queue_item(item, false);
                }
                None
            }
            Some(ContextAction::AudiobookshelfMarkPlayed(targets)) => {
                self.start_audiobookshelf_mark(targets, true);
                None
            }
            Some(ContextAction::AudiobookshelfMarkUnplayed(targets)) => {
                self.start_audiobookshelf_mark(targets, false);
                None
            }
            action => action,
        }
    }

    /// Start the finished-state mark worker for `targets` (design D5). A
    /// Service that is not Ready, a missing setup or credential, or a mark
    /// still in flight flashes a toast and changes nothing; otherwise the
    /// worker runs on a thread and reports through the generation-gated mark
    /// completion. A mark already in flight holds the only completion
    /// channel: starting a second worker would orphan the first one's result
    /// (PR 914 review), so the request is refused outright.
    fn start_audiobookshelf_mark(
        &mut self,
        targets: Vec<AudiobookshelfMenuTarget>,
        finished: bool,
    ) {
        if self.setup.audiobookshelf_mark_rx.is_some() {
            self.flash(
                "Audiobookshelf mark already in progress".into(),
                ToastSeverity::Warning,
            );
            return;
        }
        if self.audiobookshelf_runtime.state != mbv_core::service_runtime::ServiceState::Ready {
            self.flash(
                "Audiobookshelf is unavailable".into(),
                ToastSeverity::Warning,
            );
            return;
        }
        let generation = self.audiobookshelf_runtime.generation();
        let Some((setup, api_key)) =
            crate::app::dispatch::session::service_startup::audiobookshelf_setup_and_key(
                &self.config.lock().unwrap(),
            )
        else {
            self.flash(
                "Audiobookshelf is unavailable".into(),
                ToastSeverity::Warning,
            );
            return;
        };
        self.setup.audiobookshelf_mark_rx = Some(
            crate::app::dispatch::session::service_startup::start_audiobookshelf_mark(
                setup, api_key, generation, targets, finished,
            ),
        );
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
                | ContextAction::Favourite(_)
                | ContextAction::Unfavourite(_)
                | ContextAction::RemoveFromContinueWatching
                | ContextAction::RemoveFromQueue(_)
                | ContextAction::ClearQueue
                | ContextAction::ShuffleQueue
                | ContextAction::FeedsPlay(_)
                | ContextAction::FeedsShuffle(_)
                | ContextAction::FeedsEnqueue(_)
                | ContextAction::FeedsMarkPlayed(_)
                | ContextAction::FeedsMarkUnplayed(_)
                | ContextAction::AudiobookshelfPlay(_)
                | ContextAction::AudiobookshelfShuffle(_)
                | ContextAction::AudiobookshelfEnqueue(_)
                | ContextAction::AudiobookshelfMarkPlayed(_)
                | ContextAction::AudiobookshelfMarkUnplayed(_)
                | ContextAction::HomePlaySelection(_)
                | ContextAction::HomeEnqueueSelection(_)
                | ContextAction::ShowVisualizer
                | ContextAction::ShowMediaArt
                | ContextAction::HideVisualSlot,
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
            let index = self.displayed_queue().cursor();
            self.dispatch(&crate::app::dispatch::action::AppCommand::QueuePlayCursor(
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

    /// The played-state action a `Recently added` row's context menu offers
    /// (spec `home-latest-sections`, "Home actions resolve the row's own
    /// section and Service identity"): for each Service, the one
    /// mark entry its destination menu builds from the item's cached state —
    /// mark played when unfinished, mark unplayed when finished.
    pub fn toggle_recent_item_played_state(&mut self, item: &QueueItem) {
        match item {
            QueueItem::Emby(item) => self.toggle_watched_home_item(item),
            QueueItem::Feed(entry) => {
                self.set_feed_entries_played(std::slice::from_ref(entry), !entry.played);
            }
            QueueItem::Audiobookshelf(item) => {
                let target = AudiobookshelfMenuTarget::for_item(item);
                let finished = self.audiobookshelf_target_finished(&target);
                self.start_audiobookshelf_mark(vec![target], !finished);
            }
            // A Twitch row offers the generic queue actions only: played
            // state is a Service concept it does not carry (design D8).
            QueueItem::Twitch(_) => {}
        }
    }
}
