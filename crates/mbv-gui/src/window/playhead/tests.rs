//! Tests of the playhead (D16): the contracts of `playhead.test.ts` — the
//! position advances by the injected elapsed time while playing and holds
//! while paused, and while paused the finish times move with the injected
//! wall clock. The times are recorders and the clock is the recording clock
//! (D10), so no test waits.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};

use super::{Playhead, Times};
use crate::bridge::frame::playback::GuiPlayhead;
use crate::bridge::sections::GuiSection;
use crate::window::clock::{Clock, RecordingClock};

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`).
const SECOND: i64 = 10_000_000;

/// The same second, in f64, so the assertions compare without a cast.
const SECOND_F: f64 = 10_000_000.0;

/// The injected times and the handles a test drives (the port of the
/// TypeScript fixture): nothing runs for real, and the test advances the
/// clocks and fires what the playhead armed.
#[derive(Default)]
struct FakeTimes {
    /// The monotonic clock, the prediction's elapsed source, in
    /// milliseconds.
    now_ms: Mutex<f64>,
    /// The wall clock, the finish times' now source, in unix milliseconds.
    wall_ms: Mutex<i64>,
}

impl Times for FakeTimes {
    fn now_ms(&self) -> f64 {
        *self.now_ms.lock().expect("the test clock lives")
    }

    fn wall_now_ms(&self) -> i64 {
        *self.wall_ms.lock().expect("the test clock lives")
    }
}

impl FakeTimes {
    /// Move the monotonic clock.
    fn advance_ms(&self, ms: f64) {
        *self.now_ms.lock().expect("the test clock lives") += ms;
    }

    /// Move the wall clock.
    fn advance_wall(&self, ms: i64) {
        *self.wall_ms.lock().expect("the test clock lives") += ms;
    }
}

/// A deadline far past every timer the test arms, for
/// [`RecordingClock::fire_due`].
fn long_past() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

/// Fire every armed timer into the playhead, and report how many fired.
fn fire(clock: &RecordingClock, playhead: &mut Playhead) -> usize {
    let fired = clock.fire_due(long_past());
    for timer in &fired {
        playhead.on_timer(*timer);
    }
    fired.len()
}

/// The handles a test drives, shared out of the document's root. The
/// document stays alive for the fixture's life: the playhead's signals live
/// in the root's scope, so dropping the document drops the playhead's
/// values.
struct Fixture {
    doc: DioxusDocument,
    playhead: Playhead,
    times: Arc<FakeTimes>,
    clock: Arc<RecordingClock>,
}

/// The anchor playing `position_ticks` of a 200 s item in slot 7.
fn playing(position_ticks: i64) -> GuiPlayhead {
    GuiPlayhead {
        active: true,
        active_slot: Some(7),
        position_ticks,
        runtime_ticks: 200 * SECOND,
        paused: false,
        buffered_ticks: 0,
    }
}

/// The root's props: the recorders travel in, and the playhead — whose
/// signals need the root's scope — travels out through the handle.
#[derive(Clone)]
struct PlayheadProps {
    times: Arc<FakeTimes>,
    clock: Arc<RecordingClock>,
    handle: Rc<RefCell<Option<Playhead>>>,
}

fn playhead_app(props: PlayheadProps) -> Element {
    let PlayheadProps {
        times,
        clock,
        handle,
    } = props;
    let playhead = use_hook(move || Playhead::new(Arc::clone(&clock) as Arc<dyn Clock>, times));
    handle.borrow_mut().replace(playhead);
    rsx! {}
}

/// A document whose root built the playhead over the shared recorders.
fn built_playhead() -> Fixture {
    let times = Arc::new(FakeTimes {
        now_ms: Mutex::new(1_000.0),
        wall_ms: Mutex::new(0),
    });
    let clock = Arc::new(RecordingClock::new());
    let handle = Rc::new(RefCell::new(None));
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            playhead_app,
            PlayheadProps {
                times: Arc::clone(&times),
                clock: Arc::clone(&clock),
                handle: Rc::clone(&handle),
            },
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    let playhead = handle
        .borrow()
        .clone()
        .expect("the root handed the playhead over");
    Fixture {
        doc,
        playhead,
        times,
        clock,
    }
}

/// Contract 1 (task 3.2): the predicted position advances by the injected
/// elapsed time while playing — one frame five seconds after the stamp
/// lands five seconds of ticks ahead once the tick fires.
#[test]
fn the_predicted_position_advances_by_the_injected_elapsed_time_while_playing() {
    let Fixture {
        doc: _doc,
        playhead,
        times,
        clock,
    } = built_playhead();
    let mut playhead = playhead;
    playhead.stamp(playing(60 * SECOND));
    assert_eq!(clock.pending(), 2, "a playing anchor arms the tick");

    times.advance_ms(5_000.0);
    fire(&clock, &mut playhead);

    assert_eq!(playhead.position.cloned(), 65.0 * SECOND_F);
}

/// Contract 2 (task 3.2): the predicted position holds while paused, and the
/// tick stops — the window sends no update, so the finish times move
/// only with the wall clock.
#[test]
fn the_predicted_position_holds_while_paused() {
    let Fixture {
        doc: _doc,
        playhead,
        times,
        clock,
    } = built_playhead();
    let mut playhead = playhead;
    playhead.stamp(playing(60 * SECOND));
    playhead.stamp(GuiPlayhead {
        paused: true,
        ..playing(60 * SECOND)
    });
    times.advance_ms(10_000.0);
    let fired = fire(&clock, &mut playhead);

    assert_eq!(fired, 1, "a paused anchor fires only the minute clock");

    assert_eq!(playhead.position.cloned(), 60.0 * SECOND_F);
}

/// Contract 3 (task 3.2): while paused the playhead sends no update, so the
/// finish times move only with the window's wall clock: the minute timer
/// fires, the finish facts recompute, and the clock now is two minutes
/// later.
#[test]
fn while_paused_the_finish_times_move_with_the_injected_wall_clock() {
    let Fixture {
        doc: _doc,
        playhead,
        times,
        clock,
    } = built_playhead();
    let mut playhead = playhead;
    playhead.stamp(GuiPlayhead {
        paused: true,
        ..playing(60 * SECOND)
    });
    let before = playhead.finish.cloned().expect("the runtime is known");

    times.advance_wall(2 * 60_000);
    let fired = clock.fire_due(long_past());
    for timer in fired {
        playhead.on_timer(timer);
    }

    let after = playhead.finish.cloned().expect("the runtime is known");
    assert_eq!(after.finish_secs, before.finish_secs + 120);
}

/// Regression (8dd4badf0's sibling): a position and runtime above `u32::MAX`
/// ticks — a 2-hour book — predict unclamped. The u32-bound
/// `i64_to_f64_saturating` conversion read any item past ~7.15 minutes as
/// its 7.15-minute tail, so the scrubber's fraction saturated at 1.
#[test]
fn a_two_hour_book_predicts_its_real_position_and_fraction() {
    let Fixture {
        doc: _doc,
        playhead,
        times: _times,
        clock: _clock,
    } = built_playhead();
    let mut playhead = playhead;
    playhead.stamp(GuiPlayhead {
        active: true,
        active_slot: Some(1),
        position_ticks: 90 * 60 * SECOND,
        runtime_ticks: 2 * 3600 * SECOND,
        paused: false,
        buffered_ticks: 0,
    });

    assert_eq!(playhead.position.cloned(), 90.0 * 60.0 * SECOND_F);
    assert_eq!(playhead.fraction.cloned(), 0.75);
}

/// A re-sent `playhead` section whose revision the window holds — a `sync`
/// snapshot's copy — re-stamps nothing, and `reset` drops the anchor (the
/// port of the sections test's playhead contract).
#[test]
fn a_playhead_section_at_its_held_revision_re_stamps_nothing_and_reset_drops_it() {
    let Fixture {
        doc: _doc,
        playhead,
        times: _times,
        clock: _clock,
    } = built_playhead();
    let mut playhead = playhead;
    let section = GuiPlayhead {
        active: true,
        active_slot: Some(4),
        position_ticks: 1_000,
        runtime_ticks: 2_000,
        paused: false,
        buffered_ticks: 0,
    };
    playhead.stamp_if_new(&GuiSection {
        revision: 3,
        value: section,
    });
    playhead.stamp_if_new(&GuiSection {
        revision: 3,
        value: GuiPlayhead {
            position_ticks: 9_000,
            ..section
        },
    });
    assert_eq!(playhead.anchor.cloned(), Some(section));

    playhead.reset();

    assert_eq!(playhead.anchor.cloned(), None);
}
