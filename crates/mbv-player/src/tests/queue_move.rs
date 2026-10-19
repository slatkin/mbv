// The Shuffle reaches the run as one `QueueReorder` command carrying the
// full new order (queue-content-shuffle tasks 3.2/3.3): applied inside one
// command handler, active slot first, so the run's queue mirror, the tracked
// mpv index, and the mpv playlist itself all end at the shuffled order with
// the active slot at index 0, and a natural end and Next Up follow the new
// order afterwards.

use super::*;
use rstest::rstest;
use std::io::Read;
use std::os::unix::net::UnixStream;
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

/// An mpv session over a socket pair: the run's commands land on `peer`
/// without a live mpv handle (design D9), and the zero request timeout keeps
/// the run's command calls from waiting on the silent peer.
fn socket_pair_mpv() -> (Mpv, UnixStream) {
    let (ours, peer) = UnixStream::pair().expect("socket pair");
    let mpv =
        Mpv::for_test(crate::mpv_session::ipc::IpcSession::from_stream(ours).expect("session"));
    (mpv, peer)
}

/// The `playlist-move` commands the run issued, read from the peer end of
/// the socket-pair session in order.
fn captured_playlist_moves(peer: &mut UnixStream) -> Vec<(usize, usize)> {
    peer.set_nonblocking(true).expect("nonblocking peer");
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        match peer.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => bytes.extend_from_slice(&chunk[..n]),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(error) => panic!("peer socket read failed: {error}"),
        }
    }
    String::from_utf8(bytes)
        .expect("utf8 mpv commands")
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .filter_map(|value| {
            let command = value.get("command")?.as_array()?;
            if command.first()?.as_str()? != "playlist-move" {
                return None;
            }
            Some((
                command[1].as_str()?.parse().ok()?,
                command[2].as_str()?.parse().ok()?,
            ))
        })
        .collect()
}

/// The deterministic shuffled order the owner sends: the active slot first,
/// the rest reversed.
fn shuffled_target(run: &PlaybackRun) -> (Vec<QueueSlotId>, QueueSlotId) {
    let before: Vec<QueueSlotId> = (0..run.queue_len())
        .filter_map(|idx| run.slot_id_at(idx))
        .collect();
    let active = run.active_slot_id().expect("the fixture queue plays");
    let mut rest: Vec<QueueSlotId> = before
        .iter()
        .copied()
        .filter(|slot_id| *slot_id != active)
        .collect();
    rest.reverse();
    let target: Vec<QueueSlotId> = std::iter::once(active).chain(rest).collect();
    (target, active)
}

/// The run state after the owner's `QueueReorder` reached it: the pre-shuffle
/// order it started from, and the peer socket the playlist commands landed
/// on.
struct ReorderedRun {
    run: PlaybackRun,
    status: Arc<Mutex<PlayerStatus>>,
    peer: UnixStream,
    before: Vec<QueueSlotId>,
    target: Vec<QueueSlotId>,
    active: QueueSlotId,
}

fn run_after_shuffle_reorder(start_idx: usize) -> ReorderedRun {
    let (mut run, status) = make_queue_session_for_pos_tests(start_idx);
    let before: Vec<QueueSlotId> = (0..run.queue_len())
        .filter_map(|idx| run.slot_id_at(idx))
        .collect();
    let (mpv, peer) = socket_pair_mpv();
    let (target, active) = shuffled_target(&run);
    run.cmd_queue_reorder(&target, &mpv);
    ReorderedRun {
        run,
        status,
        peer,
        before,
        target,
        active,
    }
}

/// The contract: with the active slot in the middle, the `QueueReorder`
/// handler ends with the active slot at index 0, the queue mirror at the
/// shuffled order, and the mpv playlist rewritten to match it (spec scenario
/// "Shuffle with a playing item").
#[test]
fn shuffle_reorder_with_the_active_slot_in_the_middle_ends_at_index_0() {
    let ReorderedRun {
        run,
        status,
        mut peer,
        before,
        target,
        active,
    } = run_after_shuffle_reorder(1);

    let after: Vec<QueueSlotId> = (0..run.queue_len())
        .filter_map(|idx| run.slot_id_at(idx))
        .collect();
    assert_eq!(after, target);
    assert_eq!(run.active_slot_id(), Some(active));
    assert_eq!(run.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
    // The mpv playlist model starts from the pre-shuffle order and replays
    // what the run told mpv to do, so a queue/playlist divergence shows here.
    let mut playlist = before;
    let moves = captured_playlist_moves(&mut peer);
    assert!(
        !moves.is_empty(),
        "a mid-queue active slot needs at least one playlist move"
    );
    for (from, mpv_to) in moves {
        apply_mpv_playlist_move(&mut playlist, from, mpv_to);
    }
    assert_eq!(
        playlist, target,
        "the mpv playlist ends at the shuffled order"
    );
}

/// The contract: with the active slot at the end, the `QueueReorder` handler
/// still ends with it at index 0 and the tracked index following it.
#[test]
fn shuffle_reorder_with_the_active_slot_at_the_end_ends_at_index_0() {
    let ReorderedRun {
        run,
        status,
        target,
        active,
        ..
    } = run_after_shuffle_reorder(2);

    let after: Vec<QueueSlotId> = (0..run.queue_len())
        .filter_map(|idx| run.slot_id_at(idx))
        .collect();
    assert_eq!(after, target);
    assert_eq!(run.active_slot_id(), Some(active));
    assert_eq!(run.current_idx, 0);
    assert_eq!(status.lock().unwrap().current_idx, 0);
}

/// The contract (queue-content-shuffle task 3.3): a natural end after the
/// Shuffle reorder advances into the new order — the slot now at index 1 —
/// not the old order's next slot.
#[test]
fn natural_end_after_the_shuffle_reorder_advances_to_the_new_next_slot() {
    let (mut run, status, _events, http) = make_queue_session_for_pos_tests_with_mock(1);
    let (mpv, _peer) = socket_pair_mpv();
    let (target, _active) = shuffled_target(&run);
    // The advance path's four requests: the completed item's stop report,
    // its mark-played (a natural video EOF counts as played), the next
    // item's playback-info lookup, and its start report.
    for _ in 0..4 {
        http.respond(200, "");
    }

    run.cmd_queue_reorder(&target, &mpv);
    assert!(!run.on_end_file(EndFileReason::Eof, &mpv, &mut noop_progress()));

    assert_eq!(run.active_slot_id(), Some(target[1]));
    assert_eq!(run.current_idx, 1);
    assert_eq!(status.lock().unwrap().current_idx, 1);
}

/// The contract (queue-content-shuffle task 3.3): the Next Up card after the
/// Shuffle reorder names the new next slot — the run fires `QueueNextUp`
/// with the shuffled order's index 1, which the owner resolves against the
/// reordered queue.
#[test]
fn queue_next_up_after_the_shuffle_reorder_names_the_new_next_slot() {
    let (mut run, _status, events) = make_queue_session_for_pos_tests_with_events(1);
    let (mpv, _peer) = socket_pair_mpv();
    let (target, _active) = shuffled_target(&run);
    run.cmd_queue_reorder(&target, &mpv);

    // Inside the 60s-before-end Next Up window of the 1h episode, with more
    // than the 20s minimum remaining.
    run.on_time_pos(3545.0, &mpv);

    let event = events.try_recv().expect("the next-up event");
    assert!(
        matches!(event, PlayerEvent::QueueNextUp { next_idx: 1 }),
        "the card must name the new next slot, got {event:?}"
    );
}
