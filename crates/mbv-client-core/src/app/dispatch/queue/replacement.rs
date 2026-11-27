use super::{App, PanelFocus, PendingQueueAction, ReplacementExecutor, RoutedReplacementPrep};
use crate::app::SidebarId;

impl App {
    pub(in crate::app) fn on_queue_replace_silent(&mut self) {
        self.queue_dirty = false;
    }

    /// Entry point for a resolved queue replacement (user request
    /// 2026-11-27): every replacement executes at once — no populated-queue
    /// confirmation stands before it, and no unsaved-playlist save/discard
    /// prompt follows. The user decides; the replacement replaces.
    pub(in crate::app) fn request_queue_replacement(
        &mut self,
        action: PendingQueueAction,
        via: ReplacementExecutor,
    ) {
        self.run_replacement(action, via);
    }

    /// Runs one resolved queue replacement through the executor its entry
    /// point selected. `Pending` only replaces the queue; `PlaylistsSidebar`
    /// also closes the Playlists sidebar and focuses the Queue.
    pub(in crate::app) fn run_replacement(
        &mut self,
        action: PendingQueueAction,
        via: ReplacementExecutor,
    ) {
        match via {
            ReplacementExecutor::Pending => self.execute_queue_replacement(action),
            ReplacementExecutor::PlaylistsSidebar => {
                self.execute_queue_replacement(action);
                if self.pending_overlay.is_none() {
                    self.request_sidebar_dismiss(SidebarId::Playlists);
                    self.set_panel_focus(PanelFocus::Queue);
                }
            }
            ReplacementExecutor::Routed(prep) => self.run_routed_replacement(action, prep),
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

    /// Executes one resolved replacement through the existing playback and
    /// admission executor. The queue itself is replaced on the Player owner
    /// by the play executor (row 5.3), so no Client-side queue staging
    /// happens here.
    pub(in crate::app) fn execute_queue_replacement(&mut self, action: PendingQueueAction) {
        self.execute_pending_queue_action(action);
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
