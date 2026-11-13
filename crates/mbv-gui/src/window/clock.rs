//! The window's clocks (D10 of `replace-tauri-with-blitz`): one thread holds
//! the deadline queue behind [`Clock::after`] and [`Clock::cancel`], and
//! [`Clock::want_frames`] gates the launch copy's `RedrawRequested` hook, so
//! a frame-driven surface (the playhead) paces at vsync. Tests inject
//! [`RecordingClock`], so no test sleeps.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use dioxus_native::prelude::{ReadableExt, Signal, WritableExt, WritableVecExt};

use crate::window::channel::{WindowEvent, WindowEventSender};

/// One timer's identity, as [`Clock::after`] returns it and
/// [`WindowEvent::Timer`] carries it to the surface that asked for it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimerId(u64);

/// The window's clocks (D10). The search debounce, the art retry, the minute
/// clock, and the playhead's frames go through it, so a test can inject
/// [`RecordingClock`] instead.
pub trait Clock: Send + Sync + 'static {
    /// Run `f` once, after `duration` has passed. The passed timer sends
    /// [`WindowEvent::Timer`] with the returned id to the root's drain task.
    fn after(&self, duration: Duration) -> TimerId;

    /// Cancel a timer: a cancelled timer sends nothing, even at its deadline.
    fn cancel(&self, timer: TimerId);

    /// Gate the frame loop (D10): while frames are wanted, the launch copy's
    /// `RedrawRequested` hook sends [`WindowEvent::Frame`] and requests the
    /// next redraw.
    fn want_frames(&self, wanted: bool);

    /// Whether frames are currently wanted.
    fn frames_wanted(&self) -> bool;
}

/// The deadline queue the timer thread waits over.
#[derive(Debug)]
struct Deadlines {
    queue: BinaryHeap<Reverse<(Instant, TimerId)>>,
    next_id: u64,
}

/// The real clock (D10): one thread over a deadline queue.
#[derive(Debug)]
pub struct TimerClock {
    deadlines: Arc<Mutex<Deadlines>>,
    wake: Arc<Condvar>,
    events: WindowEventSender,
    frames_wanted: AtomicBool,
}

impl TimerClock {
    /// A clock that sends passed timers through `events`, with its thread
    /// started. The thread ends when the window channel closes — a passed
    /// timer then has no reader — and the process exit drops it otherwise.
    #[must_use]
    pub fn new(events: WindowEventSender) -> Self {
        let clock = Self {
            deadlines: Arc::new(Mutex::new(Deadlines {
                queue: BinaryHeap::new(),
                next_id: 0,
            })),
            wake: Arc::new(Condvar::new()),
            events,
            frames_wanted: AtomicBool::new(false),
        };
        let deadlines = Arc::clone(&clock.deadlines);
        let wake = Arc::clone(&clock.wake);
        let events = clock.events.clone();
        thread::spawn(move || timer_thread(&deadlines, &wake, &events));
        clock
    }
}

impl Clock for TimerClock {
    fn after(&self, duration: Duration) -> TimerId {
        let mut deadlines = self.deadlines.lock().expect("the deadline queue lives");
        let id = TimerId(deadlines.next_id);
        deadlines.next_id += 1;
        deadlines
            .queue
            .push(Reverse((Instant::now() + duration, id)));
        self.wake.notify_all();
        id
    }

    fn cancel(&self, timer: TimerId) {
        let mut deadlines = self.deadlines.lock().expect("the deadline queue lives");
        let kept: Vec<_> = deadlines
            .queue
            .drain()
            .filter(|entry| entry.0.1 != timer)
            .collect();
        for entry in kept {
            deadlines.queue.push(entry);
        }
    }

    fn want_frames(&self, wanted: bool) {
        self.frames_wanted.store(wanted, Ordering::Relaxed);
    }

    fn frames_wanted(&self) -> bool {
        self.frames_wanted.load(Ordering::Relaxed)
    }
}

/// The timers that fired (D10): the root's drain task forwards every passed
/// timer's id here, and the surface that armed the timer consumes its own id.
/// A signal, so a surface's effect re-runs when its timer lands.
#[derive(Clone, Copy, Debug)]
pub struct FiredTimers {
    timers: Signal<Vec<TimerId>>,
}

impl Default for FiredTimers {
    fn default() -> Self {
        Self::new()
    }
}

impl FiredTimers {
    /// A router with no fired timers.
    #[must_use]
    pub fn new() -> Self {
        Self {
            timers: Signal::new(Vec::new()),
        }
    }

    /// A timer the clock passed (D10): the root's drain task forwards it.
    pub fn push(&mut self, timer: TimerId) {
        self.timers.push(timer);
    }

    /// Whether `timer` fired, and remove it: the armed surface consumes its
    /// own id, so the router cannot grow.
    pub fn take(&mut self, timer: TimerId) -> bool {
        let mut timers = self.timers.cloned();
        let Some(index) = timers.iter().position(|armed| *armed == timer) else {
            return false;
        };
        timers.remove(index);
        self.timers.set(timers);
        true
    }
}

/// The timer thread (D10): wait over the deadline queue, and send each passed
/// timer's id through the window channel. A new timer or a cancel wakes the
/// wait, so the earliest deadline is recomputed each time.
fn timer_thread(deadlines: &Mutex<Deadlines>, wake: &Condvar, events: &WindowEventSender) {
    loop {
        let mut deadlines = deadlines.lock().expect("the deadline queue lives");
        let deadline = loop {
            match deadlines.queue.peek().map(|entry| (entry.0).0) {
                Some(deadline) => break deadline,
                None => deadlines = wake.wait(deadlines).expect("the deadline queue lives"),
            }
        };
        let now = Instant::now();
        if now < deadline {
            deadlines = wake
                .wait_timeout(deadlines, deadline - now)
                .expect("the deadline queue lives")
                .0;
            // A spurious wake, a cancel, or a new earlier timer: recompute.
            continue;
        }
        if let Some(Reverse((_, timer))) = deadlines.queue.pop() {
            drop(deadlines);
            if events.unbounded_send(WindowEvent::Timer(timer)).is_err() {
                // The window channel is closed; no timer has a reader any more.
                return;
            }
        }
    }
}

/// The recorded deadline queue of a [`RecordingClock`].
#[cfg(test)]
#[derive(Debug, Default)]
struct RecordedDeadlines {
    queue: Vec<(Instant, TimerId)>,
    cancelled: std::collections::HashSet<TimerId>,
    next_id: u64,
    fired: Vec<TimerId>,
    frames_wanted: bool,
}

/// The recording clock the tests inject (D10, D16): `after` and `cancel`
/// record, and [`RecordingClock::fire_due`] answers what the real timer
/// thread would send. Nothing waits and no thread runs.
#[cfg(test)]
#[derive(Debug, Default)]
pub struct RecordingClock {
    deadlines: Mutex<RecordedDeadlines>,
}

#[cfg(test)]
impl RecordingClock {
    /// A clock with no timers.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// How many timers are armed and neither fired nor cancelled — a test's
    /// view of the queue.
    ///
    /// # Panics
    ///
    /// Panics when the clock's state mutex is poisoned, which no test should
    /// be able to cause.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.deadlines
            .lock()
            .expect("the test clock lives")
            .queue
            .len()
    }

    /// Send every timer whose deadline passed and that no [`Clock::cancel`]
    /// cancelled, as the real timer thread would, and report the ids that
    /// fired.
    ///
    /// # Panics
    ///
    /// Panics when the clock's state mutex is poisoned, which no test should
    /// be able to cause.
    pub fn fire_due(&self, now: Instant) -> Vec<TimerId> {
        let mut deadlines = self.deadlines.lock().expect("the test clock lives");
        let due: Vec<TimerId> = deadlines
            .queue
            .iter()
            .filter(|(deadline, id)| *deadline <= now && !deadlines.cancelled.contains(id))
            .map(|(_, id)| *id)
            .collect();
        deadlines.fired.extend(due.iter().copied());
        deadlines.queue.retain(|(deadline, _)| *deadline > now);
        due
    }
}

#[cfg(test)]
impl Clock for RecordingClock {
    fn after(&self, duration: Duration) -> TimerId {
        let mut deadlines = self.deadlines.lock().expect("the test clock lives");
        let id = TimerId(deadlines.next_id);
        deadlines.next_id += 1;
        deadlines.queue.push((Instant::now() + duration, id));
        id
    }

    fn cancel(&self, timer: TimerId) {
        self.deadlines
            .lock()
            .expect("the test clock lives")
            .cancelled
            .insert(timer);
    }

    fn want_frames(&self, wanted: bool) {
        self.deadlines
            .lock()
            .expect("the test clock lives")
            .frames_wanted = wanted;
    }

    fn frames_wanted(&self) -> bool {
        self.deadlines
            .lock()
            .expect("the test clock lives")
            .frames_wanted
    }
}

#[cfg(test)]
mod tests;
