//! Shell-side Pin-swap lifecycle handling (tray-pin-swap design D6). The
//! launch snapshot is a shell query (`Model::launch_state_snapshot`, the only
//! reverse read from the mounted owner) and the exit kind is shell-owned, so
//! `SwapPrepare`/`SwapQuit` never reach `App` dispatch: the run loop's
//! player-event drains hand them here first.

use crate::app::QUIT_REQUESTED;
use crate::app::shell::Model;
use mbv_ctrl::CtrlCmd;
use mbv_ctrl::player::PlayerEvent;
use mbv_player::PlayerProxy;
use std::sync::atomic::Ordering;

/// Which connected daemon link delivered a pin-swap event. The `SwapPrepared`
/// reply must travel on the same link the Owner used to ask, so the event's
/// source link names the reply target.
pub(in crate::app) enum PinSwapLink {
    /// The active player connection (`App::player`).
    Active,
    /// The suspended home link (`App::suspended_local`).
    Home,
}

impl Model {
    /// Handle one pin-swap event at the shell. Returns true when `ev` was one
    /// of the two pin-swap events and has been fully handled; the caller then
    /// skips `App` dispatch for it.
    pub(in crate::app) fn handle_pin_swap_event(
        &mut self,
        ev: &PlayerEvent,
        link: &PinSwapLink,
    ) -> bool {
        match ev {
            PlayerEvent::SwapPrepare => {
                // Save now, before the replacement Client starts (spec
                // tui-launch-state "Pin swap carries the launch location").
                // The swap is worth more than the snapshot: answer even when
                // the save failed (design D6).
                let launch_state = self.launch_state_snapshot();
                crate::app::dispatch::run_loop::teardown::save_launch_state(&launch_state);
                if !self
                    .pin_swap_reply(link)
                    .send_ctrl_cmd(CtrlCmd::SwapPrepared)
                {
                    // The link died after the request; the Owner's swap
                    // deadline abandons the swap and notifies, so a dropped
                    // reply needs no recovery here.
                    tracing::warn!(name: "pin_swap.prepared_reply.unsent", target: "pin_swap", "SwapPrepared reply could not be sent; the swap will abandon at its deadline");
                }
                true
            }
            PlayerEvent::SwapQuit => {
                // Exit swapped out: teardown skips the launch-state save and
                // the coordinated shutdown request, so the daemon and
                // playback keep running with the replacement Client (spec
                // daemon-lifecycle "Swapped-out TUI with Stay Alive off").
                self.swapped_out_exit = true;
                QUIT_REQUESTED.store(true, Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    /// The connection a `SwapPrepared` reply travels on: the same link that
    /// delivered the `SwapPrepare` request.
    fn pin_swap_reply(&self, link: &PinSwapLink) -> &PlayerProxy {
        match link {
            PinSwapLink::Active => &self.app.player,
            PinSwapLink::Home => {
                &self
                    .app
                    .suspended_local
                    .as_ref()
                    .expect("the home link delivered the pin-swap event")
                    .player
            }
        }
    }
}
