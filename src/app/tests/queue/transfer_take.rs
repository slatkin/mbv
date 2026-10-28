//! Row 8.1 (design D1 right column): the Client-run Take sequence, mocked
//! on both links. Every test owns one `queue-handoff` contract; receivers
//! are injected and answers are pre-pushed, so no test sleeps.

use std::sync::mpsc;

use crossterm::event::KeyCode;

use crate::app::tests::tick_integration::harness::TickHarness;
use mbv_ctrl::{CtrlCmd, CtrlCompatibility};
use mbv_remote_player::{DaemonEndpoint, RemotePlayer};
use mbv_ui_model::confirm::ConfirmAction;
use mbv_ui_model::overlay::OverlayRequest;

use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::panel_targets::PanelTarget;
use crate::app::state::queue_transfer::QueueTransferDirection;
use crate::app::tests::queue::transfer_handoff::{
    PROBE_LINK, applied, confirm_key, hand_off_key as transfer_key, peer_compat, peer_replace,
    peer_session, peer_snapshot, staged_probe_connect, stop_sent_to,
};
use crate::app::tests::{
    make_audio_items, make_local_daemon_app_stub_with_cmd_rx,
    make_local_daemon_app_stub_with_cmd_rx_and_compat,
};

/// The local Owner's negotiated compatibility for the copy: a real Owner
/// advertises the start-position capability from its Hello (task 2.3); the
/// plain stub baseline advertises nothing.
fn owner_compat() -> CtrlCompatibility {
    let mut compat = CtrlCompatibility::current();
    compat.supports_answered_queue_ops = true;
    compat.supports_queue_replace_start_position = true;
    compat
}

/// The `UnifiedQueueReplace` the local Owner received, if any.
fn local_replace(local_cmd_rx: &mpsc::Receiver<CtrlCmd>) -> Option<CtrlCmd> {
    let mut replace = None;
    while let Ok(cmd) = local_cmd_rx.try_recv() {
        if matches!(cmd, CtrlCmd::UnifiedQueueReplace { .. }) {
            replace = Some(cmd);
        }
    }
    replace
}

#[test]
fn take_from_the_connected_peer_stops_it_only_after_local_observes_and_ends_unconnected() {
    // queue-handoff "Take copies the peer queue to the local Owner and moves
    // playback there" + "Take disconnects and works without a connection",
    // scenario "Take from the connected peer": the local Owner starts the
    // peer's queue at the peer position, the peer stops only in the tick
    // that observes the start slot playing, and the Client ends unconnected.
    let (mut app, local_cmd_rx) =
        make_local_daemon_app_stub_with_cmd_rx_and_compat(make_audio_items(2), owner_compat());
    let peer_items = make_audio_items(5);
    let (mut peer, _stub_rx, peer_cmd_rx) =
        RemotePlayer::stub_with_command_rx(peer_items.clone(), 1);
    peer.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    // The peer plays slot 2 of 5 at 03:10.
    peer.update_status(|status| status.position_ticks = 1_900_000_000);
    let sess = peer_session("peer-owner");
    let (_dropped_tx, attach_rx) = mpsc::channel();
    app.switch_to_direct_remote(
        &sess,
        peer,
        attach_rx,
        &DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
    );
    app.panel_targets = vec![PanelTarget::Emby(Box::new(sess))];
    // The local Owner's post-replacement answer, pre-pushed onto its
    // suspended link: the submitted slot ids (1..=5) are kept (design D8),
    // and the start slot 2 is observed active and playing.
    let (home_tx, home_rx) = mpsc::channel();
    app.suspended_local.as_mut().unwrap().player_rx = home_rx;
    home_tx
        .send(applied(1, peer_snapshot(&peer_items, 1, true)))
        .unwrap();
    while local_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}

    app.begin_queue_transfer(QueueTransferDirection::Take, &transfer_key());

    // The populated local queue raised the confirm prompt and nothing was
    // sent to either Player owner.
    let modal = match &app.pending_overlay {
        Some(OverlayRequest::Confirm(modal)) => modal,
        other => panic!("a populated local queue must raise the confirm prompt, got {other:?}"),
    };
    assert!(
        modal.message.contains(mbv_emby::device_name().as_str()),
        "the prompt names the local Owner, got {:?}",
        modal.message
    );
    assert!(
        local_replace(&local_cmd_rx).is_none(),
        "nothing is sent before the user confirms"
    );
    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "the peer receives nothing before the user confirms"
    );
    // Mirror the shell's modal dismissal before the confirm answer.
    app.pending_overlay = None;

    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(transfer_key()),
        confirm_key(KeyCode::Enter),
    );

    // The local Owner carries the copy: the peer queue's submitted slot ids,
    // the peer active slot's index, and the peer playback position.
    let Some(CtrlCmd::UnifiedQueueReplace {
        slots,
        start_idx,
        start_position_secs,
        op,
        ..
    }) = local_replace(&local_cmd_rx)
    else {
        panic!("the confirmed Take sends the copy, got no replace");
    };
    assert_eq!(
        slots.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
        vec![1, 2, 3, 4, 5],
        "the copy carries the peer queue's submitted slot ids"
    );
    assert_eq!(
        start_idx,
        Some(1),
        "the copy starts at the peer's active slot"
    );
    assert_eq!(
        start_position_secs,
        Some(190.0),
        "the copy carries the peer playback position"
    );
    assert!(op.is_some(), "the copy is an answered owner operation");
    assert!(
        app.pending_queue_transfer.is_some(),
        "the pending transfer arms after the copy is sent"
    );
    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "the peer keeps playing while the local Owner starts"
    );

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.pending_queue_transfer.is_none(), "the transfer settles");
    assert!(
        stop_sent_to(&peer_cmd_rx),
        "the peer stops only after the local Owner observed the start slot"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "the local Owner never stops");
    assert!(
        !app.direct_remote_connected && !app.has_remote_queue(),
        "the Take ends unconnected and the queue panel shows the local Owner queue"
    );
    assert!(
        app.local_owner_plays(),
        "the local Owner keeps playing the taken queue"
    );
}

#[test]
fn take_from_an_unconnected_peer_stops_the_probe_after_local_observes_and_stays_unconnected() {
    // queue-handoff "Take disconnects and works without a connection",
    // scenario "Take from an unconnected peer", plus the probe-stop contract
    // (row 8.1): the source Stop reaches the probe link the transfer opened
    // — never `self.player` — and the settlement closes that link.
    let _guard = crate::app::DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    let (mut app, local_cmd_rx) =
        make_local_daemon_app_stub_with_cmd_rx_and_compat(make_audio_items(2), owner_compat());
    let peer_items = make_audio_items(3);
    let (mut probe, _stub_rx, probe_cmd_rx) =
        RemotePlayer::stub_with_command_rx(peer_items.clone(), 1);
    probe.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    // The peer plays slot 2 of 3 at 42.0s.
    probe.update_status(|status| status.position_ticks = 420_000_000);
    let (_dropped_tx, probe_event_rx) = mpsc::channel();
    *PROBE_LINK.lock().unwrap() = Some((probe, probe_event_rx));
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(staged_probe_connect);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(peer_session("peer-owner")))];
    // The local Owner's post-replacement answer, pre-pushed onto its event
    // receiver: the submitted slot ids (1..=3) are kept (design D8), and the
    // start slot 2 is observed active and playing.
    let (local_tx, local_rx) = mpsc::channel();
    app.player_rx = local_rx;
    local_tx
        .send(applied(1, peer_snapshot(&peer_items, 1, true)))
        .unwrap();
    while local_cmd_rx.try_recv().is_ok() {}
    while probe_cmd_rx.try_recv().is_ok() {}

    app.begin_queue_transfer(QueueTransferDirection::Take, &transfer_key());

    assert!(
        matches!(app.pending_overlay, Some(OverlayRequest::Confirm(_))),
        "the populated local queue asks first"
    );
    assert!(
        local_replace(&local_cmd_rx).is_none() && peer_replace(&probe_cmd_rx).is_none(),
        "nothing is sent before the user confirms"
    );
    app.pending_overlay = None;

    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(transfer_key()),
        confirm_key(KeyCode::Enter),
    );

    let Some(CtrlCmd::UnifiedQueueReplace {
        slots,
        start_idx,
        start_position_secs,
        ..
    }) = local_replace(&local_cmd_rx)
    else {
        panic!("the confirmed Take sends the copy, got no replace");
    };
    assert_eq!(
        slots.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "the copy carries the peer queue's submitted slot ids"
    );
    assert_eq!(
        start_idx,
        Some(1),
        "the copy starts at the peer's active slot"
    );
    assert_eq!(
        start_position_secs,
        Some(42.0),
        "the copy carries the peer playback position"
    );
    assert!(
        app.pending_queue_transfer.is_some(),
        "the pending transfer arms after the copy is sent"
    );
    assert!(
        !app.direct_remote_connected && !app.has_remote_queue(),
        "the unconnected Take never attaches the probe"
    );
    assert!(
        peer_replace(&probe_cmd_rx).is_none(),
        "the probe receives nothing but the final stop"
    );

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.pending_queue_transfer.is_none(), "the transfer settles");
    assert!(
        stop_sent_to(&probe_cmd_rx),
        "the source stop reaches the probe link, never self.player"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "the local Owner never stops");
    assert!(
        app.staged_queue_transfer.is_none()
            && !app.direct_remote_connected
            && !app.has_remote_queue(),
        "the settled Take leaves no staged link and the Client stays unconnected"
    );
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn a_peer_snapshot_without_an_active_slot_receives_nothing_and_leaves_the_local_queue_unchanged() {
    // queue-handoff "Hand off and Take need playback on the source",
    // scenario "Stale F3 row shows the peer playing": the peer snapshot has
    // no active slot, so the Client reports, sends nothing to either owner,
    // and both queues stay as they are.
    let local_items = make_audio_items(2);
    let (mut app, local_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(local_items.clone());
    let (mut peer, _stub_rx, peer_cmd_rx) =
        RemotePlayer::stub_with_command_rx(make_audio_items(3), 0);
    peer.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    // The stale row: the peer snapshot holds no active slot and nothing
    // plays, while the F3 row still showed the peer playing (design D6's
    // component gate).
    peer.update_unified_queue_state_for_test(|state| {
        state.active_slot = None;
        state.status.active = false;
        state.status.paused = true;
    });
    let sess = peer_session("peer-owner");
    let (_dropped_tx, attach_rx) = mpsc::channel();
    app.switch_to_direct_remote(
        &sess,
        peer,
        attach_rx,
        &DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
    );
    app.panel_targets = vec![PanelTarget::Emby(Box::new(sess))];
    while local_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}

    app.begin_queue_transfer(QueueTransferDirection::Take, &transfer_key());

    assert!(
        local_replace(&local_cmd_rx).is_none(),
        "the local Owner receives no copy"
    );
    assert!(
        peer_replace(&peer_cmd_rx).is_none() && !stop_sent_to(&peer_cmd_rx),
        "the peer receives nothing"
    );
    assert!(app.pending_queue_transfer.is_none());
    assert!(app.staged_queue_transfer.is_none());
    assert!(
        app.status.contains("nothing is playing on the peer"),
        "the report names the stale source, got {:?}",
        app.status
    );
    assert_eq!(
        app.local_view
            .slots()
            .iter()
            .map(|slot| slot.slot_id.raw())
            .collect::<Vec<_>>(),
        vec![1, 2],
        "the local queue is unchanged"
    );
}

#[test]
fn a_connected_take_whose_peer_link_died_never_stops_the_local_owner() {
    // PR #973 spec review: the peer link drops after the copy is armed and
    // the disconnect teardown reinstalls the home link before the settle
    // ticks. The settlement must not resolve the source link through
    // `self.player` — that is now the local Owner, and stopping it would
    // kill the playback the Take just delivered.
    let (mut app, local_cmd_rx) =
        make_local_daemon_app_stub_with_cmd_rx_and_compat(make_audio_items(2), owner_compat());
    let (mut peer, _stub_rx, peer_cmd_rx) =
        RemotePlayer::stub_with_command_rx(make_audio_items(3), 1);
    peer.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    let sess = peer_session("peer-owner");
    let (_dropped_tx, attach_rx) = mpsc::channel();
    app.switch_to_direct_remote(
        &sess,
        peer,
        attach_rx,
        &DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
    );
    app.panel_targets = vec![PanelTarget::Emby(Box::new(sess))];
    // The local Owner's post-replacement answer, pre-pushed onto its
    // suspended link: the start slot is observed active and playing.
    let (home_tx, home_rx) = mpsc::channel();
    app.suspended_local.as_mut().unwrap().player_rx = home_rx;
    home_tx
        .send(applied(1, peer_snapshot(&make_audio_items(3), 1, true)))
        .unwrap();
    while local_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}

    app.begin_queue_transfer(QueueTransferDirection::Take, &transfer_key());
    app.pending_overlay = None;
    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(transfer_key()),
        confirm_key(KeyCode::Enter),
    );
    assert!(
        app.pending_queue_transfer.is_some(),
        "the confirmed Take armed the pending transfer"
    );

    // The peer link drops and the teardown reinstalls the home link (auto
    // reattach disabled or failed): `self.player` is the local Owner again.
    app.restore_local_mode("Daemon disconnected — returned to local mode");

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.pending_queue_transfer.is_none(), "the transfer settles");
    assert!(
        !stop_sent_to(&local_cmd_rx),
        "the source stop must never land on the local Owner"
    );
    assert!(
        app.local_owner_plays(),
        "the local Owner keeps playing the taken queue"
    );
    assert!(
        !app.direct_remote_connected && !app.has_remote_queue(),
        "the Take ends unconnected"
    );
}

#[test]
fn a_dead_home_link_refuses_the_connected_take_before_anything_is_sent() {
    // Review P2 (row 8.1): behind an attachment whose home link died, the
    // local Owner link is gone — the copy must not reach the peer
    // attachment instead of the local Owner.
    let (mut app, local_cmd_rx) =
        make_local_daemon_app_stub_with_cmd_rx_and_compat(make_audio_items(2), owner_compat());
    let (mut peer, _stub_rx, peer_cmd_rx) =
        RemotePlayer::stub_with_command_rx(make_audio_items(3), 1);
    peer.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    let sess = peer_session("peer-owner");
    let (_dropped_tx, attach_rx) = mpsc::channel();
    app.switch_to_direct_remote(
        &sess,
        peer,
        attach_rx,
        &DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
    );
    app.panel_targets = vec![PanelTarget::Emby(Box::new(sess))];
    // The home link died behind the attachment.
    app.suspended_local = None;
    while local_cmd_rx.try_recv().is_ok() {}
    while peer_cmd_rx.try_recv().is_ok() {}

    app.begin_queue_transfer(QueueTransferDirection::Take, &transfer_key());

    assert!(
        local_replace(&local_cmd_rx).is_none() && peer_replace(&peer_cmd_rx).is_none(),
        "the copy must not reach the peer attachment"
    );
    assert!(
        app.pending_queue_transfer.is_none() && app.staged_queue_transfer.is_none(),
        "the refused Take arms nothing"
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert!(
        app.status.contains("the local Owner connection was lost"),
        "the refusal names the lost local Owner link, got {:?}",
        app.status
    );
}

#[test]
fn a_probe_link_lost_during_the_confirm_names_the_peer() {
    // Review P2 (row 8.1): the dead link is the peer's probe — the report
    // must name the peer, not the local Owner the settlement's own link
    // loss names.
    let _guard = crate::app::DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    let (mut app, local_cmd_rx) =
        make_local_daemon_app_stub_with_cmd_rx_and_compat(make_audio_items(2), owner_compat());
    let (mut probe, _stub_rx, probe_cmd_rx) =
        RemotePlayer::stub_with_command_rx(make_audio_items(3), 1);
    probe.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    probe.set_disconnected_for_test(true);
    let (_dropped_tx, probe_event_rx) = mpsc::channel();
    *PROBE_LINK.lock().unwrap() = Some((probe, probe_event_rx));
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(staged_probe_connect);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(peer_session("peer-owner")))];

    app.begin_queue_transfer(QueueTransferDirection::Take, &transfer_key());

    assert!(
        matches!(app.pending_overlay, Some(OverlayRequest::Confirm(_))),
        "the staged Take raised the confirm prompt"
    );
    app.pending_overlay = None;
    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(transfer_key()),
        confirm_key(KeyCode::Enter),
    );

    assert!(
        app.status.contains("the peer connection was lost"),
        "the report names the dead probe link, got {:?}",
        app.status
    );
    assert!(
        local_replace(&local_cmd_rx).is_none() && peer_replace(&probe_cmd_rx).is_none(),
        "nothing is sent to either owner"
    );
    assert!(
        app.pending_queue_transfer.is_none() && app.staged_queue_transfer.is_none(),
        "the dead probe arms nothing"
    );
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
}
