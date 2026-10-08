use super::{
    App, ConfirmAction, ConfirmModal, PanelFocus, PendingQueueAction, ReplacementExecutor,
    RoutedReplacementPrep,
};
use crate::app::SidebarId;
use mbv_ui_model::confirm::ConfirmButton;

impl App {
    pub(in crate::app) fn on_queue_replace_silent(&mut self) {
        self.queue_dirty = false;
    }

    pub(in crate::app) fn replace_queue_or_prompt(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action)
            && self.queue_dirty
            && self.queue_is_saved_playlist()
        {
            self.queue_deferrals.defer_for_save_answer(action);
            let name = mbv_ui_model::ui_util::trunc_str(self.queue_playlist_name(), 36);
            self.ask_confirm(ConfirmModal {
                message: format!("Save changes to \"{name}\"?"),
                buttons: vec![
                    ConfirmButton::affirmative("Enter", "Save"),
                    ConfirmButton::cancel("d", "Discard"),
                    ConfirmButton::cancel("Esc", "Cancel"),
                ],
                on_confirm: ConfirmAction::DiscardOrSaveDirtyPlaylist,
            });
        } else {
            self.execute_pending_queue_action(action);
        }
    }

    /// Design D6 predicate: executing `action` would replace a populated
    /// playback-target queue (local or directly controlled remote), so the
    /// caller must confirm before the replacement runs. An empty target queue
    /// needs no confirmation. Only a `PlayItems` payload is gated; a bare
    /// clear already owns its own confirmation flow.
    pub(in crate::app) fn queue_replacement_needs_confirmation(
        &self,
        action: &PendingQueueAction,
    ) -> bool {
        matches!(action, PendingQueueAction::PlayItems { .. })
            && self.playback_queue().total_queue_len() > 0
    }

    /// Design D6 entry point for a resolved queue replacement: an empty target
    /// queue executes immediately, a populated one stores the complete action
    /// and asks first. Local saved-playlist protection is not part of this
    /// gate; the confirmed execution still runs it through
    /// `replace_queue_or_prompt`.
    ///
    /// The gated payload belongs to the `QueueDeferrals` gate transition,
    /// while the save completion consumes only the save-bound transition, so
    /// an in-flight playlist save cannot fire a replacement the user never confirmed.
    pub(in crate::app) fn request_queue_replacement(
        &mut self,
        action: PendingQueueAction,
        via: ReplacementExecutor,
    ) {
        if self.queue_replacement_needs_confirmation(&action) {
            self.queue_deferrals.hold_gated_replacement(action, via);
            self.ask_confirm(ConfirmModal::two_button(
                "Replace the current queue?".into(),
                "Confirm",
                "Cancel",
                ConfirmAction::ReplacePopulatedQueue,
            ));
        } else {
            self.run_replacement(action, &via);
        }
    }

    /// Runs one already-confirmed (or gate-free) queue replacement through the
    /// executor its entry point selected. Both the empty-queue path and the
    /// `ReplacePopulatedQueue` confirmation arm call this, so a gated payload
    /// replays exactly what an ungated one would have. `Pending` only replaces
    /// the queue; `PlaylistsSidebar` also closes the Playlists sidebar and
    /// focuses the Queue when no overlay was raised.
    pub(in crate::app) fn run_replacement(
        &mut self,
        action: PendingQueueAction,
        via: &ReplacementExecutor,
    ) {
        match via {
            ReplacementExecutor::Pending => self.execute_queue_replacement(action),
            ReplacementExecutor::PlaylistsSidebar => {
                self.execute_queue_replacement(action);
                // The Playlists sidebar closes so the replaced queue is
                // visible. Done here rather than at the call site so a gated
                // replacement still dismisses on confirm while a cancelled one
                // leaves the sidebar alone. A raised save/discard prompt keeps
                // the sidebar.
                if self.pending_overlay.is_none() {
                    self.request_sidebar_dismiss(SidebarId::Playlists);
                    self.set_panel_focus(PanelFocus::Queue);
                }
            }
            ReplacementExecutor::Routed(prep) => self.run_routed_replacement(action, *prep),
        }
    }

    /// Replays one routed entry point's pre-play prep, then runs the shared
    /// `play_items_routed` executor. The prep is the site policy carried by
    /// `ReplacementExecutor::Routed`; `play_items_routed` itself never
    /// replaces the queue.
    fn run_routed_replacement(&mut self, action: PendingQueueAction, prep: RoutedReplacementPrep) {
        let PendingQueueAction::PlayItems {
            items,
            start_idx,
            source,
            ..
        } = action
        else {
            return;
        };
        match prep {
            RoutedReplacementPrep::Album
            | RoutedReplacementPrep::MusicAlbums
            | RoutedReplacementPrep::Selection => {
                self.play_items_routed(items, start_idx, source);
            }
            RoutedReplacementPrep::Folder | RoutedReplacementPrep::ShuffleFolder => {
                self.set_panel_focus(PanelFocus::Queue);
                self.play_items_routed(items, start_idx, source);
            }
        }
    }

    /// Executes one already-resolved replacement through the existing playback
    /// and admission executor. Local saved-playlist protection still runs
    /// through `replace_queue_or_prompt`, which may raise its own save/discard
    /// prompt as a second step before this payload executes; the queue itself
    /// is replaced on the Player owner by the play executor (row 5.3), so no
    /// Client-side queue staging happens here.
    pub(in crate::app) fn execute_queue_replacement(&mut self, action: PendingQueueAction) {
        self.replace_queue_or_prompt(action);
    }

    pub(in crate::app) fn execute_pending_queue_action(&mut self, action: PendingQueueAction) {
        if self.action_touches_local_queue(&action) {
            self.queue_dirty = false;
        }
        match action {
            PendingQueueAction::PlayItems {
                items,
                start_idx,
                source,
            } => self.execute_pending_play_items(items, start_idx, source),
            PendingQueueAction::ClearQueue => self.execute_pending_queue_clear(),
        }
    }
}
