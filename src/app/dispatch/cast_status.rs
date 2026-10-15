// Status polling (6.1), now-playing presentation (6.2), position
// extrapolation (6.3, 6.4), matching a reported entry to a dispatched item
// (6.5), and progress reporting to that item's provider (6.6).

use crate::app::App;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::types::cast::{
    CastAttachment, CastEvent, CastProgressTarget, DispatchedCastItem,
};
use mbv_audiobookshelf::{AudiobookshelfClient, AudiobookshelfPlaybackProgress};
use mbv_cast::CastError;
use mbv_cast::CastTransport;
use mbv_cast::client::{CastPlaybackState, CastStatus};
use mbv_emby_model::seconds_to_ticks;
use std::time::{Duration, Instant};

/// design.md targets 5-10s; the receiver drops an unanswered sender after
/// roughly 90s (see `CastClient::keep_alive`'s doc comment), so any cadence
/// in that range keeps the heartbeat alive with margin to spare.
pub(in crate::app) const CAST_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(7);

/// The keep-alive cadence, decoupled from the status poll. The receiver
/// terminates a sender whose PONGs lapse in well under ten seconds (observed
/// ~7s: two 2026-10-15 broken-pipe deaths landed exactly one poll interval
/// after the last pong, and the queue load's blocking exchange delayed the
/// next poll past the deadline, killing the connection within a second of
/// every play). The 7s poll rode at that deadline with no margin; pinging
/// every 3s keeps a pong inside the deadline even while a queue-load
/// exchange occupies the single worker thread (the load job also pongs
/// first, so the gap it can create is its own reply time alone).
pub(in crate::app) const CAST_KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(3);

/// How long a submitted cast job may go without any worker event before the
/// worker counts as unresponsive. The worst legitimate dispatch waits out an
/// in-flight status poll (one `CAST_STATUS_POLL_INTERVAL`), the provider
/// resolution round trip (Emby `PlaybackInfo`, hard-bound near 5s), and the
/// transport job itself; the bound adds margin over that sum. A
/// slow-but-alive exchange that exceeds it is treated as the wedge anyway --
/// the reconnect is the recovery, and a wedged worker never recovers on its
/// own.
///
/// The bound exists because `rust_cast` operations have no socket timeout:
/// one unresponsive receiver exchange blocks the single worker thread
/// forever. Status freezes and every later job queues behind it while the
/// target still reads connected -- the observed "plays nothing, reads as
/// connected" state after a receiver stops its media app.
pub(in crate::app) const CAST_JOB_ANSWER_BOUND: Duration = Duration::from_secs(20);

/// A progress report computed from a matched dispatched item, decoupled from
/// how it's actually sent so the matching+extrapolation core (6.5, and the
/// "is there anything to report" half of 6.6) is unit-testable without a
/// live Emby/Audiobookshelf/feed-store client.
struct CastProgressReport {
    position_seconds: f32,
    is_paused: bool,
}

impl App {
    pub(in crate::app) fn drain_cast_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(event) = self.channels.cast_rx.try_recv() {
            produced = true;
            self.handle_cast_event(event);
        }
        produced
    }

    /// Polls the attached receiver's status on `CAST_STATUS_POLL_INTERVAL` by
    /// submitting a job to its worker thread. The heartbeat itself is pumped
    /// by `spawn_cast_keep_alive` on its own cadence; this job's keep-alive
    /// is belt-and-braces ahead of the status exchange.
    pub(in crate::app) fn spawn_cast_status_poll(&mut self) {
        let Some(attachment) = self.cast_attachment.as_ref() else {
            return;
        };
        let Some(client) = attachment.client.clone() else {
            return;
        };
        let receiver_id = attachment.receiver_id.clone();
        self.cast_status_loading = true;
        self.last_cast_poll = Instant::now();
        // The unresponsive-watchdog clock: this poll expects a worker event.
        self.cast_job_sent.get_or_insert_with(Instant::now);
        let tx = self.channels.cast_tx.clone();
        let _ = client.send(Box::new(move |transport| {
            if let Err(e) = transport.keep_alive() {
                tracing::warn!(name: "cast.keep_alive.failed", target: "cast", error = %e, "cast keep-alive failed");
            }
            let status = transport.status();
            let _ = tx.send(CastEvent::StatusUpdated {
                receiver_id,
                status,
            });
        }));
    }

    /// Submits a pong-only job every [`CAST_KEEP_ALIVE_INTERVAL`], independent
    /// of the status poll: the poll's 7s cadence sat at the receiver's
    /// heartbeat deadline, and any worker occupancy (the queue load's
    /// blocking exchange above all) delayed the next pong past it, so the
    /// receiver closed the connection. The keep-alive job produces no worker
    /// event and never touches the watchdog clock; a socket this job finds
    /// dead is reported by the next status poll, which drives 7.6.
    pub(in crate::app) fn spawn_cast_keep_alive(&mut self) {
        let Some(attachment) = self.cast_attachment.as_ref() else {
            return;
        };
        let Some(client) = attachment.client.clone() else {
            return;
        };
        self.last_cast_keep_alive = Instant::now();
        let _ = client.send(Box::new(|transport: &mut dyn CastTransport| {
            if let Err(e) = transport.keep_alive() {
                tracing::warn!(name: "cast.keep_alive.failed", target: "cast", error = %e, "cast keep-alive failed");
            }
        }));
    }

    pub(in crate::app) fn apply_cast_status(
        &mut self,
        receiver_id: &str,
        status: Result<CastStatus, CastError>,
    ) {
        let Some(attachment) = self.cast_attachment.as_mut() else {
            return;
        };
        if attachment.receiver_id != receiver_id {
            return;
        }
        // Only the poll this attachment spawned re-opens the tick gate: a
        // late event from a previous attachment's worker (the watchdog
        // reconnect race) must not clear a poll still in flight.
        self.cast_status_loading = false;
        match status {
            Ok(status) => {
                attachment.status = Some(status);
                attachment.status_at = Instant::now();
                self.report_cast_progress();
            }
            Err(e) => {
                // Connection loss (7.6): present the target as disconnected
                // and stop polling it -- clearing `client` means
                // `spawn_cast_status_poll`'s next tick finds nothing to
                // submit a job to. `dispatched`/`status` and mbv's own queue
                // (`local_view.queue`, untouched here) are left exactly as
                // they were; only progress reporting and further polling
                // stop. A `rust_cast` receive blocks rather than times out,
                // so a poll error is a broken connection, not a slow
                // receiver -- recover the way the watchdog does: a fresh
                // connect relaunches the receiver's app session, and a
                // failed reconnect detaches (ConnectFailed), so the retry
                // stays bounded.
                tracing::warn!(name: "cast.status_poll.failed", target: "cast", error = %e, "cast status poll failed");
                let receiver_id = attachment.receiver_id.clone();
                attachment.mark_disconnected();
                self.flash(
                    "Cast connection lost; reconnecting".to_string(),
                    ToastSeverity::Warning,
                );
                self.connect_cast_receiver(receiver_id);
            }
        }
    }

    /// The unresponsive-worker watchdog. When a submitted cast job has
    /// produced no worker event within `CAST_JOB_ANSWER_BOUND`, the worker
    /// thread is wedged on an unbounded `rust_cast` exchange; recover the way
    /// a dropped connection is handled (7.6): log, present the target as
    /// disconnected, stop polling it -- then reconnect. A fresh connect
    /// relaunches the receiver's app session, which a media stop may have
    /// ended, so playback works again without a manual re-attach.
    pub(in crate::app) fn check_cast_worker_responsive(&mut self) {
        let Some(sent) = self.cast_job_sent else {
            return;
        };
        let Some(attachment) = self.cast_attachment.as_mut() else {
            return;
        };
        if sent.elapsed() < CAST_JOB_ANSWER_BOUND || attachment.client.is_none() {
            return;
        }
        let receiver_id = attachment.receiver_id.clone();
        attachment.mark_disconnected();
        self.cast_job_sent = None;
        tracing::warn!(name: "cast.worker.unresponsive", target: "cast", "cast worker produced no event within the answer bound");
        self.flash(
            "Cast target stopped responding; reconnecting".to_string(),
            ToastSeverity::Warning,
        );
        self.connect_cast_receiver(receiver_id);
    }

    /// `None` when no cast target is attached; the caller falls back to
    /// local/remote-session playback state.
    pub(in crate::app) fn cast_effective_playback_state(
        &self,
    ) -> Option<mbv_ui_model::playback::PlaybackState> {
        let cast = self.cast_attachment.as_ref()?;
        let Some(status) = cast.status.as_ref() else {
            // Attached and dispatched, but no status polled yet: present as
            // active so controls appear immediately (optimistic, matching
            // `attach_cast`'s optimistic-attach shape) rather than idle.
            return Some(mbv_ui_model::playback::PlaybackState {
                active: !cast.dispatched.is_empty(),
                active_idx: self.cast_active_queue_index(cast),
                position_ticks: 0,
                runtime_ticks: 0,
                paused: false,
            });
        };
        let active = status.state != CastPlaybackState::Idle;
        let position_seconds = cast_extrapolate(status, cast.status_at).unwrap_or(0.0);
        let runtime_ticks = seconds_to_ticks(f64::from(status.duration_seconds.unwrap_or(0.0)));
        Some(mbv_ui_model::playback::PlaybackState {
            active,
            active_idx: self.cast_active_queue_index(cast),
            position_ticks: seconds_to_ticks(f64::from(position_seconds)),
            runtime_ticks,
            paused: status.state == CastPlaybackState::Paused,
        })
    }

    fn cast_active_queue_index(&self, cast: &CastAttachment) -> Option<usize> {
        let status = cast.status.as_ref()?;
        let item = match_cast_dispatched_item(cast, status)?;
        self.local_view
            .slots()
            .iter()
            .position(|slot| slot.item.content_id() == item.content_id)
    }

    /// Title text for the now-playing header while a cast target is attached
    /// (6.2). `None` when the receiver reports no active media.
    pub(in crate::app) fn cast_now_playing_title(cast: &CastAttachment) -> Option<String> {
        if cast.disconnected {
            return Some("Cast: disconnected".to_string());
        }
        let status = cast.status.as_ref()?;
        if status.state == CastPlaybackState::Idle {
            return None;
        }
        match match_cast_dispatched_item(cast, status) {
            Some(item) => Some(item.title.clone()),
            // "Receiver plays something mbv did not dispatch" (spec): present
            // it, don't treat it as an error.
            None => Some("Cast".to_string()),
        }
    }

    pub(in crate::app) fn cast_extrapolated_position_seconds(&self) -> Option<f32> {
        let cast = self.cast_attachment.as_ref()?;
        cast_extrapolate(cast.status.as_ref()?, cast.status_at)
    }

    /// Reports progress to the matched item's provider (6.6). No-op when
    /// nothing is attached, no status has been reported yet, or the
    /// receiver's playing entry can't be matched back to a dispatched item.
    pub(in crate::app) fn report_cast_progress(&mut self) {
        let Some(cast) = self.cast_attachment.as_ref() else {
            return;
        };
        let Some(status) = cast.status.as_ref() else {
            return;
        };
        let Some((item, report)) = cast_progress_for_status(cast, status) else {
            return;
        };
        let position_ticks = seconds_to_ticks(f64::from(report.position_seconds));
        match &item.report {
            CastProgressTarget::Emby {
                item_id,
                media_source_id,
                session_id,
                runtime_ticks,
            } => {
                let Some(client) = self.emby_snapshot() else {
                    return;
                };
                let item_id = mbv_ids::ItemId::new(item_id.clone());
                let media_source_id = media_source_id.clone();
                let session_id = session_id.clone();
                let is_paused = report.is_paused;
                let progress = mbv_emby::ProgressReport {
                    item_id,
                    media_source_id,
                    position_ticks,
                    runtime_ticks: *runtime_ticks,
                    is_paused,
                    session_id,
                    event_name: "timeupdate".to_string(),
                };
                std::thread::spawn(move || {
                    client.report_progress_http(&progress);
                });
            }
            CastProgressTarget::Feed { feed_id, guid } => {
                if let Some(feed_id) = feed_id.clone() {
                    let guid = guid.clone();
                    self.write_feed_entry_state(&feed_id, &guid, position_ticks, false);
                }
            }
            CastProgressTarget::AudiobookshelfEpisode {
                session_id,
                duration_seconds,
                ..
            } => {
                let Some(ctx) = self.audiobookshelf_cast_context() else {
                    return;
                };
                let session_id = session_id.clone();
                let duration = *duration_seconds;
                let position_seconds = f64::from(report.position_seconds);
                std::thread::spawn(move || {
                    // `time_listened` drives Audiobookshelf's listening-time
                    // stats, not resume position (`current_time` is what's
                    // used for resume); reporting 0.0 here is a deliberate
                    // simplification -- stats accuracy isn't a spec
                    // requirement, resume position is, and that's covered.
                    let _ = ctx.client.sync_playback_session_bounded(
                        &ctx.credential,
                        &session_id,
                        AudiobookshelfPlaybackProgress {
                            current_time: position_seconds,
                            time_listened: 0.0,
                            duration,
                        },
                        AudiobookshelfClient::REQUEST_HARD_BOUND,
                    );
                });
            }
        }
    }
}

/// Matches the receiver's reported playing entry back to a dispatched item
/// by URL (6.5). `None` when the receiver is idle, reports no content id, or
/// is playing something mbv didn't dispatch.
fn match_cast_dispatched_item<'a>(
    cast: &'a CastAttachment,
    status: &CastStatus,
) -> Option<&'a DispatchedCastItem> {
    let content_id = status.playing_content_id.as_ref()?;
    cast.dispatched.iter().find(|item| &item.url == content_id)
}

/// Position extrapolation (6.3): advances from the last reported position by
/// elapsed wall-clock time and the reported playback rate. Holds (6.4) while
/// paused/buffering/idle. Drift correction (6.4) falls out structurally: each
/// new status resets `status_at` and `position_seconds` to the freshest
/// report, so extrapolation always starts from what the receiver last said.
fn cast_extrapolate(status: &CastStatus, status_at: Instant) -> Option<f32> {
    let position = status.position_seconds?;
    match status.state {
        CastPlaybackState::Playing => {
            let elapsed = status_at.elapsed().as_secs_f32();
            let advanced = position + elapsed * status.playback_rate;
            Some(match status.duration_seconds {
                Some(duration) => advanced.min(duration),
                None => advanced,
            })
        }
        CastPlaybackState::Paused | CastPlaybackState::Buffering | CastPlaybackState::Idle => {
            Some(position)
        }
    }
}

fn cast_progress_for_status<'a>(
    cast: &'a CastAttachment,
    status: &CastStatus,
) -> Option<(&'a DispatchedCastItem, CastProgressReport)> {
    let item = match_cast_dispatched_item(cast, status)?;
    Some((
        item,
        CastProgressReport {
            position_seconds: cast_extrapolate(status, cast.status_at).unwrap_or(0.0),
            is_paused: status.state == CastPlaybackState::Paused,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::state::types::cast::CastProgressTarget as ProgressTarget;
    use crate::app::tests::{QueueViewTestExt, make_app_stub};
    use mbv_queue::QueueItemContentId;

    fn dispatched(url: &str) -> DispatchedCastItem {
        DispatchedCastItem {
            url: url.to_string(),
            content_id: QueueItemContentId::Feed("guid".into()),
            title: "Title".into(),
            report: ProgressTarget::Feed {
                feed_id: Some("feed".into()),
                guid: "guid".into(),
            },
        }
    }

    fn attachment(dispatched_items: Vec<DispatchedCastItem>) -> CastAttachment {
        CastAttachment {
            receiver_id: "device-1".into(),
            receiver_name: "Living Room".into(),
            client: None,
            dispatched: dispatched_items,
            status: None,
            status_at: Instant::now(),
            volume: 50,
            muted: false,
            disconnected: false,
        }
    }

    fn playing_status(position: f32, content_id: &str) -> CastStatus {
        CastStatus {
            position_seconds: Some(position),
            duration_seconds: Some(100.0),
            playback_rate: 1.0,
            state: CastPlaybackState::Playing,
            playing_content_id: Some(content_id.to_string()),
            idle_reason: None,
        }
    }

    #[test]
    fn matches_reported_entry_to_dispatched_item() {
        let cast = attachment(vec![dispatched("https://a")]);
        let status = playing_status(10.0, "https://a");
        let matched = match_cast_dispatched_item(&cast, &status);
        assert!(matched.is_some());
    }

    #[test]
    fn no_match_when_receiver_plays_something_not_dispatched() {
        let cast = attachment(vec![dispatched("https://a")]);
        let status = playing_status(10.0, "https://not-dispatched");
        assert!(match_cast_dispatched_item(&cast, &status).is_none());
    }

    #[test]
    fn holds_position_while_paused() {
        let mut status = playing_status(10.0, "https://a");
        status.state = CastPlaybackState::Paused;
        let status_at = Instant::now()
            .checked_sub(Duration::from_secs(5))
            .unwrap_or_else(Instant::now);
        assert_eq!(cast_extrapolate(&status, status_at), Some(10.0));
    }

    #[test]
    fn a_fresh_status_report_corrects_drift() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        app.cast_attachment.as_mut().unwrap().dispatched = vec![dispatched("https://a")];
        app.apply_cast_status("device-1", Ok(playing_status(10.0, "https://a")));
        // A later report disagrees with where extrapolation from the first
        // report would have put the position by now.
        app.apply_cast_status("device-1", Ok(playing_status(50.0, "https://a")));
        let position = app.cast_extrapolated_position_seconds().unwrap();
        assert!((49.5..=50.5).contains(&position), "position={position}");
    }

    #[test]
    fn now_playing_title_is_the_matched_dispatched_item_while_playing() {
        let mut cast = attachment(vec![dispatched("https://a")]);
        cast.status = Some(playing_status(10.0, "https://a"));
        let _app = make_app_stub();
        assert_eq!(App::cast_now_playing_title(&cast).as_deref(), Some("Title"));
    }

    #[test]
    fn spawn_cast_status_poll_calls_keep_alive_and_status_on_the_transport() {
        use mbv_cast::{FakeCastTransport, spawn_fake_cast_worker};
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, calls) = spawn_fake_cast_worker(FakeCastTransport {
            status: Some(playing_status(1.0, "https://a")),
            ..Default::default()
        });
        app.set_cast_client("device-1", job_tx);
        app.spawn_cast_status_poll();
        let event = app
            .channels
            .cast_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        let calls = calls.lock().unwrap().clone();
        assert!(calls.contains(&"keep_alive".to_string()));
        assert!(app.cast_attachment.unwrap().status.is_some());
    }

    /// The keep-alive heartbeat is a pong-only job on its own cadence
    /// (`CAST_KEEP_ALIVE_INTERVAL`), independent of the status poll: the
    /// receiver terminates a sender whose pongs lapse, and the poll's 7s
    /// cadence sat at that deadline while the queue load occupied the worker.
    #[test]
    fn spawn_cast_keep_alive_sends_a_pong_on_the_transport() {
        use mbv_cast::{CastJob, FakeCastTransport, spawn_fake_cast_worker};
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, calls) = spawn_fake_cast_worker(FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx.clone());

        app.spawn_cast_keep_alive();
        // Jobs run serially on the worker: when this sentinel runs, the
        // keep-alive job submitted before it has finished.
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let _ = job_tx.send(Box::new(move |_transport: &mut dyn CastTransport| {
            let _ = done_tx.send(());
        }) as CastJob);
        done_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls, vec!["keep_alive".to_string()]);
        // Nothing armed and no poll gate touched: the keep-alive produces no
        // worker event and is invisible to the watchdog.
        assert!(app.cast_job_sent.is_none());
        assert!(!app.cast_status_loading);
    }

    #[test]
    fn an_idle_receiver_stays_attached_and_dispatchable() {
        // cast-session-control "Receiver is idle": an idle report is a
        // status, not a connection loss. Before the client mapped an empty
        // report to an idle status, the first poll after attach against an
        // idle receiver errored, the target was marked disconnected, and the
        // transport was dropped -- nothing could ever be dispatched to it.
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);

        app.apply_cast_status("device-1", Ok(CastStatus::idle()));

        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(!attachment.disconnected);
        assert!(attachment.client.is_some(), "the transport stays usable");
        assert_eq!(App::cast_now_playing_title(attachment), None);
    }

    fn connect_ok_stub(
        id: &str,
        _timeout: Duration,
    ) -> Result<std::sync::mpsc::Sender<mbv_cast::CastJob>, CastError> {
        // The stub only serves the receiver this test attaches to; anything
        // else is as unreachable as a real connect to an unknown id.
        if id != "device-1" {
            return Err(CastError::receiver_not_found());
        }
        let (tx, _calls) = mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        Ok(tx)
    }

    fn connect_failing_stub(
        _id: &str,
        _timeout: Duration,
    ) -> Result<std::sync::mpsc::Sender<mbv_cast::CastJob>, CastError> {
        Err(CastError::receiver_not_found())
    }

    /// The unresponsive-worker watchdog: a submitted job with no worker
    /// event within the answer bound is a wedged worker (`rust_cast` has no
    /// socket timeout). The target is presented disconnected and the
    /// reconnect relaunches the receiver's app session, so playback works
    /// again without a manual re-attach.
    #[test]
    fn an_unresponsive_cast_target_is_presented_disconnected_and_reconnected() {
        let _guard = crate::app::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_ok_stub);
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);
        app.cast_job_sent = Some(
            Instant::now()
                .checked_sub(CAST_JOB_ANSWER_BOUND + Duration::from_secs(1))
                .unwrap(),
        );

        app.check_cast_worker_responsive();

        assert!(
            app.status.contains("stopped responding"),
            "the wedge is flashed, got {:?}",
            app.status
        );
        // The reconnect re-attaches through the overridden connect and
        // restores a usable transport.
        let event = app
            .channels
            .cast_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = None;
        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(!attachment.disconnected);
        assert!(attachment.client.is_some(), "the transport is usable again");
    }

    /// The wedge the watchdog recovers can be a poll exchange mid-flight: its
    /// `StatusUpdated` will never arrive, so `cast_status_loading` stays up
    /// through recovery. The reconnect must clear it, or the tick gate
    /// (`drains.rs`) never spawns another poll, the keep-alive heartbeat
    /// stops, and the watchdog cannot re-arm (ff63b1db9 review).
    #[test]
    fn watchdog_recovery_clears_the_stuck_poll_gate_so_polling_resumes() {
        let _guard = crate::app::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_ok_stub);
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);
        // The stuck state a wedged poll leaves: the loading gate is up and the
        // watchdog clock is already past the answer bound.
        app.cast_status_loading = true;
        app.cast_job_sent = Some(
            Instant::now()
                .checked_sub(CAST_JOB_ANSWER_BOUND + Duration::from_secs(1))
                .unwrap(),
        );

        app.check_cast_worker_responsive();
        let event = app
            .channels
            .cast_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = None;

        assert!(
            !app.cast_status_loading,
            "the reconnect resets the poll gate so ticks spawn polls again"
        );
    }

    #[test]
    fn a_responsive_cast_worker_is_not_touched_by_the_watchdog() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);
        app.cast_job_sent = Some(Instant::now());

        app.check_cast_worker_responsive();

        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(!attachment.disconnected);
        assert!(attachment.client.is_some());
    }

    /// A late event from a previous attachment's worker (the watchdog
    /// reconnect race) must not disturb the current attachment's watchdog
    /// bookkeeping: the clock stays armed for the in-flight job and the poll
    /// gate stays closed around its still-outstanding poll.
    #[test]
    fn a_stale_receivers_status_event_leaves_the_watchdog_bookkeeping_alone() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        app.cast_job_sent = Some(Instant::now());
        app.cast_status_loading = true;

        app.handle_cast_event(CastEvent::StatusUpdated {
            receiver_id: "device-2".to_string(),
            status: Ok(CastStatus::idle()),
        });

        assert!(
            app.cast_job_sent.is_some(),
            "the watchdog clock stays armed for the in-flight job"
        );
        assert!(
            app.cast_status_loading,
            "the poll gate stays closed around the in-flight poll"
        );
    }

    /// The `worker_gone` dispatch outcome carries an empty receiver id: no
    /// worker event will ever arrive for that job, so the event must not
    /// disarm the watchdog -- the reconnect it triggers is the recovery.
    #[test]
    fn a_worker_gone_dispatch_leaves_the_watchdog_armed_for_the_reconnect() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        app.cast_job_sent = Some(Instant::now());

        app.handle_cast_event(CastEvent::Dispatched {
            receiver_id: String::new(),
            outcome: Err(CastError::worker_gone()),
            uncastable: Vec::new(),
        });

        assert!(
            app.cast_job_sent.is_some(),
            "the watchdog clock stays armed so the reconnect fires"
        );
    }

    #[test]
    fn a_dropped_connection_presents_disconnected_stops_polling_and_leaves_the_queue_intact() {
        let _guard = crate::app::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_failing_stub);
        let mut app = make_app_stub();
        let items = crate::app::tests::make_items(2);
        app.local_view.adopt_items(items.clone(), 0);
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);

        app.apply_cast_status("device-1", Err(CastError::not_connected()));

        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(attachment.disconnected);
        assert!(attachment.client.is_none());
        assert_eq!(
            App::cast_now_playing_title(attachment).as_deref(),
            Some("Cast: disconnected")
        );
        assert_eq!(
            app.local_view
                .slots()
                .iter()
                .map(|slot| slot.item.content_id())
                .collect::<Vec<_>>(),
            items
                .iter()
                .map(|item| mbv_queue::QueueItemContentId::Emby(item.id.clone()))
                .collect::<Vec<_>>()
        );

        // The loss reconnects; a failed reconnect detaches instead of
        // looping: the recovery stays bounded.
        let event = app
            .channels
            .cast_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = None;
        assert!(
            !app.is_cast_attached(),
            "a failed reconnect detaches instead of looping"
        );
    }

    /// The loss recovery: the reconnect a dropped connection spawns
    /// re-attaches and restores a usable transport -- the same recovery the
    /// unresponsive-worker watchdog gets.
    #[test]
    fn a_dropped_connection_reconnects_and_recovers_the_target() {
        let _guard = crate::app::CAST_CONNECT_TEST_LOCK.lock().unwrap();
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = Some(connect_ok_stub);
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string(), "Living Room".to_string());
        let (job_tx, _calls) =
            mbv_cast::spawn_fake_cast_worker(mbv_cast::FakeCastTransport::default());
        app.set_cast_client("device-1", job_tx);

        app.apply_cast_status("device-1", Err(CastError::not_connected()));
        assert!(app.cast_attachment.as_ref().unwrap().disconnected);

        let event = app
            .channels
            .cast_rx
            .recv_timeout(Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        *crate::app::CAST_CONNECT_OVERRIDE.lock().unwrap() = None;

        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(!attachment.disconnected);
        assert!(attachment.client.is_some(), "the transport is usable again");
    }
}
