use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use mbv_queue::{QueueItem, QueueSlot, QueueSlotId};
use time::UtcOffset;

use super::{finish_chain, playback_finish_epoch, queue_finish_clock};
use crate::app::QueueView;
use crate::app::tests::make_app_stub;

/// One Emby item with the stored facts the projection reads.
fn item(name: &str, runtime_ticks: i64, played: bool) -> QueueItem {
    let mut item = make_item(name, "Movie");
    item.runtime_ticks = runtime_ticks;
    item.played = played;
    QueueItem::Emby(Box::new(item))
}

/// One app whose queue holds `items` in canonical order, with `active_idx`
/// carrying the playhead.
fn app_with_queue(items: Vec<QueueItem>, active_idx: usize) -> crate::app::App {
    let mut app = make_app_stub();
    let slots = items
        .into_iter()
        .enumerate()
        .map(|(i, item)| mbv_ctrl::UnifiedQueueSlot {
            slot_id: u64::try_from(i).unwrap_or(u64::MAX) + 1,
            item,
        })
        .collect();
    let state = mbv_ctrl::UnifiedQueueStateData {
        status: mbv_ctrl::player::PlayerStatus::default(),
        active_slot: None,
        slots,
        revision: 1,
        source: mbv_queue::QueueSource::Unknown,
        lineage: mbv_queue::QueueLineage::default(),
        in_flight_transition: None,
        queued_latest_transition: None,
    };
    app.local_view = QueueView::from_snapshot(&state);
    let runtime = app
        .playback_queue()
        .item_at(active_idx)
        .map_or(0, QueueItem::runtime_ticks);
    let queue_len = app.playback_queue().slots().len();
    app.player.update_status(|status| {
        status.active = true;
        status.current_idx = active_idx;
        status.queue_len = queue_len;
        status.runtime_ticks = runtime;
    });
    app
}

/// The remaining count is the canvas's `remainingCount++` rule: every row
/// that does not carry the playhead counts, a watched row before the playhead
/// included (audit/main.md 32). A playhead on the second of three slots
/// leaves the watched row before it and the row after it.
#[test]
fn remaining_counts_a_watched_row_before_the_playhead() {
    let minute = 60 * TICKS_PER_SECOND;
    let app = app_with_queue(
        vec![
            item("Watched", 10 * minute, true),
            item("Playing", 20 * minute, false),
            item("Later", 10 * minute, false),
        ],
        1,
    );
    assert_eq!(app.queue_summary_projection().remaining, 2);
}

/// The queue finish clock carries the canvas's ` +1d` suffix only when the
/// finish epoch lies past the local day of the projection's clock: a finish
/// later the same day keeps the bare zero-padded clock (audit/main.md 31,
/// audit/collapsed.md G2).
#[test]
fn queue_finish_clock_suffixes_only_a_next_day_finish() {
    // 2026-01-15 23:30 UTC.
    let now = 1_768_519_800;
    // 2026-01-15 23:50 UTC and 2026-01-16 00:10 UTC.
    assert_eq!(
        queue_finish_clock(1_768_521_000, now, UtcOffset::UTC),
        "23:50"
    );
    assert_eq!(
        queue_finish_clock(1_768_522_200, now, UtcOffset::UTC),
        "00:10 +1d"
    );
}

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
