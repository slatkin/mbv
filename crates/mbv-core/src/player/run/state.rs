use std::num::NonZeroU8;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum LoadState {
    Ready,
    Pending(NonZeroU8),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Drained {
    HitZero,
    StillPending,
    AlreadyReady,
}

impl LoadState {
    pub(crate) fn begin_single() -> Self {
        LoadState::Pending(NonZeroU8::new(1).unwrap())
    }

    pub(crate) fn drain(&mut self) -> Drained {
        match self {
            LoadState::Ready => Drained::AlreadyReady,
            LoadState::Pending(n) => {
                let v = n.get();
                if v <= 1 {
                    *self = LoadState::Ready;
                    Drained::HitZero
                } else {
                    *n = NonZeroU8::new(v - 1).unwrap();
                    Drained::StillPending
                }
            }
        }
    }

    pub(crate) fn is_ready(&self) -> bool {
        matches!(self, LoadState::Ready)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum StopReport {
    NotSent,
    Sent,
    Accepted,
}

impl StopReport {
    pub(crate) fn mark_sent(accepted: bool) -> Self {
        if accepted {
            StopReport::Accepted
        } else {
            StopReport::Sent
        }
    }

    pub(crate) fn reset(&mut self) {
        *self = StopReport::NotSent;
    }

    pub(crate) fn is_accepted(&self) -> bool {
        matches!(self, StopReport::Accepted)
    }

    pub(crate) fn is_sent(&self) -> bool {
        !matches!(self, StopReport::NotSent)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum NextUp {
    Idle,
    Armed,
    Fired,
}

impl NextUp {
    pub(crate) fn arm(&mut self) {
        if matches!(self, NextUp::Idle) {
            *self = NextUp::Armed;
        }
    }

    pub(crate) fn fire(&mut self) {
        *self = NextUp::Fired;
    }

    pub(crate) fn reset(&mut self) {
        *self = NextUp::Idle;
    }

    pub(crate) fn is_fired(&self) -> bool {
        matches!(self, NextUp::Fired)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum IntroState {
    Pending,
    Shown,
    Dismissed,
}

impl IntroState {
    pub(crate) fn new(past: bool) -> Self {
        if past {
            IntroState::Dismissed
        } else {
            IntroState::Pending
        }
    }

    pub(crate) fn shown(&mut self) {
        *self = IntroState::Shown;
    }

    pub(crate) fn dismissed(&mut self) {
        *self = IntroState::Dismissed;
    }

    pub(crate) fn reset(&mut self, past: bool) {
        if past {
            *self = IntroState::Dismissed;
        } else {
            *self = IntroState::Pending;
        }
    }

    pub(crate) fn is_pending(&self) -> bool {
        matches!(self, IntroState::Pending)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum StartupPause {
    None,
    Holding { events_to_skip: u8 },
}

impl StartupPause {
    pub(crate) fn new(active: bool) -> Self {
        if active {
            StartupPause::Holding { events_to_skip: 2 }
        } else {
            StartupPause::None
        }
    }

    pub(crate) fn consume_event(&mut self) -> bool {
        match self {
            StartupPause::None => false,
            StartupPause::Holding { events_to_skip } => {
                if *events_to_skip > 0 {
                    *events_to_skip -= 1;
                    true
                } else {
                    false
                }
            }
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = StartupPause::None;
    }

    pub(crate) fn is_holding(&self) -> bool {
        matches!(self, StartupPause::Holding { .. })
    }
}
