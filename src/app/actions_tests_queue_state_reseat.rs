use crate::app::action::Command;
use crate::app::tests::{make_app_stub, make_audio_items};
use mbv_core::player::{PlayerCommand, PlayerEvent};
use std::time::{Duration, Instant};

#[test]
fn queue_cursor_on_replaced_generation_submits_new_slots_without_stale_jump() {
    let mut app = make_app_stub();
    app.player_tab.set_items(make_audio_items(1), 0);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    }
    app.bare_owner
        .observe_track_change(app.player_tab.slot_id_at(0).unwrap());

    app.replace_playback_queue(make_audio_items(4), 0);
    let replacement_ids: Vec<_> = app.player_tab.slots().iter().map(|s| s.slot_id).collect();
    let commands = app.player.spy_on_commands();
    app.dispatch(Command::QueuePlayCursor(3));

    match commands.try_recv().expect("replacement must submit its queue") {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert_eq!(start_idx, 3);
            assert_eq!(items.iter().map(|s| s.slot_id).collect::<Vec<_>>(), replacement_ids);
        }
        _ => panic!("expected SubmitQueue"),
    }
    assert!(commands.try_recv().is_err(), "a stale generation must not mint JumpTo");

    let target = app.player_tab.slot_id_at(3).unwrap();
    app.bare_owner
        .sync_canonical_queue(app.player_tab.queue.clone());
    let (request_id, generation) = app.bare_owner.mint_local_transition();
    let transition = mbv_core::playback_transition::Transition::new(
        request_id, generation, target,
    );
    assert!(matches!(
        app.bare_owner.accept_local_transition(transition),
        mbv_core::playback_transition::DispatchDecision::DispatchNow(_)
    ));
    assert_eq!(app.pending_playback_slot(), Some(target));
    app.handle_player_event(PlayerEvent::TrackChanged {
        slot_id: target,
        transition: Some((request_id, generation)),
    });
    assert_eq!(app.pending_playback_slot(), None);
    let before = commands.try_iter().count();
    app.expire_bare_transition(Instant::now() + Duration::from_secs(6));
    assert_eq!(commands.try_iter().count(), before, "expired replacement must not snap to old row");
}

#[test]
fn replacement_slot_identity_wins_over_numeric_collision() {
    let mut app = make_app_stub();
    let mut old = make_audio_items(1);
    old[0].id = "old-content".into();
    app.player_tab.set_items(old, 0);
    let old_slot = app.player_tab.slot_id_at(0).unwrap();
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    }
    let mut replacement = make_audio_items(2);
    replacement[0].id = "new-content".into();
    replacement[1].id = "new-target".into();
    app.replace_playback_queue(replacement, 0);
    let new_slot = app.player_tab.slot_id_at(0).unwrap();
    assert_ne!(old_slot, new_slot);

    let commands = app.player.spy_on_commands();
    app.dispatch(Command::QueuePlayCursor(0));
    assert!(matches!(
        commands.try_recv(),
        Ok(PlayerCommand::SubmitQueue { items, start_idx: 0 })
            if items[0].slot_id == new_slot && items[0].item.id() == "new-content"
    ));
    assert!(commands.try_recv().is_err(), "old occurrence must never be jumped");
}
