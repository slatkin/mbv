use super::*;

// Covers initial snapshots and reconnects: `unified_queue_state_for_peer` is
// the function handle_ws calls for both. Tested directly here to avoid the
// socket plumbing that integration tests cover.
#[test]
fn unified_projection_uses_observed_slot_not_desired_queue_slot() {
    let queue = PlaybackQueue::from_queue_items(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        Some(0),
    );
    let observed = queue.slots()[1].slot_id;
    let status = crate::player::PlayerStatus::default();
    let source = crate::config::QueueSource::Unknown;
    let event = super::unified_queue_state_for_peer(
        &status,
        &queue,
        &source,
        crate::ctrl::QueueLineage::default(),
        Some(observed),
        None,
        None,
        true,
        true,
    );
    let CtrlEvent::UnifiedQueueState(data) = event else {
        panic!("expected UnifiedQueueState");
    };
    assert_eq!(data.active_slot, Some(observed.raw()));
}

// A cold-started queue plays its first track with no track-to-track
// transition, so `observed_active_slot` is never set. While the daemon is
// playing, the projection falls back to the canonical queue's active slot so
// a peer's now-playing highlight doesn't strand on a stale row.
#[test]
fn unified_projection_falls_back_to_canonical_active_slot_while_playing() {
    let queue = PlaybackQueue::from_queue_items(
        vec![
            emby_qi("a", "Video", "Movie"),
            emby_qi("b", "Video", "Movie"),
        ],
        Some(0),
    );
    let start_slot = queue.slots()[0].slot_id;
    let source = crate::config::QueueSource::Unknown;

    // Not playing: no observation and no fallback -> no active slot.
    let idle = crate::player::PlayerStatus::default();
    let CtrlEvent::UnifiedQueueState(idle_data) = super::unified_queue_state_for_peer(
        &idle,
        &queue,
        &source,
        crate::ctrl::QueueLineage::default(),
        None,
        None,
        None,
        true,
        true,
    ) else {
        panic!("expected UnifiedQueueState");
    };
    assert_eq!(idle_data.active_slot, None);

    // Playing with no observation: fall back to the canonical active slot.
    let playing = crate::player::PlayerStatus {
        active: true,
        ..Default::default()
    };
    let CtrlEvent::UnifiedQueueState(playing_data) = super::unified_queue_state_for_peer(
        &playing,
        &queue,
        &source,
        crate::ctrl::QueueLineage::default(),
        None,
        None,
        None,
        true,
        true,
    ) else {
        panic!("expected UnifiedQueueState");
    };
    assert_eq!(playing_data.active_slot, Some(start_slot.raw()));
}
