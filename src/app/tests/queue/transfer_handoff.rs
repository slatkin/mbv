//! Row 7.2 (design D1/D3/D4): the Client-run Hand off sequence, mocked on
//! both links. Every test owns one `queue-handoff` contract; receivers are
//! injected and answers are pre-pushed, so no test sleeps.

use std::sync::Mutex;
use std::sync::mpsc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::panel_targets::PanelTarget;
use crate::app::state::queue_transfer::QueueTransferDirection;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{make_audio_items, make_items, make_local_daemon_app_stub_with_cmd_rx};
use mbv_ctrl::player::{PlayerEvent, PlayerStatus};
use mbv_ctrl::{
    CTRL_PROTOCOL_VERSION, CtrlCmd, CtrlCompatibility, QueueOpId, QueueOpOutcome, UnifiedQueueSlot,
    UnifiedQueueStateData,
};
use mbv_emby_model::EmbyItem;
use mbv_queue::{QueueItem, QueueLineage, QueueSource};
use mbv_remote_player::{DaemonEndpoint, RemotePlayer, RemotePlayerError};
use mbv_ui_model::confirm::ConfirmAction;
use mbv_ui_model::overlay::OverlayRequest;
use mbv_ui_model::panel_targets::SessionTargetKey;

/// The peer link `staged_probe_connect` hands out, staged by the test that
/// installed the override (nextest runs one test per process; the lock guard
/// keeps plain `cargo test` honest too).
pub(super) static PROBE_LINK: Mutex<Option<(RemotePlayer, mpsc::Receiver<PlayerEvent>)>> =
    Mutex::new(None);

/// The injected direct connect for the unconnected Hand off tests: hands back
/// the probe link the test staged, so the test holds the peer's command
/// receiver while the flow takes the link itself.
pub(super) fn staged_probe_connect(
    _endpoint: &DaemonEndpoint,
) -> Result<(RemotePlayer, mpsc::Receiver<PlayerEvent>), RemotePlayerError> {
    PROBE_LINK
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| RemotePlayerError::owner_build_mismatch_for_test("test", "test-build"))
}

/// An F3 peer row that advertises a direct Player-owner endpoint (design D6).
/// The port is never dialed: the unconnected tests inject the connect.
pub(super) fn peer_session(id: &str) -> mbv_emby::SessionInfo {
    let mut sess = mbv_emby::test_support::make_session(id, "mbv");
    sess.id = id.to_string();
    sess.device_id = format!("device-{id}");
    sess.host = "127.0.0.1".to_string();
    sess.supported_commands = vec![mbv_emby::mbv_direct_tcp_port_command(47788)];
    sess
}

pub(super) fn peer_compat(
    answered: bool,
    start_position: bool,
    audio_only: bool,
) -> CtrlCompatibility {
    let mut compat = CtrlCompatibility::for_peer(CTRL_PROTOCOL_VERSION)
        .expect("local ctrl protocol version is compatible");
    compat.supports_answered_queue_ops = answered;
    compat.supports_queue_replace_start_position = start_position;
    compat.supports_audio_only = audio_only;
    compat
}

/// The peer's post-replacement answer: the submitted slot ids (1..=n) are
/// kept (design D8), `active_index` names the observed active slot, and
/// `playing` decides the snapshot's audible status.
pub(super) fn peer_snapshot(
    items: &[EmbyItem],
    active_index: usize,
    playing: bool,
) -> UnifiedQueueStateData {
    UnifiedQueueStateData {
        status: PlayerStatus {
            active: playing,
            paused: !playing,
            ..PlayerStatus::default()
        },
        active_slot: (active_index < items.len()).then(|| (active_index + 1) as u64),
        slots: items
            .iter()
            .enumerate()
            .map(|(index, item)| UnifiedQueueSlot {
                slot_id: (index + 1) as u64,
                item: QueueItem::Emby(Box::new(item.clone())),
            })
            .collect(),
        revision: 1,
        source: QueueSource::Album,
        lineage: QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    }
}

pub(super) fn applied(op: u64, snapshot: UnifiedQueueStateData) -> PlayerEvent {
    PlayerEvent::QueueOpResult {
        op: QueueOpId(op),
        outcome: QueueOpOutcome::Applied(Box::new(snapshot)),
    }
}

pub(super) fn hand_off_key() -> SessionTargetKey {
    SessionTargetKey::Emby("peer-owner".to_string())
}

pub(super) fn confirm_key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A local-daemon app whose local Owner plays `items`, plus the local Owner's
/// command receiver. The stub starts audible on slot 1, so the Hand off
/// source gate holds unless a test moves it.
fn hand_off_source_app(items: Vec<EmbyItem>) -> (App, mpsc::Receiver<CtrlCmd>) {
    let (app, local_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(items);
    app.player
        .update_status(|status| status.position_ticks = 75_400_000);
    while local_cmd_rx.try_recv().is_ok() {}
    (app, local_cmd_rx)
}

/// A local-daemon app already connected to a peer stub: the connected Hand
/// off shape (design D1). Returns the local and peer command receivers and
/// the peer event sender, so a test can pre-push the replacement's answer and
/// later events. `peer_items` seeds the peer queue: empty skips the confirm
/// gate, populated raises it.
fn connected_hand_off_app(
    compat: CtrlCompatibility,
    local_items: Vec<EmbyItem>,
    peer_items: Vec<EmbyItem>,
) -> (
    App,
    mpsc::Receiver<CtrlCmd>,
    mpsc::Receiver<CtrlCmd>,
    mpsc::Sender<PlayerEvent>,
) {
    let (mut app, local_cmd_rx) = hand_off_source_app(local_items);
    let sess = peer_session("peer-owner");
    let (mut peer, peer_rx, peer_cmd_rx) = RemotePlayer::stub_with_command_rx(peer_items, 0);
    peer.set_ctrl_compatibility_for_test(compat);
    app.switch_to_direct_remote(
        &sess,
        peer,
        peer_rx,
        &DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
    );
    app.panel_targets = vec![PanelTarget::Emby(Box::new(sess))];
    let (peer_tx, peer_rx) = mpsc::channel();
    app.player_rx = peer_rx;
    while peer_cmd_rx.try_recv().is_ok() {}
    (app, local_cmd_rx, peer_cmd_rx, peer_tx)
}

/// The `UnifiedQueueReplace` the peer received, if any.
pub(super) fn peer_replace(peer_cmd_rx: &mpsc::Receiver<CtrlCmd>) -> Option<CtrlCmd> {
    let mut replace = None;
    while let Ok(cmd) = peer_cmd_rx.try_recv() {
        if matches!(cmd, CtrlCmd::UnifiedQueueReplace { .. }) {
            replace = Some(cmd);
        }
    }
    replace
}

pub(super) fn stop_sent_to(local_cmd_rx: &mpsc::Receiver<CtrlCmd>) -> bool {
    let mut stopped = false;
    while let Ok(cmd) = local_cmd_rx.try_recv() {
        stopped |= matches!(
            cmd,
            CtrlCmd::PlaybackIntent(intent) if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
        );
    }
    stopped
}

#[test]
fn hand_off_confirms_then_attaches_and_stops_local_only_after_the_peer_observes() {
    // queue-handoff "Hand off copies the local queue to the peer and moves
    // playback there" + "Replacing a populated target queue asks first" +
    // "Hand off connects the Client to the peer": the unconnected flow opens
    // the peer link to read its queue, asks, attaches on confirm, and stops
    // local only in the tick that observes the start slot playing.
    let _guard = crate::app::DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    let items = make_audio_items(3);
    let (mut app, local_cmd_rx) = hand_off_source_app(items.clone());
    let (mut probe, _stub_rx, peer_cmd_rx) =
        RemotePlayer::stub_with_command_rx(make_audio_items(3), 0);
    probe.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    // The stub's own event channel has no live sender; swap in one the test
    // pre-pushes the replacement's answer onto, so the answer pump sees it.
    let (answer_tx, answer_rx) = mpsc::channel();
    answer_tx
        .send(applied(1, peer_snapshot(&items, 0, true)))
        .unwrap();
    *PROBE_LINK.lock().unwrap() = Some((probe, answer_rx));
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(staged_probe_connect);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(peer_session("peer-owner")))];

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    // The populated peer queue raised the confirm prompt and nothing was sent.
    let modal = match &app.pending_overlay {
        Some(OverlayRequest::Confirm(modal)) => modal,
        other => panic!("a populated peer queue must raise the confirm prompt, got {other:?}"),
    };
    assert!(
        modal.message.contains("peer-owner"),
        "the prompt names the target, got {:?}",
        modal.message
    );
    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "nothing is sent before the user confirms"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "local keeps playing");
    // Mirror the shell's modal dismissal before the confirm answer.
    app.pending_overlay = None;

    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(hand_off_key()),
        confirm_key(KeyCode::Enter),
    );

    // The peer link is attached (as by Connect) and carries the copy: the
    // local queue's slots, the active slot's index, and the local position.
    assert!(app.direct_remote_connected, "Hand off ends connected");
    assert!(
        app.has_remote_queue(),
        "the queue panel shows the peer queue"
    );
    let Some(CtrlCmd::UnifiedQueueReplace {
        slots,
        start_idx,
        start_position_secs,
        op,
        ..
    }) = peer_replace(&peer_cmd_rx)
    else {
        panic!("the confirmed Hand off sends the copy, got no replace");
    };
    assert_eq!(
        slots.iter().map(|slot| slot.slot_id).collect::<Vec<_>>(),
        vec![1, 2, 3],
        "the copy carries the local queue's submitted slot ids"
    );
    assert_eq!(slots.len(), 3, "the copy carries the whole local queue");
    assert_eq!(
        start_idx,
        Some(0),
        "the copy starts at the local active slot"
    );
    assert_eq!(
        start_position_secs,
        Some(mbv_emby_model::ticks_to_seconds(75_400_000)),
        "the copy carries the local playback position"
    );
    assert!(op.is_some(), "the copy is an answered owner operation");
    assert!(
        app.pending_queue_transfer.is_some(),
        "the pending transfer arms after the copy is sent"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "local still plays");

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.pending_queue_transfer.is_none(), "the transfer settles");
    assert!(
        stop_sent_to(&local_cmd_rx),
        "local stops only after the peer observed the start slot"
    );
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn cancelling_the_confirm_drops_the_opened_link_and_sends_nothing() {
    // queue-handoff "Replacing a populated target queue asks first",
    // scenario "Peer already holds a queue": cancel leaves both queues and
    // both playbacks unchanged, and the link opened to read the peer queue
    // is dropped — the Client stays unconnected.
    let _guard = crate::app::DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    let (mut app, local_cmd_rx) = hand_off_source_app(make_audio_items(2));
    let (mut probe, _stub_rx, peer_cmd_rx) =
        RemotePlayer::stub_with_command_rx(make_audio_items(1), 0);
    probe.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    let (answer_tx, answer_rx) = mpsc::channel();
    answer_tx
        .send(applied(1, peer_snapshot(&make_audio_items(1), 0, true)))
        .unwrap();
    *PROBE_LINK.lock().unwrap() = Some((probe, answer_rx));
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(staged_probe_connect);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(peer_session("peer-owner")))];

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    assert!(
        matches!(app.pending_overlay, Some(OverlayRequest::Confirm(_))),
        "the populated peer queue asks first"
    );
    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(hand_off_key()),
        confirm_key(KeyCode::Esc),
    );

    assert!(
        app.staged_queue_transfer.is_none(),
        "the staged link is dropped"
    );
    assert!(
        !app.direct_remote_connected && !app.has_remote_queue(),
        "cancel leaves the Client unconnected"
    );
    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "cancel sends nothing to the peer"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "local keeps playing");
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn an_audio_only_peer_refuses_a_video_active_item_and_sends_nothing() {
    // queue-handoff "A failed Hand off or Take leaves the source playing",
    // scenario "Audio-only peer cannot play the active video" (design D3
    // admission pre-check): the copy is refused before it is sent.
    let (app, local_cmd_rx, peer_cmd_rx, _peer_tx) = connected_hand_off_app(
        peer_compat(true, true, true),
        make_items(2)
            .into_iter()
            .map(|mut item| {
                item.media_type = "Video".into();
                item
            })
            .collect(),
        Vec::new(),
    );
    let mut app = app;

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "the refused copy is never sent"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "local playback continues");
    assert!(app.pending_queue_transfer.is_none());
    assert!(app.staged_queue_transfer.is_none());
    assert_eq!(app.status_severity, ToastSeverity::Warning);
    assert!(
        app.status.contains("cannot play the active item"),
        "the refusal names the cause, got {:?}",
        app.status
    );
    assert!(
        !matches!(app.pending_overlay, Some(OverlayRequest::Confirm(_))),
        "a refused Hand off never asks to replace the peer queue"
    );
}

#[test]
fn an_older_peer_without_the_start_position_capability_is_refused() {
    // queue-handoff "Hand off and Take need a peer that accepts a start
    // position", scenario "Older peer": the refusal names the cause and
    // neither queue nor playback changes.
    let (mut app, local_cmd_rx, peer_cmd_rx, _peer_tx) = connected_hand_off_app(
        peer_compat(true, false, false),
        make_audio_items(2),
        Vec::new(),
    );

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "an older peer receives no copy"
    );
    assert!(!stop_sent_to(&local_cmd_rx), "local playback continues");
    assert!(app.pending_queue_transfer.is_none());
    assert!(app.staged_queue_transfer.is_none());
    assert!(
        app.status.contains("does not support Hand off"),
        "the refusal names the cause, got {:?}",
        app.status
    );
}

#[test]
fn a_peer_link_lost_before_observation_leaves_local_playing() {
    // queue-handoff "A failed Hand off or Take leaves the source playing",
    // scenario "Connection lost during Hand off": the copy was answered, the
    // peer dropped before reporting playback, and local playback continues.
    let (mut app, local_cmd_rx, _peer_cmd_rx, peer_tx) = connected_hand_off_app(
        peer_compat(true, true, false),
        make_audio_items(2),
        Vec::new(),
    );
    // The answer carries the start slot still queued but not yet audible, so
    // the armed transfer keeps waiting instead of settling at once.
    peer_tx
        .send(applied(1, peer_snapshot(&make_audio_items(2), 0, false)))
        .unwrap();

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    assert!(
        app.pending_queue_transfer.is_some(),
        "the answered copy arms the pending transfer"
    );
    peer_tx
        .send(PlayerEvent::RemoteDisconnected("peer gone".into()))
        .unwrap();

    let mut harness = TickHarness::new(app);
    harness.step();

    let app = &harness.model().app;
    assert!(app.pending_queue_transfer.is_none(), "the transfer reports");
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert!(
        app.status.starts_with("Hand off failed"),
        "the failure is reported, got {:?}",
        app.status
    );
    assert!(
        !stop_sent_to(&local_cmd_rx),
        "the lost peer never stops the source"
    );
    assert!(
        app.local_owner_plays(),
        "local playback continues after the peer link is lost"
    );
}

/// Every `CtrlCmd` the local Owner received, drained for inspection.
fn local_owner_commands(local_cmd_rx: &mpsc::Receiver<CtrlCmd>) -> Vec<CtrlCmd> {
    let mut commands = Vec::new();
    while let Ok(command) = local_cmd_rx.try_recv() {
        commands.push(command);
    }
    commands
}

fn hand_back_sent(commands: &[CtrlCmd]) -> bool {
    commands
        .iter()
        .any(|command| matches!(command, CtrlCmd::Stop | CtrlCmd::SelectPlaybackOutput(_)))
}

#[test]
fn an_unconnected_hand_off_never_stops_the_local_owners_remote_output() {
    // Review P1 (unit G, commit 654eb9f89): the attach's sever must not run
    // the Owner-output hand-back — the local Owner may be the transfer source
    // playing through a Cast or Emby Session output, and its Stop must wait
    // for the settlement's observation, not ride the attach.
    let _guard = crate::app::DIRECT_CONNECT_TEST_LOCK.lock().unwrap();
    let items = make_audio_items(3);
    let (mut app, local_cmd_rx) = hand_off_source_app(items.clone());
    app.player
        .remote()
        .update_playback_output_for_test(mbv_ctrl::OutputState::Cast {
            receiver_id: "r-1".to_string(),
            name: "Living Room".to_string(),
            connection: mbv_ctrl::OutputConnection::Connected,
        });
    let (mut probe, _stub_rx, _peer_cmd_rx) = RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    probe.set_ctrl_compatibility_for_test(peer_compat(true, true, false));
    let (answer_tx, answer_rx) = mpsc::channel();
    answer_tx
        .send(applied(1, peer_snapshot(&items, 0, true)))
        .unwrap();
    *PROBE_LINK.lock().unwrap() = Some((probe, answer_rx));
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = Some(staged_probe_connect);
    app.panel_targets = vec![PanelTarget::Emby(Box::new(peer_session("peer-owner")))];

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    let sent_at_attach = local_owner_commands(&local_cmd_rx);
    assert!(
        !hand_back_sent(&sent_at_attach),
        "the attach must not stop the source or hand its output back, got {sent_at_attach:?}"
    );

    let mut harness = TickHarness::new(app);
    harness.step();

    let sent_at_settle = local_owner_commands(&local_cmd_rx);
    assert!(
        !hand_back_sent(&sent_at_settle),
        "no Owner-output hand-back rides the transfer, got {sent_at_settle:?}"
    );
    assert_eq!(
        sent_at_settle
            .iter()
            .filter(|command| matches!(
                command,
                CtrlCmd::PlaybackIntent(intent) if intent.action == mbv_ctrl::PlaybackIntentAction::Stop
            ))
            .count(),
        1,
        "the only local stop is the settlement's source stop"
    );
    *crate::app::DIRECT_CONNECT_OVERRIDE.lock().unwrap() = None;
}

#[test]
fn a_dead_home_link_never_stops_the_peer_at_the_success_settlement() {
    // Review P2 (unit G, commit 654eb9f89): when the suspended home link dies
    // mid-transfer while the peer is attached, the success settlement has no
    // source link left — it must skip the source stop instead of falling
    // through to `self.player`, which is the peer and would kill the
    // just-handed-off playback.
    let (mut app, _local_cmd_rx, peer_cmd_rx, peer_tx) = connected_hand_off_app(
        peer_compat(true, true, false),
        make_audio_items(2),
        Vec::new(),
    );
    // The peer's answer already shows the start slot active and playing, so
    // the very next settlement succeeds.
    peer_tx
        .send(applied(1, peer_snapshot(&make_audio_items(2), 0, true)))
        .unwrap();
    let (home_tx, home_rx) = mpsc::channel();
    app.suspended_local.as_mut().unwrap().player_rx = home_rx;
    home_tx
        .send(PlayerEvent::RemoteDisconnected("home owner lost".into()))
        .unwrap();

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());
    assert!(app.pending_queue_transfer.is_some(), "the copy armed");

    let mut harness = TickHarness::new(app);
    harness.step();

    let model = harness.model();
    let app = &model.app;
    assert!(app.pending_queue_transfer.is_none(), "the transfer settles");
    assert!(
        !stop_sent_to(&peer_cmd_rx),
        "the settlement must never stop the peer it just handed off to"
    );
    assert!(
        app.has_remote_queue(),
        "the peer keeps the handed-off queue and playback"
    );
    assert!(
        model.application.mounted(&mbv_ui_msg::ComponentId::Modal(
            mbv_ui_msg::ModalId::DaemonLost
        )),
        "the dead home link raised the daemon-lost modal"
    );
}

#[test]
fn a_peer_loss_during_the_hand_off_confirm_drops_the_prompt_and_sends_nothing() {
    // PR #973 review: the connected Hand off's confirm prompt outlived the
    // peer link — Enter then sent the copy through `queue_link(Peer)`,
    // which the disconnect teardown had made the local Owner, replacing the
    // source queue and panicking on the Peer queue view. The teardown must
    // drop the staged transfer and its prompt before any answer can run it.
    let (mut app, local_cmd_rx, _peer_cmd_rx, _peer_tx) = connected_hand_off_app(
        peer_compat(true, true, false),
        make_audio_items(2),
        make_audio_items(3),
    );

    app.begin_queue_transfer(QueueTransferDirection::HandOff, &hand_off_key());

    assert!(
        matches!(app.pending_overlay, Some(OverlayRequest::Confirm(_))),
        "the populated peer queue asks first"
    );
    assert!(
        app.staged_queue_transfer.is_some(),
        "the connected Hand off is staged behind the prompt"
    );

    // The peer link dies while the prompt is up; auto-reconnect is off in
    // the stub config, so the teardown restores the local link.
    app.handle_player_event(PlayerEvent::RemoteDisconnected("peer gone".into()));

    assert!(
        app.staged_queue_transfer.is_none(),
        "the staged transfer dies with the connection"
    );
    assert!(
        matches!(app.pending_overlay, Some(OverlayRequest::DismissConfirm)),
        "the teardown dismisses the transfer prompt, got {:?}",
        app.pending_overlay.as_ref().map(std::mem::discriminant)
    );
    assert!(
        !app.direct_remote_connected && !app.has_remote_queue(),
        "the teardown restored the local link"
    );

    // The prompt's answer is now inert: nothing reaches the local Owner.
    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(hand_off_key()),
        confirm_key(KeyCode::Enter),
    );

    let sent = local_owner_commands(&local_cmd_rx);
    assert!(
        !sent
            .iter()
            .any(|cmd| matches!(cmd, CtrlCmd::UnifiedQueueReplace { .. })),
        "the staged Hand off never reaches the local Owner, got {sent:?}"
    );
    assert!(app.pending_queue_transfer.is_none());
    assert_eq!(
        app.local_view
            .slots()
            .iter()
            .map(|slot| slot.slot_id.raw())
            .collect::<Vec<_>>(),
        vec![1, 2],
        "the source queue is unchanged"
    );
}

#[test]
fn a_confirmed_hand_off_without_a_peer_link_reports_and_sends_nothing() {
    // PR #973 review: `queue_link(Peer)` falls through to `self.player` —
    // the local Owner the disconnect teardown reinstalled — so the copy
    // re-verifies the peer link before it is sent, the same gate the
    // Take's copy applies to its own target link.
    let (mut app, local_cmd_rx, peer_cmd_rx, _peer_tx) = connected_hand_off_app(
        peer_compat(true, true, false),
        make_audio_items(2),
        make_audio_items(3),
    );

    // The peer link dies and the teardown restores the local link.
    app.handle_player_event(PlayerEvent::RemoteDisconnected("peer gone".into()));
    assert!(!app.has_remote_queue());

    // Re-stage the connected shape directly: `link = None` reads the
    // current attachment at the copy, which is now the local Owner.
    app.staged_queue_transfer = Some(crate::app::state::queue_transfer::StagedQueueTransfer {
        direction: QueueTransferDirection::HandOff,
        target: hand_off_key(),
        sess: peer_session("peer-owner"),
        endpoint: DaemonEndpoint::Tcp("127.0.0.1:47788".parse().unwrap()),
        link: None,
    });
    app.apply_confirm_action(
        ConfirmAction::ConfirmQueueTransfer(hand_off_key()),
        confirm_key(KeyCode::Enter),
    );

    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert!(
        app.status
            .contains("Hand off failed: the peer connection was lost"),
        "the loss is reported, got {:?}",
        app.status
    );
    let local_sent = local_owner_commands(&local_cmd_rx);
    assert!(
        !local_sent
            .iter()
            .any(|cmd| matches!(cmd, CtrlCmd::UnifiedQueueReplace { .. })),
        "the copy never reaches the local Owner, got {local_sent:?}"
    );
    assert!(
        peer_replace(&peer_cmd_rx).is_none(),
        "the peer receives nothing"
    );
    assert!(app.pending_queue_transfer.is_none());
    assert_eq!(
        app.local_view
            .slots()
            .iter()
            .map(|slot| slot.slot_id.raw())
            .collect::<Vec<_>>(),
        vec![1, 2],
        "the source queue is unchanged"
    );
}
