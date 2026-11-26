//! The peer link (design D7 of `gui-named-queues`, ADR 0041): what the Queue
//! panel, its transport, its track switches, and its confirmed removals do
//! while a connected peer holds the active link and a local queue is shown.
//!
//! A GUI Client's shown entry picks the shown queue and the player panel even
//! while a peer is connected, so every command these tests check names the
//! shown owner rather than the connection.

use super::*;
use crate::app::tests::route_state::stub_endpoint;

/// A GUI Client connected to a peer shows the local Named queue it picked
/// (ADR 0041: the shown entry, not the connection, picks the shown queue).
#[test]
fn a_gui_client_connected_to_a_peer_shows_a_local_queue() {
    let mut app = app_with_named_queues(make_items(3));
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());
    assert!(app.suspended_local.is_some(), "the local link stays held");
    app.set_client_capabilities(transport_client_capabilities());

    app.show_named_queue(current());

    assert_eq!(app.playing_queue_link(), QueueLink::Local);
    assert_eq!(app.displayed_queue().slots().len(), 3);
}

/// The TUI Client keeps the connection rule (ADR 0034): while a peer is
/// connected it shows the peer's queue, whatever the GUI-only shown entry
/// holds.
#[test]
fn the_tui_link_keeps_the_connection_rule() {
    let mut app = app_with_named_queues(make_items(3));
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(2), 0);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx, &stub_endpoint());
    app.shown_entry = Some(crate::app::state::playback::ShownEntry::Local(current()));

    assert_eq!(app.playing_queue_link(), QueueLink::Peer);
    assert_eq!(app.displayed_queue().slots().len(), 2);
}

/// The pause intent one transport command sent to `link`'s owner.
fn pause_intent(paused: bool) -> impl Fn(&mbv_ctrl::CtrlCmd) -> bool {
    move |cmd| {
        matches!(
            cmd,
            mbv_ctrl::CtrlCmd::PlaybackIntent(mbv_ctrl::PlaybackIntent {
                action: mbv_ctrl::PlaybackIntentAction::SetPaused { paused: sent },
                ..
            }) if *sent == paused
        )
    }
}

/// The local Owner's answered status position, distinct from both links'
/// starting positions: a status write that lands on the wrong proxy is visible.
const ANSWERED_POSITION: i64 = 99;

/// A GUI Client that shows a local Named queue reads the local Owner's own
/// status: the suspended home link's snapshot fills its proxy, and the peer's
/// mirror never takes the local Owner's playback (design D7, spec
/// `gui-queue-chooser` "The player panel follows the shown entry's owner").
#[test]
fn a_local_edit_answer_updates_the_local_owner_status_and_not_the_peer() {
    let (mut app, _home_cmd_rx, _peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(current());
    let tx = inject_home_events(&mut app);
    let mut answered = emby_unified_state(&make_items(3), 0);
    answered.status.position_ticks = ANSWERED_POSITION;
    tx.send(op_answer(1, answered)).unwrap();

    let adopted = app.await_queue_op_with_bound(
        QueueLink::Local,
        mbv_ctrl::QueueOpId(1),
        std::time::Duration::from_secs(1),
    );

    assert!(adopted, "the local Owner's answer applies");
    assert_eq!(
        app.shown_playback_status().position_ticks,
        ANSWERED_POSITION,
        "the panel reads the local Owner's own status"
    );
    assert_eq!(
        app.player.status_snapshot().position_ticks,
        PEER_POSITION,
        "the peer's mirror never takes the local Owner's status"
    );
}

/// A peer that publishes its own queue while a local queue is shown fills the
/// peer's view only: the shown local rows and their source stay the local
/// Owner's (design D7, ADR 0041).
#[test]
fn a_peer_snapshot_never_fills_the_shown_local_queue() {
    let (mut app, _home_cmd_rx, _peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(current());
    app.local_view.adopt_source(mbv_queue::QueueSource::Album);

    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(
        emby_unified_state(&make_items(1), 0),
    )));

    assert_eq!(
        app.local_view.slots().len(),
        3,
        "the shown local queue keeps its own rows"
    );
    assert_eq!(
        app.queue_for_link(QueueLink::Peer).slots().len(),
        1,
        "the peer's queue takes the peer's snapshot"
    );
    assert_eq!(
        app.local_view.source(),
        &mbv_queue::QueueSource::Album,
        "the peer's source never replaces the local Owner's"
    );
}

/// A resting local queue shown while the local Owner plays: the panel and the
/// transport follow the local Owner, so pause pauses its playback and the
/// connected peer's playback is untouched (design D7, spec
/// `gui-queue-chooser` "A resting queue is shown while another plays").
#[test]
fn a_shown_resting_queue_pauses_the_local_owner_not_the_peer() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(resting());
    assert!(
        matches!(
            home_cmd_rx.try_recv(),
            Ok(mbv_ctrl::CtrlCmd::NamedQueueShow { .. })
        ),
        "the show reaches the local Owner's link"
    );

    assert_eq!(app.effective_playback_state().position_ticks, HOME_POSITION);
    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(
        pause_intent(true)(&home_cmd_rx.try_recv().unwrap()),
        "pause reaches the local Owner's playback"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the connect's peer keeps playing"
    );
}

/// The peer entry shown: the panel and the transport follow the peer, so the
/// panel reports the peer's position and pause reaches it (design D7, spec
/// `gui-queue-chooser` "A peer entry is shown").
#[test]
fn a_shown_peer_entry_follows_the_peer_playback() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    assert_eq!(app.playing_queue_link(), QueueLink::Peer);

    assert_eq!(app.effective_playback_state().position_ticks, PEER_POSITION);
    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(
        pause_intent(true)(&peer_cmd_rx.try_recv().unwrap()),
        "pause reaches the shown peer's playback"
    );
    assert!(
        home_cmd_rx.try_recv().is_err(),
        "the local Owner is not the shown entry"
    );
}

/// Whether `cmd` is the standard stop playback intent.
fn stop_intent(cmd: &mbv_ctrl::CtrlCmd) -> bool {
    matches!(
        cmd,
        mbv_ctrl::CtrlCmd::PlaybackIntent(mbv_ctrl::PlaybackIntent {
            action: mbv_ctrl::PlaybackIntentAction::Stop,
            ..
        })
    )
}

/// The confirming answer to a shared confirmation prompt.
fn confirm_key() -> mbv_ui_msg::KeyEvent {
    mbv_ui_msg::KeyEvent::new(mbv_ui_msg::Key::Enter, mbv_ui_msg::KeyModifiers::NONE)
}

/// A confirmed removal of the now-playing row stops the shown owner, never the
/// active one: with a local queue shown beside a connected peer, the stop
/// reaches the local Owner, the peer keeps playing, and the local Owner's
/// `Stopped` is what completes the deferred removal (design D7).
#[test]
fn confirming_the_now_playing_local_row_stops_the_local_owner_not_the_peer() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(current());
    drain_commands(&home_cmd_rx);
    let tx = inject_home_events(&mut app);

    app.apply_confirm_action(
        crate::app::ConfirmAction::RemoveActiveQueueItem(0),
        confirm_key(),
    );

    assert!(
        matches!(home_cmd_rx.try_recv(), Ok(cmd) if stop_intent(&cmd)),
        "the shown owner stops"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the peer's playback is not the shown entry's"
    );
    assert!(
        app.pending_delete_slot.is_some(),
        "the removal waits for the shown owner's stop"
    );

    let local_slot = app
        .local_view
        .slot_id_at(0)
        .expect("the local queue holds the row");
    tx.send(PlayerEvent::Stopped {
        slot_id: Some(local_slot),
        run_identity: 0,
        position_ticks: 0,
        played: false,
        consume: false,
        error: None,
    })
    .unwrap();

    app.drain_home_event();

    assert!(
        app.pending_delete_slot.is_none(),
        "the local Owner's stop completes the removal"
    );
    let sent = home_cmd_rx
        .try_recv()
        .expect("the removal reaches the local Owner");
    assert!(
        matches!(
            sent,
            mbv_ctrl::CtrlCmd::UnifiedQueueRemoveSlot {
                slot_id,
                queue: None,
                ..
            } if slot_id == mbv_ctrl::slot_id_to_u64(local_slot)
        ),
        "the removal names the local Owner's row: {sent:?}"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the peer's queue is not the one the removal names"
    );
}

/// The multi-select variant stops the shown owner too, and the slots it will
/// remove wait for that owner's `Stopped` (design D7).
#[test]
fn confirming_the_now_playing_local_selection_stops_the_local_owner_not_the_peer() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(current());
    drain_commands(&home_cmd_rx);
    let slot_ids = vec![
        app.local_view
            .slot_id_at(0)
            .expect("the local queue holds the row"),
    ];

    app.apply_confirm_action(
        crate::app::ConfirmAction::RemoveActiveQueueSelection {
            link: QueueLink::Local,
            slot_ids: slot_ids.clone(),
        },
        confirm_key(),
    );

    assert!(
        matches!(home_cmd_rx.try_recv(), Ok(cmd) if stop_intent(&cmd)),
        "the shown owner stops"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the peer's playback is not the shown entry's"
    );
    assert_eq!(
        app.pending_delete_selection.as_deref(),
        Some(slot_ids.as_slice()),
        "the selection waits for the shown owner's stop"
    );
}

/// The track chips read the shown owner's status, so a chosen track reaches
/// that owner: with a local queue shown beside a connected peer, both track
/// switches reach the local Owner (design D7 of `gui-named-queues`).
#[test]
fn a_chosen_track_reaches_the_shown_owner_not_the_active_link() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(current());

    app.set_audio_track(3);
    app.set_subtitle_track(4);

    assert!(
        matches!(
            home_cmd_rx.try_recv(),
            Ok(mbv_ctrl::CtrlCmd::PlayerCmd(
                mbv_ctrl::WireCommand::SetAudio(3)
            ))
        ),
        "the audio track switch reaches the shown owner"
    );
    assert!(
        matches!(
            home_cmd_rx.try_recv(),
            Ok(mbv_ctrl::CtrlCmd::PlayerCmd(mbv_ctrl::WireCommand::SetSub(
                4
            )))
        ),
        "the subtitle track switch reaches the shown owner"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the peer's tracks are not the shown entry's"
    );
}

/// A replacement that starts a shown resting local queue mutes the owner that
/// starts playing it: the local Owner, never a connected peer (design D7 of
/// `gui-named-queues`).
#[test]
fn a_resting_queue_play_mutes_the_shown_local_owner_not_the_peer() {
    let (mut app, home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    app.show_named_queue(resting());
    drain_commands(&home_cmd_rx);
    app.mute_on = true;

    app.execute_pending_queue_action(replacement_of(&make_items(2)));

    assert_eq!(
        sent_replace_queue(
            &home_cmd_rx
                .try_recv()
                .expect("the replacement reaches the shown owner")
        ),
        Some(resting()),
        "the replacement names the shown resting queue"
    );
    assert!(
        matches!(
            home_cmd_rx.try_recv(),
            Ok(mbv_ctrl::CtrlCmd::PlayerCmd(
                mbv_ctrl::WireCommand::SetMute(true)
            ))
        ),
        "the owner that starts playing is the one muted"
    );
    assert!(
        peer_cmd_rx.try_recv().is_err(),
        "the peer is not the queue the replacement started"
    );
}

/// The TUI Client keeps the connection rule for the panel and the transport
/// too: with a peer connected, its status is the panel's and its playback is
/// what the transport controls, and the connect records no shown entry.
#[test]
fn the_tui_panel_and_transport_keep_the_connection_rule() {
    let (mut app, home_cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    while home_cmd_rx.try_recv().is_ok() {}
    let (peer, peer_rx, peer_cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(make_items(2), 1);
    let sess = make_session("peer-host", "mbv");
    app.switch_to_direct_remote(&sess, peer, peer_rx, &stub_endpoint());
    app.player
        .update_status(|status| status.position_ticks = PEER_POSITION);
    while peer_cmd_rx.try_recv().is_ok() {}

    assert!(app.shown_entry.is_none(), "a TUI Client records no entry");
    assert_eq!(app.effective_playback_state().position_ticks, PEER_POSITION);
    LocalPlaybackTarget::toggle_play_pause(&mut app);

    assert!(pause_intent(true)(&peer_cmd_rx.try_recv().unwrap()));
    assert!(
        home_cmd_rx.try_recv().is_err(),
        "the local Owner is not the connection's playback target"
    );
}

/// A lost connection removes the peer entry and shows the current local queue,
/// with the entry-change reset: the filter clears and the active row returns to
/// the shown queue's start slot (spec `gui-queue-chooser` "A connect adds a peer
/// entry" — "The connection is lost").
#[test]
fn the_connection_is_lost_shows_the_current_queue() {
    let (mut app, home_cmd_rx, _peer_cmd_rx) = app_with_connected_peer();
    while home_cmd_rx.try_recv().is_ok() {}
    adopt_current_queue(&mut app, &queue_with_start(&make_items(3), 1));
    app.queue_filter = "kid".to_string();
    app.queue_filter_active = true;

    app.restore_local_mode("lost");

    assert_eq!(
        app.shown_entry,
        Some(ShownEntry::Local(current())),
        "the queue panel returns to the current local queue"
    );
    assert!(
        app.queue_filter.is_empty() && !app.queue_filter_active,
        "the Queue filter closes and empties"
    );
    let projection = app.queue_surface_projection();
    assert!(
        projection.presentation_reset,
        "the Queue Surface drops its marks"
    );
    assert!(
        matches!(projection.cursor, QueueCursorUpdate::Set(1)),
        "the active row rests on the shown queue's start slot"
    );
}

/// A close never warns on a playback the local queues do not own. With the
/// local Owner's link gone — a lost daemon — the peer alone plays, so closing
/// the stale local current queue asks nothing and reaches no owner at all: the
/// prompt and the close used to land on the peer through the lost link's own
/// stand-in (spec `gui-queue-chooser` "The close control closes the shown
/// entry": confirmation is for the current queue while it plays, and only a
/// local queue's close is ever confirmed).
#[test]
fn a_close_behind_a_lost_local_link_asks_nothing_and_reaches_no_peer() {
    let (mut app, _home_cmd_rx, peer_cmd_rx) = app_with_connected_peer();
    app.suspended_local = None;
    app.show_local_current_entry();
    app.clear_pending_overlay_for_test();

    app.request_close_shown_queue();

    assert!(
        app.pending_overlay_is_none_for_test(),
        "the peer's playback arms no stop warning for a local queue's close"
    );
    assert!(
        peer_cmd_rx.try_iter().next().is_none(),
        "no local queue command reaches the peer"
    );
}
