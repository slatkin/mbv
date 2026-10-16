use mbv_core::applog as app_logging;
use mbv_ctrl::player::PlayerStatus;
use mbv_emby::EmbyClient;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_ids::{EmbySessionId, ItemId, MediaSourceId};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::thread;
use std::time::Duration;

// Snapshotted inputs for a stopped-report job: the values report_stopped
// captures today before handing off, so the worker never reads ids/status
// under a lock itself. `is_audio` and `last_valid_pos` are only needed for
// the log line (`pos` is already the zeroed-for-audio value to send).
pub(super) struct StoppedReportData {
    client: Arc<EmbyClient>,
    ws_tx: Option<mbv_ws::WsSender>,
    id: ItemId,
    msid: MediaSourceId,
    sid: EmbySessionId,
    is_audio: bool,
    last_valid_pos: i64,
    pos: i64,
    runtime_ticks: i64,
}

// How a start-report job gets its media_source_id/session_id: either the
// caller already resolved them synchronously (transition_to), or the fetch
// is deferred to the worker (transition_to_deferred, for pipe output where
// even the synchronous get_playback_info call would delay loadfile).
pub(super) enum StartIds {
    Resolved {
        media_source_id: MediaSourceId,
        session_id: EmbySessionId,
    },
    Deferred {
        ids: Arc<Mutex<(ItemId, MediaSourceId, EmbySessionId)>>,
        is_audio: Arc<AtomicBool>,
        /// The playback span of the slot this start belongs to; the worker
        /// records `play_session` into it once the session id resolves
        /// (design D5 — the shared `ids` may still hold the previous session
        /// at job entry, so they are never read for the span here).
        playback_span: tracing::Span,
    },
}

// Snapshotted inputs for an output-driven start-report job (design D9): the
// ids and the presented positions the output already resolved, so the worker
// reads nothing from a lock itself.
pub(super) struct StartedReportData {
    client: Arc<EmbyClient>,
    id: ItemId,
    msid: MediaSourceId,
    sid: EmbySessionId,
    position_ticks: i64,
    runtime_ticks: i64,
}

// The three jobs SessionReporter used to run on detached per-call threads,
// now executed FIFO on one worker thread fed by `SessionReporter::job_tx`.
pub(super) enum ReportJob {
    Stopped(StoppedReportData),
    Start {
        client: Arc<EmbyClient>,
        item: Box<EmbyItem>,
        ids: StartIds,
    },
    /// One output-driven start report (design D9): the ids and the presented
    /// positions were snapshotted at submit.
    OutputStart(StartedReportData),
    /// One output-driven progress report (design D9): snapshotted at submit,
    /// sent on the worker like every other report job.
    OutputProgress {
        client: Arc<EmbyClient>,
        ws_tx: Option<mbv_ws::WsSender>,
        report: mbv_emby::ProgressReport,
    },
    ProgressJoinThenStopped {
        handle: Option<thread::JoinHandle<()>>,
        budget: Duration,
        stopped: Option<StoppedReportData>,
    },
}

fn execute_stopped_report(data: StoppedReportData) {
    let StoppedReportData {
        client,
        ws_tx,
        id,
        msid,
        sid,
        is_audio,
        last_valid_pos,
        pos,
        runtime_ticks,
    } = data;
    // `playback.report` correlation span (design D5): the stopped data's own
    // ids, so the lines match the session actually being reported.
    let span = report_span(id.as_str(), Some(&sid));
    let _entered = span.entered();
    if let Some(ref tx) = ws_tx
        && tx.is_connected()
    {
        let _ = tx.flush(Duration::from_secs(1));
    }
    tracing::info!(name: "player.report_stopped.sent", target: "player", item = %id, is_audio, last_valid_pos_seconds = last_valid_pos / TICKS_PER_SECOND, position_seconds = pos / TICKS_PER_SECOND, "sent stopped report");
    let ok = client.report_stopped(&id, &msid, pos, &sid, runtime_ticks);
    if !ok {
        tracing::warn!(name: "player.report_stopped.failed", target: "player", "stopped report failed for previous item");
    }
}

/// Fill a span's `play_session` field once the Emby session id is known.
/// Empty ids (feed-only playback) leave the field unrecorded.
pub(crate) fn record_play_session(span: &tracing::Span, session_id: &EmbySessionId) {
    if !session_id.as_str().is_empty() {
        span.record("play_session", session_id.as_str());
    }
}

/// The `playback.report` correlation span for one report (design D5): the
/// `item` and `play_session` of the session being reported. An `Empty`
/// `play_session` stays unrecorded until the session id is known. The span is
/// a root (`parent: None`), so report lines never inherit a `playback` span.
pub(crate) fn report_span(item: &str, session: Option<&EmbySessionId>) -> tracing::Span {
    let span = tracing::info_span!(
        target: "player",
        parent: None,
        "playback.report",
        item = %item,
        play_session = tracing::field::Empty,
    );
    if let Some(sid) = session {
        record_play_session(&span, sid);
    }
    span
}

fn report_start_job(
    client: &EmbyClient,
    item: &EmbyItem,
    media_source_id: &MediaSourceId,
    session_id: &EmbySessionId,
) {
    let ok = client.report_start(item, media_source_id, session_id);
    if !ok {
        tracing::warn!(name: "player.report_start.failed", target: "player", phase = "report_worker", item = %item.id, "start report failed");
    }
}

/// Executes one output-driven start report (design D9): the same
/// `Sessions/Playing` start the run sends, from the ids and presented
/// positions the output snapshotted at submit.
fn execute_output_start_report(data: StartedReportData) {
    let StartedReportData {
        client,
        id,
        msid,
        sid,
        position_ticks,
        runtime_ticks,
    } = data;
    let span = report_span(id.as_str(), Some(&sid));
    let _entered = span.entered();
    tracing::info!(name: "player.report_start.sent", target: "player", item = %id, position_ticks, "sent output start report");
    let ok = client.report_start_ids(&id, &msid, &sid, position_ticks, runtime_ticks);
    if !ok {
        tracing::warn!(name: "player.report_start.failed", target: "player", item = %id, "output start report failed");
    }
}

/// Sends one progress report: over the websocket when it is connected, HTTP
/// otherwise — and on a websocket send failure the client falls back to HTTP
/// itself. Shared by the progress reporter thread and the output-driven
/// progress job (design D9).
fn send_progress(
    client: &EmbyClient,
    ws_tx: Option<&mbv_ws::WsSender>,
    report: &mbv_emby::ProgressReport,
) {
    if let Some(tx) = ws_tx.filter(|tx| tx.is_connected()) {
        client.report_progress_ws(report, tx);
        return;
    }
    client.report_progress_http(report);
}

fn run_report_worker(rx: mpsc::Receiver<ReportJob>) {
    for job in rx {
        match job {
            ReportJob::Stopped(data) => execute_stopped_report(data),
            ReportJob::OutputStart(data) => execute_output_start_report(data),
            ReportJob::OutputProgress {
                client,
                ws_tx,
                report,
            } => send_progress(&client, ws_tx.as_ref(), &report),
            ReportJob::Start { client, item, ids } => match ids {
                StartIds::Resolved {
                    media_source_id,
                    session_id,
                } => {
                    let span = report_span(item.id.as_str(), Some(&session_id));
                    let _entered = span.entered();
                    report_start_job(&client, &item, &media_source_id, &session_id);
                }
                StartIds::Deferred {
                    ids,
                    is_audio,
                    playback_span,
                } => {
                    // The shared ids can still name the previous session
                    // here, so the span carries only the new item until the
                    // fetch resolves the new session id (design D5).
                    let report_span = report_span(item.id.as_str(), None);
                    let _entered = report_span.enter();
                    let info = client.get_playback_info(&item.id);
                    record_play_session(&report_span, &info.session_id);
                    record_play_session(&playback_span, &info.session_id);
                    {
                        let mut locked = ids
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner);
                        locked.0 = ItemId::new(item.id.clone());
                        locked.1 = info.media_source_id.clone();
                        locked.2 = info.session_id.clone();
                    };
                    is_audio.store(item.is_audio(), Ordering::Relaxed);
                    report_start_job(&client, &item, &info.media_source_id, &info.session_id);
                }
            },
            ReportJob::ProgressJoinThenStopped {
                handle,
                budget,
                stopped,
            } => {
                if let Some(h) = handle {
                    let _ = mbv_net::bounded::run_with_hard_bound(
                        move || {
                            let _ = h.join();
                            Ok::<(), String>(())
                        },
                        budget,
                    );
                }
                if let Some(data) = stopped {
                    execute_stopped_report(data);
                }
            }
        }
    }
}

// Shared between the event loop thread and the progress reporter thread.
// All mutable fields are Arc-wrapped so transitions are visible to both.
#[derive(Clone)]
pub struct SessionReporter {
    pub(super) client: Arc<EmbyClient>,
    ws_tx: Option<mbv_ws::WsSender>,
    // (item_id, msid, sid) in a single lock so progress and event-loop threads never
    // observe a torn triple during item transitions.
    pub(super) ids: Arc<Mutex<(ItemId, MediaSourceId, EmbySessionId)>>,
    // Shared with progress thread so it knows whether to send progress or just ping.
    pub(super) is_audio: Arc<AtomicBool>,
    /// Set once any start report has been issued for this reporter. Emby has
    /// no session before the first start report, so progress and ping gate on
    /// it (their reports would reference a session the server never saw), and
    /// `report_start_when_playing` gates on it so a start already issued by an
    /// advance path (`start_item`/`transition_to`) is not repeated when the
    /// run's first `PlaybackRestart` arrives for that item.
    pub(super) start_reported: Arc<AtomicBool>,
    status: Arc<Mutex<PlayerStatus>>,
    // FIFO worker: stopped/start/progress-join jobs are sent here instead of
    // spawning a detached thread per call, so an outgoing stopped-report and
    // an incoming start-report can never race out of order (#bound-daemon-
    // playback-memory). The worker thread ends when every clone's sender
    // drops.
    pub(super) job_tx: mpsc::Sender<ReportJob>,
}

impl std::fmt::Debug for SessionReporter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionReporter")
            .field("client", &self.client)
            .field("ws_tx", &self.ws_tx)
            .field("ids", &self.ids)
            .field("is_audio", &self.is_audio)
            .field(
                "start_reported",
                &self.start_reported.load(Ordering::Relaxed),
            )
            .field("status", &self.status)
            .finish_non_exhaustive()
    }
}

impl SessionReporter {
    pub fn new(
        client: Arc<EmbyClient>,
        ws_tx: Option<mbv_ws::WsSender>,
        item_id: ItemId,
        msid: MediaSourceId,
        sid: EmbySessionId,
        is_audio: bool,
        status: Arc<Mutex<PlayerStatus>>,
    ) -> Self {
        let (job_tx, job_rx) = mpsc::channel::<ReportJob>();
        // The report worker outlives any slot, so it must not inherit the
        // spawning slot's `playback` span — dispatcher only (design D5).
        thread::spawn(app_logging::carry_dispatcher(move || {
            run_report_worker(job_rx);
        }));
        SessionReporter {
            client,
            ws_tx,
            ids: Arc::new(Mutex::new((item_id, msid, sid))),
            is_audio: Arc::new(AtomicBool::new(is_audio)),
            start_reported: Arc::new(AtomicBool::new(false)),
            status,
            job_tx,
        }
    }

    // Snapshots the values report_stopped_background/the progress-join job
    // need, so the worker reads nothing from a shared lock. `None` when the
    // reporter has no session (feed-only playback), matching has_session's
    // no-op contract for callers.
    pub(super) fn stopped_report_data(&self, last_valid_pos: i64) -> Option<StoppedReportData> {
        if !self.has_session() {
            return None;
        }
        let (id, msid, sid) = self
            .ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let is_audio = self.is_audio.load(Ordering::Relaxed);
        let pos = if is_audio { 0 } else { last_valid_pos };
        let runtime_ticks = self
            .status
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .runtime_ticks;
        Some(StoppedReportData {
            client: Arc::clone(&self.client),
            ws_tx: self.ws_tx.clone(),
            id,
            msid,
            sid,
            is_audio,
            last_valid_pos,
            pos,
            runtime_ticks,
        })
    }

    /// Returns `true` when the reporter holds a real Emby session (non-empty
    /// item ID). Guards against noisy failed HTTP calls during feed-only
    /// playback where no Emby session was ever established.
    pub(super) fn has_session(&self) -> bool {
        !self
            .ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .0
            .as_str()
            .is_empty()
    }

    /// Clear all session IDs so subsequent report calls are safe no-ops.
    /// Called when transitioning from Emby playback to a feed entry to
    /// prevent stale session state from leaking into the feed lifecycle.
    pub(super) fn clear_session(&self) {
        let mut ids = self
            .ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        ids.0 = ItemId::empty();
        ids.1 = MediaSourceId::new("");
        ids.2 = EmbySessionId::new("");
    }

    // Sends progress via websocket when connected, otherwise falls back to HTTP.
    // Recovers from poisoned mutexes so the progress thread never panics while
    // holding a lock.  No-op when the reporter has no session (feed-only
    // playback) or no start report has been sent yet (the progress thread
    // spawns at submit and would otherwise reference a session Emby never
    // saw started), so callers never need to guard.
    pub(super) fn report_progress(&self, event_name: &str) {
        if !self.has_session() || !self.start_reported.load(Ordering::Relaxed) {
            return;
        }
        let (id, msid, sid) = self
            .ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        // `playback.report` from the shared ids at call time — exactly the
        // ids this report sends (design D5).
        let span = report_span(id.as_str(), Some(&sid));
        let _entered = span.entered();
        let (pos, runtime, paused) = {
            let s = self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (s.position_ticks, s.runtime_ticks, s.paused)
        };
        let report = mbv_emby::ProgressReport {
            item_id: id,
            media_source_id: msid,
            position_ticks: pos,
            runtime_ticks: runtime,
            is_paused: paused,
            session_id: sid,
            event_name: event_name.to_string(),
        };
        send_progress(&self.client, self.ws_tx.as_ref(), &report);
    }

    // Zeroes position for audio items so Emby doesn't resume audio from mid-track.
    // Returns false (no-op) when the reporter has no session so callers get the
    // correct StopReport state without needing per-site guards.
    pub(super) fn report_stopped(&self, last_valid_pos: i64) -> bool {
        let Some(d) = self.stopped_report_data(last_valid_pos) else {
            return false;
        };
        tracing::info!(name: "player.report_stopped.sent", target: "player", item = %d.id, is_audio = d.is_audio, last_valid_pos_seconds = d.last_valid_pos / TICKS_PER_SECOND, position_seconds = d.pos / TICKS_PER_SECOND, "sent stopped report");
        self.client
            .report_stopped(&d.id, &d.msid, d.pos, &d.sid, d.runtime_ticks)
    }

    // Fire-and-forget variant of report_stopped for the item being left behind
    // during a transition, so the player thread can issue loadfile for the new
    // item immediately instead of waiting on this HTTP call (and the WS flush,
    // which report_stopped's synchronous callers don't do — it's bookkeeping
    // that doesn't affect playback).  No-op when the reporter has no session.
    pub(crate) fn report_stopped_background(&self, last_valid_pos: i64) {
        if let Some(data) = self.stopped_report_data(last_valid_pos) {
            let _ = self.job_tx.send(ReportJob::Stopped(data));
        }
    }

    /// Reports the session start for a playback output whose session ids are
    /// already resolved (design D9: the Cast output's progress target carries
    /// ids, not an item payload): the position and runtime come from the
    /// presented status the output publishes, so the start carries where the
    /// output actually began. Queued on the report worker; a no-op once a
    /// start went out or when the reporter has no session.
    pub fn report_output_start(&self) {
        if self.start_reported.load(Ordering::Relaxed) || !self.has_session() {
            return;
        }
        let (id, msid, sid) = self
            .ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let (position_ticks, runtime_ticks) = {
            let status = self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (status.position_ticks, status.runtime_ticks)
        };
        self.start_reported.store(true, Ordering::Relaxed);
        let _ = self.job_tx.send(ReportJob::OutputStart(StartedReportData {
            client: Arc::clone(&self.client),
            id,
            msid,
            sid,
            position_ticks,
            runtime_ticks,
        }));
    }

    /// Reports one progress position for a playback output (design D9): the
    /// presented status supplies the position, runtime, and paused state at
    /// submit time. Queued on the report worker; a no-op when the reporter
    /// has no session or no start has gone out.
    pub fn report_output_progress(&self, event_name: &str) {
        if !self.has_session() || !self.start_reported.load(Ordering::Relaxed) {
            return;
        }
        let (id, msid, sid) = self
            .ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone();
        let (position_ticks, runtime_ticks, is_paused) = {
            let status = self
                .status
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (status.position_ticks, status.runtime_ticks, status.paused)
        };
        let report = mbv_emby::ProgressReport {
            item_id: id,
            media_source_id: msid,
            position_ticks,
            runtime_ticks,
            is_paused,
            session_id: sid,
            event_name: event_name.to_string(),
        };
        let _ = self.job_tx.send(ReportJob::OutputProgress {
            client: Arc::clone(&self.client),
            ws_tx: self.ws_tx.clone(),
            report,
        });
    }

    /// Reports the stop of a playback output's item (design D9): a no-op
    /// while no start went out, so a session Emby never saw takes no stop.
    /// Audio positions report as zero, as the run's stop reports do. Queued
    /// on the report worker.
    pub fn report_output_stop(&self, last_valid_pos: i64) {
        if !self.start_reported.load(Ordering::Relaxed) {
            return;
        }
        if let Some(data) = self.stopped_report_data(last_valid_pos) {
            let _ = self.job_tx.send(ReportJob::Stopped(data));
        }
    }

    // Fire-and-forget report_start for a new item. Used once transition_to has
    // already updated self.ids synchronously via get_playback_info, so this
    // call is pure Emby bookkeeping the session doesn't need to wait on.
    fn report_start_background(
        &self,
        item: &EmbyItem,
        media_source_id: &MediaSourceId,
        session_id: &EmbySessionId,
    ) {
        self.start_reported.store(true, Ordering::Relaxed);
        let _ = self.job_tx.send(ReportJob::Start {
            client: Arc::clone(&self.client),
            item: Box::new(item.clone()),
            ids: StartIds::Resolved {
                media_source_id: media_source_id.clone(),
                session_id: session_id.clone(),
            },
        });
    }

    /// Fire-and-forget `report_start` for the item the run is playing, using
    /// the ids the submit already resolved. Called by the run when playback
    /// actually begins, so a run that never plays reports no session start.
    /// A no-op once any start report went out (an advance path's
    /// `start_item`/`transition_to` already covers this run), so the run can
    /// call it on every `PlaybackRestart` without duplicating the report.
    pub(super) fn report_start_when_playing(&self, item: &EmbyItem) {
        if self.start_reported.load(Ordering::Relaxed) || !self.has_session() {
            return;
        }
        let (_, media_source_id, session_id) = self.ids.lock().unwrap().clone();
        self.report_start_background(item, &media_source_id, &session_id);
    }

    pub(super) fn report_stopped_for_shutdown(
        &self,
        last_valid_pos: i64,
        timeout: Duration,
    ) -> bool {
        let Some(d) = self.stopped_report_data(last_valid_pos) else {
            return false;
        };
        tracing::info!(name: "player.report_stopped.sent", target: "player", item = %d.id, is_audio = d.is_audio, last_valid_pos_seconds = d.last_valid_pos / TICKS_PER_SECOND, position_seconds = d.pos / TICKS_PER_SECOND, timeout_ms = timeout.as_millis(), "sent shutdown stopped report");
        self.client.report_stopped_for_shutdown(
            &d.id,
            &d.msid,
            d.pos,
            &d.sid,
            d.runtime_ticks,
            timeout,
        )
    }

    pub(super) fn report_ping(&self) {
        if !self.start_reported.load(Ordering::Relaxed) {
            return;
        }
        let (id, sid) = {
            let ids = self
                .ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            (ids.0.clone(), ids.2.clone())
        };
        let span = report_span(id.as_str(), Some(&sid));
        let _entered = span.entered();
        self.client.report_ping(&sid);
    }

    // get_playback_info + report_start for a new item, updating tracking ids
    // *before* the network call so the progress reporter thread never sends
    // stale IDs to Emby.
    // Returns (ext_sub_urls, success).
    pub(super) fn start_item(
        &self,
        item: &EmbyItem,
        playback_span: &tracing::Span,
    ) -> (Vec<String>, bool) {
        let info = self.client.get_playback_info(&item.id);
        // Update ids before report_start so the progress reporter (which reads
        // ids on a 10-second timer) always sees the new item.
        {
            let mut ids = self
                .ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            ids.0 = ItemId::new(item.id.clone());
            ids.1 = info.media_source_id.clone();
            ids.2 = info.session_id.clone();
        };
        record_play_session(playback_span, &info.session_id);
        self.is_audio.store(item.is_audio(), Ordering::Relaxed);
        self.start_reported.store(true, Ordering::Relaxed);
        let ok = self
            .client
            .report_start(item, &info.media_source_id, &info.session_id);
        (info.external_subtitle_urls, ok)
    }

    // report_stopped for the current item and report_start for the new one are
    // both pure Emby bookkeeping, so both fire on background threads. Only
    // get_playback_info runs synchronously here — the session needs its ids
    // and ext_sub_urls before loadfile can be issued for the new item.
    pub(super) fn transition_to(
        &self,
        new_item: &EmbyItem,
        last_valid_pos: i64,
        playback_span: &tracing::Span,
    ) -> Vec<String> {
        self.report_stopped_background(last_valid_pos);
        let info = self.client.get_playback_info(&new_item.id);
        let ext_sub_urls = info.external_subtitle_urls;
        {
            let mut ids = self
                .ids
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            ids.0 = ItemId::new(new_item.id.clone());
            ids.1 = info.media_source_id.clone();
            ids.2 = info.session_id.clone();
        };
        record_play_session(playback_span, &info.session_id);
        self.is_audio.store(new_item.is_audio(), Ordering::Relaxed);
        self.report_start_background(new_item, &info.media_source_id, &info.session_id);
        ext_sub_urls
    }

    // Fully deferred variant of transition_to for pipe output, where ext_sub_urls
    // are irrelevant (audio-only) and the progress reporter can tolerate briefly
    // stale ids. Moves get_playback_info off the player thread so loadfile can be
    // issued immediately.
    pub(super) fn transition_to_deferred(
        &self,
        new_item: &EmbyItem,
        last_valid_pos: i64,
        playback_span: tracing::Span,
    ) {
        self.report_stopped_background(last_valid_pos);
        let _ = self.job_tx.send(ReportJob::Start {
            client: Arc::clone(&self.client),
            item: Box::new(new_item.clone()),
            ids: StartIds::Deferred {
                ids: Arc::clone(&self.ids),
                is_audio: Arc::clone(&self.is_audio),
                playback_span,
            },
        });
        // The start report itself fires on the worker shortly after; the flag
        // only needs to lead the run's next `PlaybackRestart` check.
        self.start_reported.store(true, Ordering::Relaxed);
    }
}
