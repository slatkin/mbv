use crate::app::dispatch::action::Command;
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
    };
    app.bare_owner
        .observe_track_change(app.player_tab.slot_id_at(0).unwrap());

    app.replace_playback_queue(make_audio_items(4), 0);
    let replacement_ids: Vec<_> = app.player_tab.slots().iter().map(|s| s.slot_id).collect();
    let commands = app.player.spy_on_commands();
    app.dispatch(&Command::QueuePlayCursor(3));

    match commands
        .try_recv()
        .expect("replacement must submit its queue")
    {
        PlayerCommand::SubmitQueue { items, start_idx } => {
            assert_eq!(start_idx, 3);
            assert_eq!(
                items.iter().map(|s| s.slot_id).collect::<Vec<_>>(),
                replacement_ids
            );
        }
        _ => panic!("expected SubmitQueue"),
    }
    assert!(
        commands.try_recv().is_err(),
        "a stale generation must not mint JumpTo"
    );

    let target = app.player_tab.slot_id_at(3).unwrap();
    app.bare_owner
        .sync_canonical_queue(app.player_tab.queue.clone());
    let (request_id, generation) = app.bare_owner.mint_local_transition();
    let transition = mbv_core::playback_transition::Transition::new(request_id, generation, target);
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
    assert_eq!(
        commands.try_iter().count(),
        before,
        "expired replacement must not snap to old row"
    );
}

#[test]
fn replaced_queue_while_playing_claims_no_row_until_confirmed() {
    use crate::app::components::media_list::MediaSemanticState;
    use crate::app::components::{ComponentId, QueueComponent};
    use crate::app::tests::tick_integration::harness::TickHarness;

    fn projected_states(harness: &mut TickHarness) -> Vec<MediaSemanticState> {
        harness.model_mut().sync_mounted_surfaces();
        harness
            .model()
            .application
            .get_component(&ComponentId::Queue)
            .and_then(|component| component.as_any().downcast_ref::<QueueComponent>())
            .expect("QueueComponent mounted")
            .projected_row_states()
    }

    let mut app = make_app_stub();
    app.player_tab.set_items(make_audio_items(1), 0);
    let confirmed = app.player_tab.slot_id_at(0).unwrap();
    app.bare_owner.observe_track_change(confirmed);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.queue_len = 1;
    };
    let mut harness = TickHarness::new(app);

    // Replace the queue while the pre-load item plays: no row of the new
    // queue is playing, and the stale run coordinate must not claim one.
    harness
        .model_mut()
        .app
        .replace_playback_queue(make_audio_items(4), 0);
    let states = projected_states(&mut harness);
    assert_eq!(states.len(), 4);
    for (index, state) in states.iter().enumerate() {
        assert!(
            !matches!(state, MediaSemanticState::NowPlaying { .. }),
            "row {index} must not claim now-playing on a stale-generation queue"
        );
    }

    // Explicit play of row 4: the generation fence upgrades the jump to a
    // full submit of the canonical queue at index 3 — no optimistic claim is
    // painted until the owner reports the new track.
    let commands = harness.model().app.player.spy_on_commands();
    harness
        .model_mut()
        .app
        .dispatch(&Command::QueuePlayCursor(3));
    match commands
        .try_recv()
        .expect("the fenced jump upgrades to a submit")
    {
        PlayerCommand::SubmitQueue { start_idx, .. } => assert_eq!(start_idx, 3),
        PlayerCommand::JumpTo { .. } => panic!("a fenced generation must never mint JumpTo"),
        _ => panic!("expected SubmitQueue"),
    }
    let target = harness.model().app.player_tab.slot_id_at(3).unwrap();
    assert_eq!(
        harness.model().app.pending_playback_slot(),
        None,
        "no unconfirmed claim while the run loads the new track"
    );

    // The owner confirms the new track: the claim settles on row 4.
    harness
        .model_mut()
        .app
        .handle_player_event(PlayerEvent::TrackChanged {
            slot_id: target,
            transition: None,
        });
    let states = projected_states(&mut harness);
    for (index, state) in states.iter().enumerate() {
        let claims = matches!(state, MediaSemanticState::NowPlaying { .. });
        if index == 3 {
            assert!(claims, "the pending row 4 claims now-playing");
        } else {
            assert!(!claims, "row {index} must not share the claim");
        }
    }

    let states = projected_states(&mut harness);
    assert!(matches!(&states[3], MediaSemanticState::NowPlaying { .. }));
}
