use super::*;
use crate::app::tests::{make_app_stub, make_items, make_remote_app_stub_with_cmd_rx};
use mbv_core::player::PlayerEvent;
use std::time::{Duration, Instant};

/// Row 1.3: the Next-Up accept on an out-of-process owner requests the jump
/// from the owner (`CtrlCmd::UnifiedQueuePlaySlot`), never constructs a local
/// `JumpTo`, leaves the client cursor on the owner snapshot (A3), and the
/// client keeps running.
#[test]
fn next_up_accept_on_out_of_process_owner_requests_unified_queue_play_slot() {
    let _guard = crate::config::TestStateDirGuard::new();
    let remote_items = make_items(3);
    let (mut app, cmd_rx) =
        make_remote_app_stub_with_cmd_rx(Vec::new(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 3;
    }
    app.next_up_item = Some(remote_items[1].clone());

    app.handle_player_event(PlayerEvent::NextUpPlay);

    let expected_slot = app.playback_queue().slots()[1].slot_id;
    assert!(
        matches!(
            cmd_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("accept must request the jump from the out-of-process owner"),
            mbv_core::ctrl::CtrlCmd::UnifiedQueuePlaySlot { slot_id }
                if slot_id == mbv_core::ctrl::slot_id_to_u64(expected_slot)
        ),
        "accept must request UnifiedQueuePlaySlot for the next-up slot"
    );
    assert!(
        cmd_rx.try_recv().is_err(),
        "no local JumpTo may be constructed for an out-of-process owner"
    );
    // A3: the client cursor stays on the owner snapshot; no optimistic write.
    assert_eq!(app.playback_queue().queue_cursor, 0);
    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().queue_cursor,
        0,
        "cursor must follow the owner snapshot, not the requested slot"
    );
    // The client survived the accept and the accept consumed the item.
    assert!(app.next_up_item.is_none());
}

/// Spec scenario: the app-process owner resolves the Next-Up accept jump
/// locally — mint/accept then the local `JumpTo`, plus the optimistic cursor.
#[test]
fn next_up_accept_on_app_process_owner_mints_and_dispatches_local_jump() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(3), 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 3;
    }
    let items = make_items(3);
    app.next_up_item = Some(items[2].clone());

    let commands = app.player.spy_on_commands();
    app.handle_player_event(PlayerEvent::NextUpPlay);

    let target = app.player_tab.slot_id_at(2).unwrap();
    assert!(
        matches!(
            commands.try_recv().expect("local owner must dispatch the jump"),
            mbv_core::player::PlayerCommand::JumpTo { slot_id, .. } if slot_id == target
        ),
        "the accept must jump to the next-up slot by identity"
    );
    assert!(commands.try_recv().is_err());
    assert_eq!(
        app.bare_owner.in_flight_transition_slot(),
        Some(target),
        "the minted transition must be in flight with the owner state"
    );
    assert_eq!(app.playback_queue().queue_cursor, 2);
}

/// Row 1.2 (expire): the promoted transition is dispatched as-is — the jump
/// that reaches the Playback run carries the queued transition's identity and
/// target, with no re-mint or re-accept.
#[test]
fn expire_dispatches_the_promoted_transition_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(2), 0);
    let slot_a = app.player_tab.slot_id_at(0).unwrap();
    let slot_b = app.player_tab.slot_id_at(1).unwrap();
    let (request_id_a, generation_a) = app.bare_owner.mint_local_transition();
    app.bare_owner.accept_local_transition(mbv_core::playback_transition::Transition::new(
        request_id_a, generation_a, slot_a,
    ));
    let (request_id_b, generation_b) = app.bare_owner.mint_local_transition();
    app.bare_owner.accept_local_transition(mbv_core::playback_transition::Transition::new(
        request_id_b, generation_b, slot_b,
    ));

    let commands = app.player.spy_on_commands();
    // First tick arms the in-flight deadline; the second passes it.
    assert!(!app.expire_bare_transition(Instant::now()));
    assert!(app.expire_bare_transition(Instant::now() + Duration::from_secs(6)));

    assert!(matches!(
        commands.try_recv().expect("the promoted transition must be dispatched"),
        mbv_core::player::PlayerCommand::JumpTo { slot_id, request_id, generation }
            if slot_id == slot_b && request_id == request_id_b && generation == generation_b
    ));
    assert!(commands.try_recv().is_err());
    assert_eq!(
        app.bare_owner.in_flight_transition_slot(),
        Some(slot_b),
        "the promoted transition — not a re-minted one — is in flight"
    );
}

/// Row 1.2 (settle): the queued transition promoted by settlement is
/// dispatched with its own identity, never re-minted or re-accepted.
#[test]
fn settle_dispatches_the_promoted_transition_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(2), 0);
    let slot_a = app.player_tab.slot_id_at(0).unwrap();
    let slot_b = app.player_tab.slot_id_at(1).unwrap();
    let (request_id_a, generation_a) = app.bare_owner.mint_local_transition();
    app.bare_owner.accept_local_transition(mbv_core::playback_transition::Transition::new(
        request_id_a, generation_a, slot_a,
    ));
    let (request_id_b, generation_b) = app.bare_owner.mint_local_transition();
    app.bare_owner.accept_local_transition(mbv_core::playback_transition::Transition::new(
        request_id_b, generation_b, slot_b,
    ));

    let commands = app.player.spy_on_commands();
    app.handle_player_event(PlayerEvent::TrackChanged {
        slot_id: slot_a,
        transition: Some((request_id_a, generation_a)),
    });

    assert!(matches!(
        commands.try_recv().expect("the promoted transition must be dispatched"),
        mbv_core::player::PlayerCommand::JumpTo { slot_id, request_id, generation }
            if slot_id == slot_b && request_id == request_id_b && generation == generation_b
    ));
    assert!(commands.try_recv().is_err());
    assert_eq!(
        app.bare_owner.in_flight_transition_slot(),
        Some(slot_b),
        "the promoted transition — not a re-minted one — is in flight"
    );
}
