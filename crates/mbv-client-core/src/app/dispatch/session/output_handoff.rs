// The continue-playback gate (confirm-output-handoff change): one shared
// predicate and one shared prompt for every Client site where a user's
// playback-output selection would hand off an active slot. The Owner keeps
// performing the hand-off itself; the Client only asks first.

use mbv_cast::discovery::CastReceiver;
use mbv_emby::SessionInfo;
use mbv_ui_msg::{Key, KeyEvent};

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::panel_targets::{PanelTarget, resolve_session_target};
use mbv_ui_model::panel_targets::SessionTargetKey;

impl App {
    /// Whether selecting a playback output now would hand off an active slot
    /// (`owner-playback-output` "Selecting an output hands off the active
    /// slot"): the local Owner holds an active slot, and the selected target
    /// is not the output that slot plays on — that selection is the Owner's
    /// no-op hand-off and needs no question. `selected_is_current_output`
    /// compares the selection against the local Owner's current output
    /// (`local_owner_output`).
    pub(in crate::app) fn output_hand_off_applies(&self, selected_is_current_output: bool) -> bool {
        !selected_is_current_output && self.local_owner_slot_active()
    }

    /// Whether the local Owner holds an active slot a selection would move:
    /// the current link's status while this Client is attached to the local
    /// Owner, the suspended home link's status while it is thin-client on a
    /// peer connection (`local_owner_plays`'s read, with a paused slot still
    /// counting — the hand-off moves it the same way). A disconnected
    /// suspended link carries a stale status and hands nothing off.
    fn local_owner_slot_active(&self) -> bool {
        // An unset endpoint (before the first connect) is the local Owner, as
        // in `local_owner_link`.
        if self
            .player_endpoint
            .as_ref()
            .is_none_or(mbv_remote_player::DaemonEndpoint::is_local)
        {
            self.player.status_snapshot().active
        } else {
            self.suspended_local.as_ref().is_some_and(|suspended| {
                !suspended.player.is_remote_disconnected()
                    && suspended.player.status_snapshot().active
            })
        }
    }

    /// The local Owner's retained playback output: the current link's record
    /// while this Client is attached to the local Owner, the suspended home
    /// link's record while it is thin-client on a peer. Each connection's
    /// reader fills its record at the handshake cold seed and on every
    /// `CtrlEvent::PlaybackOutput` since (cast-owner-output 8.2 fix), so the
    /// decision never waits for the Client's event drain. Every gate that
    /// raises a hand-off prompt and the body that runs its Yes answer MUST
    /// read this same fact — never the drained `playback_output` mirror,
    /// which while thin-client describes the peer and before the drain lags
    /// this record one tick.
    pub(in crate::app) fn local_owner_output(&self) -> Option<mbv_ctrl::OutputState> {
        // An unset endpoint (before the first connect) is the local Owner, as
        // in `local_owner_link`.
        if self
            .player_endpoint
            .as_ref()
            .is_none_or(mbv_remote_player::DaemonEndpoint::is_local)
        {
            self.player.remote().playback_output_state()
        } else {
            self.suspended_local
                .as_ref()
                .and_then(|suspended| suspended.player.remote().playback_output_state())
        }
    }

    /// Whether the local Owner can host an Emby Session output. A peer this
    /// Client controls does not take part: the selection severs to the local
    /// Owner before it is sent.
    pub(in crate::app) fn local_owner_supports_emby_session_output(&self) -> bool {
        self.local_owner_link()
            .is_some_and(|link| link.remote().supports_emby_session_output())
    }

    /// Whether the local Owner accepts `CtrlCmd::SwitchPlaybackOutput`
    /// (`output-switch`, switch-output-without-hand-off design D6). A peer
    /// this Client controls does not take part: the selection severs to the
    /// local Owner before it is sent, as for
    /// `local_owner_supports_emby_session_output`.
    pub(in crate::app) fn local_owner_supports_output_switch(&self) -> bool {
        self.local_owner_link()
            .is_some_and(|link| link.remote().supports_output_switch())
    }

    /// Whether the local Owner can host a Cast output (`playback-output`).
    /// A peer this Client controls does not take part, as for
    /// `local_owner_supports_emby_session_output`.
    pub(in crate::app) fn local_owner_supports_playback_output(&self) -> bool {
        self.local_owner_link()
            .is_some_and(|link| link.remote().supports_playback_output())
    }

    /// Whether the Session's device is the local Owner's current playback
    /// output (the no-op hand-off).
    pub(in crate::app) fn session_is_current_output(&self, sess: &SessionInfo) -> bool {
        matches!(
            self.local_owner_output(),
            Some(mbv_ctrl::OutputState::EmbySession { ref device_id, .. })
                if *device_id == sess.device_id
        )
    }

    /// Whether the receiver is the local Owner's current playback output
    /// (the no-op hand-off).
    pub(in crate::app) fn receiver_is_current_output(&self, receiver: &CastReceiver) -> bool {
        matches!(
            self.local_owner_output(),
            Some(mbv_ctrl::OutputState::Cast { ref receiver_id, .. })
                if *receiver_id == receiver.id
        )
    }

    /// Whether the local Owner's current output is a remote receiver (a Cast
    /// target or an Emby Session): selecting Local then hands that output
    /// back to this machine instead of being a no-op.
    pub(in crate::app) fn local_owner_output_is_remote(&self) -> bool {
        matches!(
            self.local_owner_output(),
            Some(mbv_ctrl::OutputState::Cast { .. } | mbv_ctrl::OutputState::EmbySession { .. })
        )
    }

    /// Raise the shared continue-playback confirm (confirm-output-handoff):
    /// the prompt names the target, Enter runs today's hand-off unchanged,
    /// and the other answer — Esc — either switches the output with no
    /// hand-off or cancels, depending on what this Owner advertised when the
    /// prompt was raised (switch-output-without-hand-off design D6).
    ///
    /// An Owner that advertises `output-switch` gets the Continue / Just
    /// switch prompt; one that does not gets Yes / No, where No cancels, so a
    /// Client meeting an Owner started before the upgrade behaves as it did
    /// and sends nothing that Owner cannot decode. The capability is read
    /// here, once, and stored in the action, so the answer always matches the
    /// buttons the user saw.
    pub(in crate::app) fn ask_continue_output_hand_off(
        &mut self,
        target: Option<SessionTargetKey>,
        name: String,
    ) {
        let switch_only = self.local_owner_supports_output_switch();
        let message = format!("Continue playback on {name}?");
        let action = mbv_ui_model::confirm::ConfirmAction::ContinueOutputHandOff {
            target,
            name,
            switch_only,
        };
        let modal = if switch_only {
            mbv_ui_model::confirm::ConfirmModal::two_button(
                message,
                "Continue",
                "Just switch",
                action,
            )
        } else {
            mbv_ui_model::confirm::ConfirmModal::two_button(message, "Yes", "No", action)
        };
        self.ask_confirm(modal);
    }

    /// The continue-playback confirm's answer: Enter (Yes) runs the
    /// selection's hand-off exactly as an unasked selection would — a remote
    /// target resolves against the displayed target snapshot (a target the
    /// snapshot no longer holds acts on nothing), and Local detaches the
    /// Owner's remote output to this machine. Esc and every other key
    /// cancel: nothing changes and the current output keeps playing.
    ///
    /// `_switch_only` is the capability the prompt was raised with
    /// (`ConfirmAction::ContinueOutputHandOff`, design D6 of
    /// `switch-output-without-hand-off`). The switch answer that reads it is
    /// not wired yet, so every key but Enter still cancels.
    pub(in crate::app) fn confirm_continue_output_hand_off(
        &mut self,
        target: Option<&SessionTargetKey>,
        name: &str,
        _switch_only: bool,
        key: KeyEvent,
    ) {
        if key.code != Key::Enter {
            return;
        }
        let Some(target) = target else {
            self.detach_to_local_confirmed();
            return;
        };
        let Some(resolved) = resolve_session_target(&self.panel_targets, target) else {
            self.flash(
                format!("\"{name}\" is no longer available"),
                ToastSeverity::Warning,
            );
            return;
        };
        match resolved {
            PanelTarget::Emby(sess) => self.activate_session_target_without_hand_off(&sess),
            PanelTarget::Cast(receiver) => self.select_cast_output(receiver),
        }
    }
}
