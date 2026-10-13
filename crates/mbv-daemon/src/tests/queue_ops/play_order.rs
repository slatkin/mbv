// `CtrlCmd::SetPlayOrder` turns the owner's order, picks a non-active slot,
// forwards the advance decision to the Playback run, and projects the new
// mode into the queue snapshot (queue-shuffle-play-order task 4.1).

use super::*;

#[test]
fn set_play_order_shuffle_forwards_the_picked_advance_and_projects_the_mode() {
    let player = cold_player();
    let cmd_rx = player.spy_on_commands();
    let client = queue_op_client("test-token");
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (client_id, client_rx) = connect_client(&mut registry.lock().unwrap());
    let (reply_tx, _reply_rx) = mpsc::channel();

    let mut owner = owner_with(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
            emby_qi("c", "Video", "Movie"),
        ],
        0,
    );
    let active_slot = owner.core.queue.slots()[0].slot_id;

    run_queue_cmd(
        CtrlCmd::SetPlayOrder(mbv_queue::PlayOrderMode::Shuffle),
        client_id,
        &reply_tx,
        &client,
        &player,
        &mut owner,
        &registry,
    );

    // The run is told to advance to the picked slot, which is never the
    // active slot (design D2).
    match cmd_rx.recv().unwrap() {
        PlayerCommand::SetUpcoming(mbv_ctrl::player::Advance::To(slot)) => {
            assert_ne!(slot, active_slot);
            assert!(owner.core.queue.slot(slot).is_some());
        }
        other => panic!("expected SetUpcoming, got {other:?}"),
    }

    // The next snapshot reports the new order mode (design D5).
    match recv_event(&client_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(state.play_order, mbv_queue::PlayOrderMode::Shuffle);
        }
        other => panic!("expected a queue-state publish, got {other:?}"),
    }
}
