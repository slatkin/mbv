// The Shuffle move list reaches the run as per-slot `QueueMove` commands
// (queue-content-shuffle task 3.2, design D3): after the list, the active
// slot sits at index 0, the run's queue mirror matches the owner's shuffled
// order, and the tracked mpv index follows it.

use super::*;
use rstest::rstest;
use std::sync::Mutex;

/// Apply mpv's documented `playlist-move` semantics to a model of its
/// playlist: the entry at `from` is removed and inserted before the entry
/// that was at `mpv_to` in pre-move coordinates.
fn apply_mpv_playlist_move(playlist: &mut Vec<QueueSlotId>, from: usize, mpv_to: usize) {
    let entry = playlist.remove(from);
    let insert_at = if mpv_to > from { mpv_to - 1 } else { mpv_to };
    playlist.insert(insert_at, entry);
}

/// The contract: the playlist-move arguments land the moved entry at the
/// requested index, in both move directions and at the playlist ends (the
/// "paradox" note on `playlist_move_args`). A live mpv handle is not allowed
/// in tests, so the playlist is modeled from the documented semantics the
/// mapping is written against.
#[rstest]
#[case::forward(1, 3)]
#[case::backward(3, 1)]
#[case::first_to_end(0, 3)]
#[case::last_to_front(3, 0)]
fn playlist_move_args_land_the_entry_at_the_requested_index(
    #[case] from: usize,
    #[case] to: usize,
) {
    let mut playlist: Vec<QueueSlotId> = (1..=4).map(QueueSlotId::from_raw).collect();
    let moved = playlist[from];

    let (mpv_from, mpv_to) = playlist_move_args(from, to);
    apply_mpv_playlist_move(&mut playlist, mpv_from, mpv_to);

    assert_eq!(playlist[to], moved);
}

/// The run state after the Shuffle move list reached it. `active_file` is on
/// so the test drives the move bookkeeping without a live mpv handle; the
/// mpv playlist command itself is pinned by the `playlist_move_args` cases,
/// whose arguments mirror `apply_queue_move` one-to-one.
fn run_after_shuffle_move_list(
    start_idx: usize,
) -> (
    PlaybackRun,
    Arc<Mutex<PlayerStatus>>,
    Vec<QueueSlotId>,
    QueueSlotId,
) {
    let (mut run, status) = make_queue_session_for_pos_tests(start_idx);
    run.active_file = true;
    let before: Vec<QueueSlotId> = (0..run.queue_len())
        .map(|idx| run.slot_id_at(idx).unwrap())
        .collect();
    let active = run.active_slot_id().unwrap();
    let mut rest: Vec<QueueSlotId> = before
        .iter()
        .copied()
        .filter(|slot_id| *slot_id != active)
        .collect();
    rest.reverse();
    let target: Vec<QueueSlotId> = std::iter::once(active).chain(rest).collect();

    for (slot_id, to) in mbv_queue::reorder_move_plan(&before, &target) {
        let from = run.queue.slot_index(slot_id).unwrap();
        run.apply_queue_move(slot_id, from, to);
    }
    (run, status, target, active)
}

/// The contract: with the active slot in the middle, the Shuffle move list
/// ends with the active slot at index 0 and the queue mirror at the shuffled
/// order (spec scenario "Shuffle with a playing item").
#[test]
fn shuffle_move_list_with_the_active_slot_in_the_middle_ends_at_index_0() {
    let (run, status, target, active) = run_after_shuffle_move_list(1);

    let after: Vec<QueueSlotId> = (0..run.queue_len())
        .map(|idx| run.slot_id_at(idx).unwrap())
        .collect();
    assert_eq!(after, target);
    assert_eq!(run.active_slot_id(), Some(active));
    assert_eq!(run.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
}

/// The contract: with the active slot at the end, the move list still ends
/// with it at index 0 and the tracked index following it.
#[test]
fn shuffle_move_list_with_the_active_slot_at_the_end_ends_at_index_0() {
    let (run, status, target, active) = run_after_shuffle_move_list(2);

    let after: Vec<QueueSlotId> = (0..run.queue_len())
        .map(|idx| run.slot_id_at(idx).unwrap())
        .collect();
    assert_eq!(after, target);
    assert_eq!(run.active_slot_id(), Some(active));
    assert_eq!(run.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
}
