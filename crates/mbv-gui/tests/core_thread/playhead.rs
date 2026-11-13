//! The playhead anchor rule (design D5): the core sends a new anchor on a
//! changed `active`, slot, pause, or runtime, and on a new sample that drifts
//! past the tolerance from the prediction — and on no other tick. The time
//! the prediction advances by is injected through
//! `backdate_playhead_anchor_for_test`; the reported position through the
//! `App`'s `set_playing_*_for_test` seams. No real wait runs.

use std::time::Duration;

use mbv::{ClientTickHost, stub_app_with_playing_items};
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_emby_model::test_support::make_item;
use rstest::rstest;

use crate::TestCore;

/// One second, in ticks.
const SECOND: i64 = TICKS_PER_SECOND;

/// The window the drift tests backdate the anchor's send instant by: large
/// enough that a playing prediction runs well past the tolerance from a
/// sample that advanced only half a second.
const DRIFT_WINDOW: Duration = Duration::from_secs(5);

/// One Music track's playing item with its runtime, as the Owner's queue
/// carries it.
fn playing_track(id: &str, runtime_ticks: i64) -> mbv_queue::QueueItem {
    let mut item = make_item(id, "Audio");
    item.id = id.into();
    item.media_type = "Audio".into();
    item.runtime_ticks = runtime_ticks;
    mbv_queue::QueueItem::Emby(Box::new(item))
}

/// A core whose first slot plays: position 0, runtime 10 s, not paused. The
/// first batch has been drained.
fn playing_core() -> TestCore {
    let app = stub_app_with_playing_items(vec![
        playing_track("trk-1", 10 * SECOND),
        playing_track("trk-2", 20 * SECOND),
    ]);
    let mut test = crate::test_core_with(app);
    assert!(test.core.step());
    let held = test.held();
    assert!(
        held.playhead_section().is_some(),
        "the first batch carries the anchor"
    );
    test
}

/// Move the playhead to the second slot with no input.
fn move_to_second_slot(test: &mut TestCore) {
    test.core.app_mut().set_playing_index_for_test(1);
}

/// Pause with no position change.
fn pause(test: &mut TestCore) {
    test.core.app_mut().set_playing_position_for_test(0, true);
}

/// Change the reported runtime with no slot change.
fn change_runtime(test: &mut TestCore) {
    test.core
        .app_mut()
        .set_playing_runtime_for_test(15 * SECOND);
}

/// Seek: a new sample 5 s past the last sent position.
fn seek(test: &mut TestCore) {
    test.core
        .app_mut()
        .set_playing_position_for_test(5 * SECOND, false);
}

/// The playhead's own advance: a new sample half a second past the last sent
/// position — inside the tolerance of any prediction the drift window does
/// not stretch.
fn advance_half_a_second(test: &mut TestCore) {
    test.core
        .app_mut()
        .set_playing_position_for_test(SECOND / 2, false);
}

/// Leave the reported playhead where it is.
fn move_nothing(_test: &mut TestCore) {}

/// The first anchor carries the tick's playhead facts (design D5): the active
/// slot, position, runtime, and paused state of the one playback projection
/// read.
#[test]
fn the_first_anchor_carries_the_playhead_facts() {
    let mut test = playing_core();

    let held = test.held();
    let anchor = &held
        .playhead_section()
        .expect("the first batch carries the anchor")
        .value;
    assert!(anchor.active);
    assert_eq!(anchor.active_slot, Some(1));
    assert_eq!(anchor.position_ticks, 0);
    assert_eq!(anchor.runtime_ticks, 10 * SECOND);
    assert!(!anchor.paused);
}

/// The anchor rule (design D5): each trigger sends a new anchor — a revision
/// past the first one — and a tick with no trigger, a new sample inside the
/// tolerance or no new sample at all even with a backdated send instant,
/// leaves the window holding the first one. A no-trigger tick can send no
/// batch at all, so the assertion reads the window's held revision.
#[rstest]
#[case::a_slot_change_re_anchors(move_to_second_slot, false, true)]
#[case::a_pause_re_anchors(pause, false, true)]
#[case::a_runtime_change_re_anchors(change_runtime, false, true)]
#[case::a_seek_past_the_tolerance_re_anchors(seek, false, true)]
#[case::a_drift_past_the_tolerance_after_a_backdate_re_anchors(advance_half_a_second, true, true)]
#[case::a_new_sample_inside_the_tolerance_sends_no_anchor(advance_half_a_second, false, false)]
#[case::no_new_sample_after_a_backdate_sends_no_anchor(move_nothing, true, false)]
fn the_anchor_rule_sends_the_playhead_only_on_a_trigger(
    #[case] move_playhead: fn(&mut TestCore),
    #[case] backdate: bool,
    #[case] sends: bool,
) {
    let mut test = playing_core();

    move_playhead(&mut test);
    if backdate {
        test.core.backdate_playhead_anchor_for_test(DRIFT_WINDOW);
    }
    assert!(test.core.step());

    let held = test.held();
    let revision = held
        .playhead_section()
        .expect("the window holds the first anchor")
        .revision;
    assert_eq!(revision > 1, sends);
}
