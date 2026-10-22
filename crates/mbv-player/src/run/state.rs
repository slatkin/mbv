use super::{Mpv, PlaybackRun, PlayerEvent, ProgressGuard, StopOutcome};

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
    /// The run's single exit: every in-run end builds its
    /// `StopOutcome` and calls this. The call closes the run identity over
    /// the lifecycle; only the first close deactivates the status snapshot
    /// and sends `PlayerEvent::Stopped`. A second end path for the same
    /// identity — a quit after a parked failed replacement, a shutdown after
    /// a reported stop — is the close saying `false`, and sends nothing: the
    /// close, not a flag, is the guard.
    pub(crate) fn end_run(&mut self, outcome: StopOutcome) {
        if !self.run_lifecycle.close(self.run_identity) {
            tracing::debug!(name: "player.run.end_suppressed", target: "player", "second end path for the ended run identity; no second Stopped");
            return;
        }
        tracing::info!(name: "player.run.ended", target: "player", slot = ?outcome.slot_id, position = outcome.position_ticks, played = outcome.played, consume = outcome.consume, has_error = outcome.error.is_some(), "run ended");
        self.status.lock().unwrap().active = false;
        let _ = self.event_tx.send(PlayerEvent::Stopped {
            slot_id: outcome.slot_id,
            run_identity: self.run_identity,
            position_ticks: outcome.position_ticks,
            played: outcome.played,
            consume: outcome.consume,
            error: outcome.error,
        });
    }

    /// Parks the run after a failed active-file select, install, or
    /// replacement prepare (`stop_for_active_file_failure`,
    /// `accept_stopped_replacement`): join the progress reporter, send the
    /// outgoing item's one stop report if it is still unreported, close the
    /// prepared source at that position, stop mpv and clear its playlist, and
    /// arm the replacement drain so `on_end_file` swallows the teardown
    /// end-file instead of letting the real end-file handlers advance the
    /// parked run or end it again. The session drops, so a later submission
    /// starts fresh and can still reuse the mpv child; the caller ends the
    /// parked run with its own `StopOutcome`.
    pub(crate) fn park_for_failed_load(&mut self, mpv: &Mpv, progress: &mut ProgressGuard) {
        let old_pos = self.last_valid_pos;
        progress.stop_and_join(Self::progress_join_budget());
        if self.is_unreported() {
            let _ = self.reporter.report_stopped(old_pos);
            self.mark_reported(StopReport::Sent);
        }
        self.close_prepared_source_at(old_pos);
        let _ = mpv.command("stop", &[]);
        let _ = mpv.command("playlist-clear", &[]);
        self.begin_item_lifecycle(StopAction::ReportedNow(self.stop_report()));
        self.ext_sub_urls.clear();
        self.reporter.clear_session();
    }

    pub(crate) fn begin_item_lifecycle(&mut self, action: StopAction) {
        self.mark_reported(match action {
            StopAction::ReportedNow(report) => report,
            StopAction::Deferred | StopAction::NothingPlaying => StopReport::NotSent,
        });
        self.accept_replacement();
        self.tracks_initialized = false;
        self.run_lifecycle.disarm_jump();
        self.reset_next_up_state();
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
