//! Shell-side Pin-swap lifecycle handling (tray-pin-swap design D6). The
//! launch snapshot is a shell query (`Model::launch_state_snapshot`, the only
//! reverse read from the mounted owner) and the exit kind is shell-owned, so
//! `SwapPrepare`/`SwapQuit` never reach `App` dispatch: the run loop's
//! player-event drains hand them here first.

use crate::app::QUIT_REQUESTED;
use crate::app::dispatch::run_loop::teardown::save_launch_state;
use crate::tui::Model;
use mbv_ui_model::playback::QueueLink;
use std::sync::atomic::Ordering;

/// Which connected daemon link delivered a pin-swap event. The `SwapPrepared`
/// reply must travel on the same link the Owner used to ask, so the event's
/// source link names the reply target.
pub(in crate::tui) enum PinSwapLink {
    /// The active player connection (`App::player`).
    Active,
    /// The suspended home link (`App::suspended_local`).
    Home,
}

impl PinSwapLink {
    /// The core queue link this connection is: the active connection is the
    /// peer link, and the suspended home connection is the local link
    /// (`App::queue_owner_player`'s pairing).
    fn reply_link(&self) -> QueueLink {
        match self {
            PinSwapLink::Active => QueueLink::Peer,
            PinSwapLink::Home => QueueLink::Local,
        }
    }
}

impl Model {
    /// Handle `SwapPrepare`: save the launch state, then answer on `link`.
    pub(in crate::tui) fn prepare_pin_swap(&mut self, link: &PinSwapLink) {
        // Save now, before the replacement Client starts (spec
        // tui-launch-state "Pin swap carries the launch location").
        // The swap is worth more than the snapshot: answer even when
        // the save failed (design D6).
        let launch_state = self.launch_state_snapshot();
        save_launch_state(&launch_state);
        if !self.app.reply_pin_swap_prepared(link.reply_link()) {
            // The link died after the request; the Owner's swap
            // deadline abandons the swap and notifies, so a dropped
            // reply needs no recovery here.
            tracing::warn!(name: "pin_swap.prepared_reply.unsent", target: "pin_swap", "SwapPrepared reply could not be sent; the swap will abandon at its deadline");
        }
    }

    /// Handle `SwapQuit`: exit swapped out.
    pub(in crate::tui) fn quit_swapped_out(&mut self) {
        // Teardown skips the launch-state save and
        // the coordinated shutdown request, so the daemon and
        // playback keep running with the replacement Client (spec
        // daemon-lifecycle "Swapped-out TUI with Stay Alive off").
        self.swapped_out_exit = true;
        QUIT_REQUESTED.store(true, Ordering::Relaxed);
    }
}
