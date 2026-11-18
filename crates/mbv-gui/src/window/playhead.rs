//! The window's playhead (D10 of `replace-tauri-with-blitz`): the port of
//! `lib/bridge/playhead.ts` over [`Clock`]. The position prediction from the
//! `playhead` anchor section: the core sends one anchor — the position,
//! runtime, paused state, and active slot at its send — on a changed fact or
//! a sample that drifted past the tolerance; between anchors the window
//! advances the position itself: the predicted position is
//! `anchor.positionTicks + (now − receipt stamp)` while playing, held while
//! paused. Everything that follows the position or the wall clock derives
//! here: the scrubber's played fraction and clocks, the transport's finish
//! time, the queue's finish clock (with its ` +1d` suffix), and each queue
//! row's end.
//!
//! The core and the window share no clock, so the window stamps each anchor
//! with its monotonic clock when it receives it; the send delay is the only
//! error, and the next anchor corrects it. The clock is injectable (D10), so
//! the tests drive time without a real wait.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dioxus_native::prelude::*;
use time::OffsetDateTime;

use crate::bridge::frame::playback::GuiPlayhead;
use crate::bridge::sections::GuiSection;
use crate::window::clock::{Clock, TimerId};
use crate::window::time::{clamp_ratio, format_duration_short, format_wall_clock, local_offset};

/// One second in Emby ticks (`mbv_emby_model::TICKS_PER_SECOND`), in f64 so
/// the prediction divides without repeated casts.
const TICKS_PER_SECOND: f64 = mbv_emby_model::TICKS_PER_SECOND_F64;

/// One millisecond in Emby ticks: the elapsed monotonic time the prediction
/// reads is milliseconds, the position is ticks.
const TICKS_PER_MS: f64 = TICKS_PER_SECOND / 1000.0;

/// One wall-clock minute, in milliseconds.
const MINUTE_MS: i64 = 60_000;

/// One second of position, in milliseconds: the elapsed and remaining clocks
/// paint whole seconds, so the prediction advances once per second boundary.
const SECOND_MS: f64 = 1_000.0;

/// One tick count as f64, so the fractions and clocks divide without
/// repeated casts. The i64→f64 cast is exact for |ticks| < 2^53 (about 28.5
/// years of media at 100 ns per tick), so the tick facts never lose
/// precision; the u32-bound `mbv_emby_model::i64_to_f64_saturating` clamped
/// them to ~7.15 minutes (the sibling of 8dd4badf0's chapter-span bug: any
/// item longer than ~7.15 minutes read as its 7.15-minute tail). The
/// num-traits i64→f64 cast is always `Some`, so the fallback is unreachable.
#[must_use]
pub(crate) fn ticks_f64(ticks: i64) -> f64 {
    num_traits::cast(ticks).unwrap_or(0.0)
}

/// The time sources the playhead reads (D10): the monotonic clock the
/// prediction advances over and the wall clock the finish times read. The
/// window's instance is [`SystemTimes`]; tests install recorders.
pub trait Times: Send + Sync + 'static {
    /// Monotonic milliseconds, as `performance.now()` gave.
    fn now_ms(&self) -> f64;

    /// The wall clock now, in unix milliseconds. The local zone the finish
    /// clocks paint comes from it.
    fn wall_now_ms(&self) -> i64;
}

/// The window's own times (D10): the process's monotonic clock from this
/// struct's creation, and the system wall clock.
#[derive(Debug)]
pub struct SystemTimes {
    start: Instant,
}

impl SystemTimes {
    /// A monotonic source that starts at its creation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Default for SystemTimes {
    fn default() -> Self {
        Self::new()
    }
}

impl Times for SystemTimes {
    fn now_ms(&self) -> f64 {
        self.start.elapsed().as_secs_f64() * 1000.0
    }

    fn wall_now_ms(&self) -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| {
                i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
            })
    }
}

/// The wall-clock facts the finish derivations read (D10).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayheadFinish {
    /// The wall clock now, in unix seconds: the ` +1d` compare's today.
    pub now_secs: i64,
    /// The active item's finish epoch, in unix seconds: the clock now plus
    /// the item's remaining time. The queue finish and each row end add
    /// their offset to it.
    pub finish_secs: i64,
}

/// The playhead one window keeps (D10): the anchor the batch stamps, the
/// timer-driven prediction clock, and the minute counter the finish times
/// move with. Create once in the root's `use_hook` and provide as context —
/// the signals and memos need the root's scope.
#[derive(Clone)]
pub struct Playhead {
    clock: Arc<dyn Clock>,
    times: Arc<dyn Times>,
    /// The stamped anchor, `None` before the first batch and after `reset`.
    pub anchor: Signal<Option<GuiPlayhead>>,
    /// The monotonic clock the prediction reads, moved by the tick timer
    /// only while the anchor is active and not paused (D10).
    pub now: Signal<f64>,
    /// The tick timer: armed to the predicted position's next whole second
    /// while playing, so the window repaints about once a second rather than
    /// at vsync (a frame loop repainted the whole scene every frame).
    tick_timer: Signal<Option<TimerId>>,
    /// The monotonic time the last stamp landed at.
    stamp_ms: Signal<f64>,
    /// The wall clock's minute counter: one timer re-arms to the next minute
    /// boundary, so the finish times move while playback is paused, with no
    /// update from the core (D10).
    pub minute: Signal<u64>,
    minute_timer: Signal<Option<TimerId>>,
    /// The revision of the `playhead` section last stamped (D6): the anchor
    /// rides the batch but not the section stores, so its revision is
    /// tracked here. A re-sent section whose revision the window holds — a
    /// `sync` snapshot's copy — re-stamps nothing, and the prediction
    /// continues from the stamp it already holds.
    stamped_revision: Signal<Option<u64>>,
    /// The predicted position, clamped into the runtime, in ticks.
    pub position: Memo<f64>,
    /// The active item's runtime, in ticks; `0` while unknown.
    pub runtime: Memo<i64>,
    /// The played fraction of the active item, clamped to `[0, 1]`.
    pub fraction: Memo<f64>,
    /// The buffered fraction of the active item, clamped to `[0, 1]`: the
    /// output's reported cache end over the runtime, painted as it is and
    /// never predicted (`gui-queue-panel-redesign` design D6). `0` while no
    /// output reports a position.
    pub buffered_fraction: Memo<f64>,
    /// The elapsed time, as the TUI's shared duration formatter paints it.
    pub elapsed: Memo<String>,
    /// The time left in the active item, in the same format. The panels
    /// paint the minus sign before it.
    pub remaining: Memo<String>,
    /// The wall-clock facts the finish derivations read, `None` while no
    /// anchor holds or the runtime is unknown. A `Memo`, so a subscriber
    /// re-renders only when a fact changes: the clocks paint at minute
    /// precision, so a per-frame recompute must not re-render them.
    pub finish: Memo<Option<PlayheadFinish>>,
}

impl std::fmt::Debug for Playhead {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Playhead")
            .field("anchor", &self.anchor.peek().clone())
            .finish_non_exhaustive()
    }
}

impl Playhead {
    /// A playhead over `clock` (D10's timers and frames) and `times` (the
    /// monotonic and wall clocks). Call from a component.
    #[must_use]
    pub fn new(clock: Arc<dyn Clock>, times: Arc<dyn Times>) -> Self {
        let anchor: Signal<Option<GuiPlayhead>> = Signal::new(None);
        let now: Signal<f64> = Signal::new(0.0);
        let stamp_ms: Signal<f64> = Signal::new(0.0);
        let minute: Signal<u64> = Signal::new(0);
        let runtime = Memo::new(move || anchor.cloned().map_or(0, |value| value.runtime_ticks));
        let position = Memo::new(move || {
            let Some(value) = anchor.cloned() else {
                return 0.0;
            };
            let elapsed = if value.paused {
                0.0
            } else {
                (now.cloned() - stamp_ms.cloned()).max(0.0)
            };
            let predicted = ticks_f64(value.position_ticks) + elapsed * TICKS_PER_MS;
            predicted.clamp(0.0, ticks_f64(value.runtime_ticks.max(0)))
        });
        let fraction =
            Memo::new(move || clamp_ratio(position.cloned(), ticks_f64(runtime.cloned())));
        let buffered_fraction = Memo::new(move || {
            let buffered = anchor.cloned().map_or(0, |value| value.buffered_ticks);
            clamp_ratio(ticks_f64(buffered), ticks_f64(runtime.cloned()))
        });
        let elapsed = Memo::new(move || {
            format_duration_short(mbv_emby_model::saturating_i64_from_f64(
                (position.cloned() / TICKS_PER_SECOND).floor(),
            ))
        });
        let remaining = Memo::new(move || {
            let left = (ticks_f64(runtime.cloned()) - position.cloned()).max(0.0);
            format_duration_short(mbv_emby_model::saturating_i64_from_f64(
                (left / TICKS_PER_SECOND).floor(),
            ))
        });
        let wall = Arc::clone(&times);
        let finish = Memo::new(move || {
            // The minute tick is a dependency, so a paused window's
            // finish times move with the wall clock (D10).
            minute.cloned();
            let value = anchor.cloned()?;
            if value.runtime_ticks <= 0 {
                return None;
            }
            let remaining_secs = mbv_emby_model::saturating_i64_from_f64(
                ((ticks_f64(value.runtime_ticks) - position.cloned()) / TICKS_PER_SECOND).floor(),
            );
            let now_secs = wall.wall_now_ms().div_euclid(1000);
            Some(PlayheadFinish {
                now_secs,
                finish_secs: now_secs + remaining_secs,
            })
        });
        Self {
            clock,
            times,
            anchor,
            now,
            tick_timer: Signal::new(None),
            stamp_ms,
            minute,
            minute_timer: Signal::new(None),
            stamped_revision: Signal::new(None),
            position,
            runtime,
            fraction,
            buffered_fraction,
            elapsed,
            remaining,
            finish,
        }
    }

    /// Stamp the anchor the batch carried (D10): it becomes the prediction's
    /// base, stamped with the clock at receipt. The prediction starts at the
    /// stamp, not at the previous `now` value — a stale one would jump the
    /// position by the gap.
    pub fn stamp(&mut self, anchor: GuiPlayhead) {
        let stamp_ms = self.times.now_ms();
        self.stamp_ms.set(stamp_ms);
        self.anchor.set(Some(anchor));
        self.now.set(stamp_ms);
        self.pump();
        self.schedule_minute();
    }

    /// Stamp `section`'s anchor when its revision is new (D6): a re-sent
    /// section whose revision the window holds — a `sync` snapshot's copy —
    /// re-stamps nothing, and the prediction continues from the stamp it
    /// already holds.
    pub fn stamp_if_new(&mut self, section: &GuiSection<GuiPlayhead>) {
        if self.stamped_revision.peek().cloned() == Some(section.revision) {
            return;
        }
        self.stamped_revision.set(Some(section.revision));
        self.stamp(section.value);
    }

    /// A passed timer (D10): the tick advances the prediction and re-arms to
    /// the next second; the minute clock re-arms to the next minute. A timer
    /// the playhead did not arm drops.
    pub fn on_timer(&mut self, timer: TimerId) {
        if self.tick_timer.peek().cloned() == Some(timer) {
            self.tick_timer.set(None);
            self.now.set(self.times.now_ms());
            self.schedule_tick();
            return;
        }
        if self.minute_timer.peek().cloned() != Some(timer) {
            return;
        }
        self.minute_timer.set(None);
        self.minute.set(self.minute.cloned() + 1);
        self.schedule_minute();
    }

    /// Drop the anchor and stop the clocks. Production never calls it: a
    /// window keeps its playhead for its life, and a test resets between
    /// cases.
    pub fn reset(&mut self) {
        if let Some(timer) = self.minute_timer.peek().cloned() {
            self.clock.cancel(timer);
        }
        self.minute_timer.set(None);
        self.cancel_tick();
        self.stamped_revision.set(None);
        self.anchor.set(None);
    }

    /// Re-arm the tick against the new stamp while the anchor is active and
    /// not paused; stop it otherwise (D10).
    fn pump(&mut self) {
        self.cancel_tick();
        self.schedule_tick();
    }

    /// Arm the tick at the predicted position's next whole second, while the
    /// anchor is active and not paused.
    fn schedule_tick(&mut self) {
        let Some(anchor) = self
            .anchor
            .peek()
            .cloned()
            .filter(|anchor| anchor.active && !anchor.paused)
        else {
            return;
        };
        let elapsed_ms = (self.times.now_ms() - *self.stamp_ms.peek()).max(0.0);
        let position_ms = ticks_f64(anchor.position_ticks) / TICKS_PER_MS + elapsed_ms;
        // One millisecond past the boundary, so the floored clocks land on
        // the new second.
        let delay_ms = SECOND_MS - position_ms.rem_euclid(SECOND_MS) + 1.0;
        let delay = Duration::from_secs_f64(delay_ms / 1000.0);
        self.tick_timer.set(Some(self.clock.after(delay)));
    }

    /// Cancel the armed tick, if any.
    fn cancel_tick(&mut self) {
        if let Some(timer) = self.tick_timer.peek().cloned() {
            self.clock.cancel(timer);
        }
        self.tick_timer.set(None);
    }

    /// Arm the one-minute timer, once: the firing re-arms itself.
    fn schedule_minute(&mut self) {
        if self.minute_timer.peek().is_some() {
            return;
        }
        let into_minute = self.times.wall_now_ms().rem_euclid(MINUTE_MS);
        let delay = u64::try_from(MINUTE_MS - into_minute).unwrap_or(0) + 1;
        self.minute_timer
            .set(Some(self.clock.after(Duration::from_millis(delay))));
    }
}

/// The queue's finish clock (D10): the wall clock at `tail_secs` past the
/// active item's end, with the ` +1d` suffix when the finish lands past the
/// local day of the clock now — the rule `queue_finish_clock` runs in Rust
/// for the TUI (`src/app/queue_summary_projection.rs`). `None` while the
/// window has no finish facts or the summary carries no tail.
#[must_use]
pub fn queue_finish_text(finish: Option<PlayheadFinish>, tail_secs: Option<i64>) -> Option<String> {
    let finish = finish?;
    let tail_secs = tail_secs?;
    let queue_secs = finish.finish_secs + tail_secs;
    let clock = format_wall_clock(queue_secs);
    if clock.is_empty() {
        return None;
    }
    let after_midnight = local_date(queue_secs)
        .zip(local_date(finish.now_secs))
        .is_some_and(|(queue, now)| queue > now);
    if after_midnight {
        Some(format!("{clock} +1d"))
    } else {
        Some(clock)
    }
}

/// The local date `unix_secs` falls on, for the ` +1d` compare.
fn local_date(unix_secs: i64) -> Option<time::Date> {
    OffsetDateTime::from_unix_timestamp(unix_secs)
        .ok()
        .map(|at| at.to_offset(local_offset()).date())
}

#[cfg(test)]
mod tests;
