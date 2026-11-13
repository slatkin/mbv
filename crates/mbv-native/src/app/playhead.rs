//! The playhead (design D8 of `blitz-queue-panel-spike`). The GUI core sends
//! an anchor, the position at the send. Between anchors the window predicts
//! the position from the time since the anchor arrived. A std thread ticks
//! four times a second while the anchor plays, so the window writes the
//! predicted position on each tick.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use futures_channel::mpsc::{UnboundedReceiver, unbounded};
use mbv_emby_model::{saturating_i64_from_f64, ticks_to_seconds};
use mbv_gui::bridge::frame::playback::GuiPlayhead;

/// The tick period while the anchor plays (D8): four ticks a second.
const TICK_PERIOD: Duration = Duration::from_millis(250);

/// The playhead anchor and the instant the window received it. The window
/// predicts the position from the anchor's position and the time since then.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    playhead: GuiPlayhead,
    received: Instant,
}

impl Anchor {
    /// The anchor the window holds from `received` on.
    #[must_use]
    pub fn new(playhead: GuiPlayhead, received: Instant) -> Self {
        Self { playhead, received }
    }

    /// Whether the window ticks for this anchor: a transport plays.
    #[must_use]
    pub fn plays(&self) -> bool {
        self.playhead.active && !self.playhead.paused
    }

    /// The item's runtime, in seconds.
    #[must_use]
    pub fn runtime_secs(&self) -> f64 {
        ticks_to_seconds(self.playhead.runtime_ticks.max(0))
    }

    /// The predicted position at `now`, in unquantized seconds, clamped to the
    /// runtime. A paused or idle anchor holds its position.
    #[must_use]
    pub fn position_at(&self, now: Instant) -> f64 {
        let base = ticks_to_seconds(self.playhead.position_ticks);
        let advance = if self.plays() {
            now.saturating_duration_since(self.received).as_secs_f64()
        } else {
            0.0
        };
        (base + advance).clamp(0.0, self.runtime_secs())
    }
}

/// The position the playhead signal takes on a tick (D8, contract 5): the
/// predicted position while the anchor plays, and no write otherwise.
#[must_use]
pub fn playhead_write(anchor: &Anchor, now: Instant) -> Option<f64> {
    anchor.plays().then(|| anchor.position_at(now))
}

/// The played fraction of `position` over `runtime`, clamped to `[0, 1]`. A
/// zero runtime is zero.
#[must_use]
pub fn played_fraction(position: f64, runtime: f64) -> f64 {
    if runtime <= 0.0 {
        return 0.0;
    }
    (position / runtime).clamp(0.0, 1.0)
}

/// The whole seconds `seconds` covers, rounded down. The clock text reads this
/// value, so it changes only on a whole-second change.
#[must_use]
pub fn whole_seconds(seconds: f64) -> i64 {
    saturating_i64_from_f64(seconds.floor())
}

/// The elapsed and remaining whole seconds for `position` against the
/// anchor's runtime. Both are zero while no anchor has arrived.
#[must_use]
pub fn clock_pair(anchor: Option<&Anchor>, position: f64) -> (i64, i64) {
    let runtime = anchor.map_or(0.0, Anchor::runtime_secs);
    (
        whole_seconds(position),
        whole_seconds((runtime - position).max(0.0)),
    )
}

/// Start the tick thread (D8). It sends one tick each period while `gate`
/// is set, and it ends when the receiver drops.
pub fn spawn_ticker(gate: Arc<AtomicBool>) -> UnboundedReceiver<()> {
    let (tx, rx) = unbounded();
    thread::spawn(move || {
        loop {
            thread::sleep(TICK_PERIOD);
            if tx.is_closed() {
                return;
            }
            if gate.load(Ordering::Relaxed) {
                // The receiver closes between the check and the send only when
                // the window is closing; the next check ends the thread.
                let _ = tx.unbounded_send(());
            }
        }
    });
    rx
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use mbv_emby_model::TICKS_PER_SECOND;
    use mbv_gui::bridge::frame::playback::GuiPlayhead;
    use rstest::rstest;

    use super::{Anchor, playhead_write};

    /// An anchor at 5 s into a 100 s item, received at `received`.
    fn anchor(active: bool, paused: bool, received: Instant) -> Anchor {
        Anchor::new(
            GuiPlayhead {
                active,
                active_slot: None,
                position_ticks: 5 * TICKS_PER_SECOND,
                runtime_ticks: 100 * TICKS_PER_SECOND,
                paused,
            },
            received,
        )
    }

    /// The playhead writes only while an anchor plays: a playing anchor
    /// writes its position advanced by 2.5 s, in seconds, and a paused or idle one writes
    /// nothing.
    #[rstest]
    #[case::playing(true, false, Some(7.5))]
    #[case::paused(true, true, None)]
    #[case::idle(false, false, None)]
    fn playhead_writes_only_while_playing(
        #[case] active: bool,
        #[case] paused: bool,
        #[case] expected: Option<f64>,
    ) {
        let received = Instant::now();
        let anchor = anchor(active, paused, received);
        let now = received + Duration::from_millis(2500);
        assert_eq!(playhead_write(&anchor, now), expected);
    }
}
