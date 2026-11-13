//! The recording clock's tests (D16). The row 1.6 contract: a cancelled timer
//! sends nothing at its deadline. The uncancelled case in the same
//! `#[case]` table shows `fire_due` itself fires, so the cancelled case
//! cannot pass vacuously.

use std::time::{Duration, Instant};

use rstest::rstest;

use super::{Clock, RecordingClock};

/// A deadline far past every timer the test schedules.
fn long_past() -> Instant {
    Instant::now() + Duration::from_secs(3600)
}

#[rstest]
#[case::cancelled_timer_sends_nothing(true, None)]
#[case::uncancelled_timer_sends_at_its_deadline(false, Some(0))]
fn a_cancelled_timer_sends_nothing(#[case] cancel: bool, #[case] expected: Option<u64>) {
    let clock = RecordingClock::new();
    let timer = clock.after(Duration::from_secs(1));
    if cancel {
        clock.cancel(timer);
    }

    let fired = clock.fire_due(long_past());

    let fired: Vec<u64> = fired.iter().map(|timer| timer.0).collect();
    assert_eq!(fired, expected.map(|id| vec![id]).unwrap_or_default());
}
