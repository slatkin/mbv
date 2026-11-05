//! Public-API tests for the Queue playback Surface state (add-gui-front-end
//! row 4.5): the seekbar's throttled scrub admission and the seek-target
//! fraction rule, moved from the TUI adapter's `queue_playback_panel` tests.

use std::time::{Duration, Instant};

use mbv_surfaces::HitRegion;
use mbv_surfaces::queue_playback::QueuePlaybackSurface;
use mbv_surfaces::seekbar::SEEKBAR_SCRUB_INTERVAL;
use mbv_ui_msg::Position;

/// A painted 40-column bar one row tall, at column 0, row 4.
fn bar() -> HitRegion {
    HitRegion::new(0.0, 4.0, 40.0, 1.0)
}

fn at(x: f32, y: f32) -> Position {
    Position { x, y }
}

/// A seekbar scrub admits one seek per throttle interval, and only for a
/// drag that anchored inside the bar span: a drag that merely crosses the
/// bar is not a scrub of it. The clock is injected, so the test never
/// sleeps. (Moved from the TUI adapter's panel tests, row 4.5.)
#[test]
fn seekbar_scrub_needs_an_in_bar_anchor_and_one_seek_per_interval() {
    let mut surface = QueuePlaybackSurface::new();
    let bar = bar();
    let admit =
        |surface: &mut QueuePlaybackSurface, from: f32, to: f32, now: Instant| -> Option<f64> {
            surface.admit_scrub(bar, at(from, 4.0), at(to, 4.0), now)
        };

    let t0 = Instant::now();
    let target = 20.0;
    assert!(
        matches!(admit(&mut surface, 0.0, target, t0), Some(f) if (f - 0.5).abs() < 1e-6),
        "an in-bar drag scrubs to the pointer column"
    );
    assert!(
        admit(
            &mut surface,
            1.0,
            target + 1.0,
            t0 + Duration::from_millis(10)
        )
        .is_none(),
        "a second scrub inside the interval is throttled"
    );
    assert!(
        admit(&mut surface, 2.0, target + 1.0, t0 + SEEKBAR_SCRUB_INTERVAL).is_some(),
        "a scrub after the interval is admitted again"
    );
    assert!(
        admit(
            &mut surface,
            bar.x - 1.0,
            target,
            t0 + SEEKBAR_SCRUB_INTERVAL + Duration::from_millis(1),
        )
        .is_none(),
        "a drag that anchored outside the bar never scrubs"
    );
}

/// The seek-target rule: a leading partial cell is an ordinary seek target
/// (its fraction of the span), a column before the bar resolves 0, and a bar
/// the Client did not paint resolves nothing (spec `queue-playback-panel` "A
/// partially filled cell is still an ordinary seek target").
#[test]
fn seek_target_resolves_the_columns_fraction_of_the_painted_span() {
    let surface = QueuePlaybackSurface::new();
    let bar = bar();
    assert!(
        matches!(surface.seek_target(bar, 3.0), Some(f) if (f - 3.0 / 40.0).abs() < 1e-9),
        "a leading partial cell is an ordinary seek target"
    );
    assert!(
        matches!(surface.seek_target(bar, bar.x - 1.0), Some(f) if f == 0.0),
        "a column before the bar resolves 0"
    );
    assert!(
        surface
            .seek_target(HitRegion::new(0.0, 4.0, 0.0, 1.0), 3.0)
            .is_none(),
        "a bar the Client did not paint resolves nothing"
    );
}
