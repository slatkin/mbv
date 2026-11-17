use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use mbv_queue::{QueueItem, QueueSlot, QueueSlotId};
use rstest::rstest;
use time::UtcOffset;

use super::{playback_finish_epoch, queue_end_tail, queue_finish_clock};
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
        queue: None,
        start_slot: None,
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
    assert_eq!(app.queue_summary_projection(&[]).remaining, 2);
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

/// The tail is the sum of the pending slots' runtimes after the active one:
/// the active slot already ends at the window's anchor, so its own runtime
/// does not extend the tail, and a playhead on the last slot has no tail.
#[test]
fn queue_end_tail_sums_the_pending_runtimes_after_the_playhead() {
    let slots = [
        slot(1, 20 * 60 * TICKS_PER_SECOND),
        slot(2, 10 * 60 * TICKS_PER_SECOND),
        slot(3, 5 * 60 * TICKS_PER_SECOND),
    ];
    assert_eq!(queue_end_tail(&slots, 0), Some(15 * 60));
    assert_eq!(queue_end_tail(&slots, 1), Some(5 * 60));
    assert_eq!(queue_end_tail(&slots, 2), Some(0));
}

/// A pending slot with an unknown runtime breaks the chain: the queue tail
/// stays unknown, because the end time is unknowable past the break. The
/// codebase's unknown-runtime marker is a zero runtime (a Twitch item, a feed
/// entry with no duration), so the marker is the break, not a known-empty
/// span.
#[test]
fn queue_end_tail_stops_at_the_first_unknown_pending_runtime() {
    let slots = [
        slot(1, 600 * TICKS_PER_SECOND),
        slot(2, 0),
        slot(3, 600 * TICKS_PER_SECOND),
    ];
    assert_eq!(queue_end_tail(&slots, 0), None);
}

/// One app whose shown queue holds one slot per entry of `runtime_minutes`,
/// carrying 1-based slot identities, with the Queue filter's text set.
fn app_with_runtimes(runtime_minutes: &[i64], filter: &str) -> crate::app::App {
    let items = runtime_minutes
        .iter()
        .enumerate()
        .map(|(index, minutes)| {
            item(
                &format!("Totals {index}"),
                minutes * 60 * TICKS_PER_SECOND,
                false,
            )
        })
        .collect();
    let mut app = app_with_queue(items, 0);
    app.queue_filter = filter.to_string();
    app
}

/// The marked slots `raw` names, as the Queue Surface reports its marks.
fn marks(raw: &[u64]) -> Vec<QueueSlotId> {
    raw.iter().copied().map(QueueSlotId::from_raw).collect()
}

/// The footer's totals: the queue's covers every slot of the shown queue, the
/// marked total covers the marked rows alone, a zero runtime is an unknown
/// runtime that makes its total a lower bound, and the filter narrows neither
/// (spec `gui-client` "The footer totals the marked rows", "No row is marked",
/// "A runtime is unknown", "The filter narrows the rows").
#[rstest]
#[case::the_footer_totals_the_marked_rows(
    &[10, 20, 30, 5],
    &[1, 3, 4],
    "",
    (65 * 60, false, Some(45 * 60), false)
)]
#[case::no_row_is_marked(&[10, 20, 30, 5], &[], "", (65 * 60, false, None, false))]
#[case::a_runtime_is_unknown(&[10, 0, 30], &[1, 2], "", (40 * 60, true, Some(10 * 60), true))]
#[case::the_filter_narrows_the_rows(&[10, 20, 30, 5], &[], "Totals 3", (65 * 60, false, None, false))]
fn queue_summary_totals_cover_the_shown_queue(
    #[case] runtime_minutes: &[i64],
    #[case] marked_slots: &[u64],
    #[case] filter: &str,
    #[case] expected: (i64, bool, Option<i64>, bool),
) {
    let app = app_with_runtimes(runtime_minutes, filter);

    let summary = app.queue_summary_projection(&marks(marked_slots));

    assert_eq!(
        (
            summary.total_secs,
            summary.total_partial,
            summary.marked_secs,
            summary.marked_partial,
        ),
        expected
    );
}
