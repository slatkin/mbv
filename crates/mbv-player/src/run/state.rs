use super::PlaybackRun;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum LoadState {
    Ready,
    DrainingReplacedFile,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum Drained {
    HitZero,
    AlreadyReady,
}

impl LoadState {
    pub(crate) fn drain(&mut self) -> Drained {
        match self {
            LoadState::Ready => Drained::AlreadyReady,
            LoadState::DrainingReplacedFile => {
                *self = LoadState::Ready;
                Drained::HitZero
            }
        }
    }

    pub(crate) fn is_ready(self) -> bool {
        matches!(self, LoadState::Ready)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum StopAction {
    ReportedNow(StopReport),
    Deferred,
    NothingPlaying,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct ItemLifecycleState {
    load_state: LoadState,
    stop_report: StopReport,
}

impl ItemLifecycleState {
    pub(crate) fn new() -> Self {
        Self {
            load_state: LoadState::Ready,
            stop_report: StopReport::NotSent,
        }
    }

    pub(crate) fn is_ready(self) -> bool {
        self.load_state.is_ready()
    }

    pub(crate) fn on_drained(&mut self) -> Drained {
        let drained = self.load_state.drain();
        if drained == Drained::HitZero {
            self.stop_report.reset();
        }
        drained
    }

    pub(crate) fn mark_reported(&mut self, report: StopReport) {
        self.stop_report = report;
    }

    pub(crate) fn accept_replacement(&mut self) {
        self.load_state = LoadState::DrainingReplacedFile;
    }

    pub(crate) fn is_unreported(self) -> bool {
        self.stop_report == StopReport::NotSent
    }

    pub(crate) fn is_sent(self) -> bool {
        self.stop_report.is_sent()
    }

    pub(crate) fn stop_report(self) -> StopReport {
        self.stop_report
    }
}

impl PlaybackRun {
    pub(crate) fn begin_item_lifecycle(&mut self, action: StopAction) {
        self.mark_reported(match action {
            StopAction::ReportedNow(report) => report,
            StopAction::Deferred | StopAction::NothingPlaying => StopReport::NotSent,
        });
        self.accept_replacement();
        self.tracks_initialized = false;
        self.forced_jump = None;
        self.reset_next_up_state();
        self.stopped_event_sent = false;
        self.mark_played_id = None;
        self.stopped_near_end = false;
    }

    pub(crate) fn accept_replacement(&mut self) {
        self.item_lifecycle.accept_replacement();
    }

    pub(crate) fn on_drained(&mut self) -> Drained {
        self.item_lifecycle.on_drained()
    }

    pub(crate) fn mark_reported(&mut self, report: StopReport) {
        self.item_lifecycle.mark_reported(report);
    }

    pub(crate) fn is_unreported(&self) -> bool {
        self.item_lifecycle.is_unreported()
    }

    pub(crate) fn stop_report_sent(&self) -> bool {
        self.item_lifecycle.is_sent()
    }

    pub(crate) fn stop_report(&self) -> StopReport {
        self.item_lifecycle.stop_report()
    }

    pub(crate) fn load_is_ready(&self) -> bool {
        self.item_lifecycle.is_ready()
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum StopReport {
    NotSent,
    Sent,
}

impl StopReport {
    pub(crate) fn reset(&mut self) {
        *self = StopReport::NotSent;
    }

    pub(crate) fn is_sent(self) -> bool {
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

    pub(crate) fn is_fired(self) -> bool {
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

    pub(crate) fn is_pending(self) -> bool {
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

    pub(crate) fn is_holding(self) -> bool {
        matches!(self, StartupPause::Holding { .. })
    }
}
