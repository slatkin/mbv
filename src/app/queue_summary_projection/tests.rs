use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use mbv_queue::{QueueItem, QueueSlot, QueueSlotId};

use super::{finish_chain, playback_finish_epoch};

/// One Emby movie slot with the stored runtime the chain reads.
fn slot(n: u64, runtime_ticks: i64) -> QueueSlot {
    let mut item = make_item(&format!("Item {n}"), "Movie");
    item.runtime_ticks = runtime_ticks;
    QueueSlot {
        slot_id: QueueSlotId::from_raw(n),
        item: QueueItem::Emby(Box::new(item)),
    }
}

/// The active item's end is the clock plus its remaining runtime; an unknown
/// runtime (zero) names no clock, and an overshooting playhead ends now.
#[test]
fn playback_finish_epoch_resolves_the_active_item_end_from_the_runtime() {
    let now = 1_000_000;
    let ten_minutes = 10 * 60 * TICKS_PER_SECOND;
    assert_eq!(
        playback_finish_epoch(now, 3 * 60 * TICKS_PER_SECOND, ten_minutes),
        Some(now + 7 * 60)
    );
    assert_eq!(playback_finish_epoch(now, 0, 0), None);
    assert_eq!(
        playback_finish_epoch(now, ten_minutes * 2, ten_minutes),
        Some(now)
    );
}

/// The chain ends the active slot at the anchor, and each later slot ends
/// one runtime after the slot before it — the anchor already carries the
/// active slot's remaining time, so each later slot's own runtime is the
/// span that extends the chain.
#[test]
fn finish_chain_ends_each_pending_slot_after_the_one_before_it() {
    let slots = [
        slot(1, 0),
        slot(2, 10 * 60 * TICKS_PER_SECOND),
        slot(3, 5 * 60 * TICKS_PER_SECOND),
    ];
    let anchor = 1_000_000;
    let (ends, finish) = finish_chain(&slots, 1, anchor);
    assert_eq!(
        ends,
        vec![
            (QueueSlotId::from_raw(2), anchor),
            (QueueSlotId::from_raw(3), anchor + 5 * 60),
        ]
    );
    assert_eq!(finish, Some(anchor + 5 * 60));
}

/// The active slot itself ends at the anchor — the playback projection's
/// finish clock — and the chain's first entry carries it unchanged.
#[test]
fn finish_chain_ends_the_active_slot_at_the_anchor() {
    let slots = [
        slot(1, 20 * 60 * TICKS_PER_SECOND),
        slot(2, 600 * TICKS_PER_SECOND),
    ];
    let (ends, finish) = finish_chain(&slots, 0, 5_000);
    assert_eq!(ends[0], (QueueSlotId::from_raw(1), 5_000));
    assert_eq!(finish, Some(5_000 + 600));
}

/// A pending slot with an unknown runtime breaks the chain: it and every
/// later slot name no clock, and the queue finish stays unknown — the end
/// time is unknowable past the break. The codebase's unknown-runtime marker
/// is a zero runtime (a Twitch item, a feed entry with no duration), so the
/// marker is the break, not a known-empty span.
#[test]
fn finish_chain_stops_at_the_first_unknown_pending_runtime() {
    let slots = [
        slot(1, 600 * TICKS_PER_SECOND),
        slot(2, 0),
        slot(3, 600 * TICKS_PER_SECOND),
    ];
    let (ends, finish) = finish_chain(&slots, 0, 5_000);
    assert_eq!(ends, vec![(QueueSlotId::from_raw(1), 5_000)]);
    assert_eq!(finish, None);
}
