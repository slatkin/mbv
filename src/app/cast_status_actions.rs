// Status polling (6.1), now-playing presentation (6.2), position
// extrapolation (6.3, 6.4), matching a reported entry to a dispatched item
// (6.5), and progress reporting to that item's provider (6.6).

use super::types_cast::{CastAttachment, CastEvent, CastProgressTarget, DispatchedCastItem};
use super::App;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::audiobookshelf::{AudiobookshelfClient, AudiobookshelfPlaybackProgress};
use mbv_core::cast_client::{CastPlaybackState, CastStatus};
use std::time::{Duration, Instant};

/// design.md targets 5-10s; the receiver drops an unanswered sender after
/// roughly 90s (see `CastClient::keep_alive`'s doc comment), so any cadence
/// in that range keeps the heartbeat alive with margin to spare.
pub(super) const CAST_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(7);

/// A progress report computed from a matched dispatched item, decoupled from
/// how it's actually sent so the matching+extrapolation core (6.5, and the
/// "is there anything to report" half of 6.6) is unit-testable without a
/// live Emby/Audiobookshelf/feed-store client.
struct CastProgressReport {
    position_seconds: f32,
    is_paused: bool,
}

impl App {
    pub(super) fn drain_cast_events(&mut self) -> bool {
        let mut produced = false;
        while let Ok(event) = self.cast_rx.try_recv() {
            produced = true;
            self.handle_cast_event(event);
        }
        produced
    }

    /// Polls the attached receiver's status on `CAST_STATUS_POLL_INTERVAL` by
    /// submitting a job to its worker thread. Calling `keep_alive()` every
    /// tick is not optional plumbing -- see design.md Risks: an unanswered
    /// sender is dropped in ~90s.
    pub(super) fn spawn_cast_status_poll(&mut self) {
        let Some(attachment) = self.cast_attachment.as_ref() else {
            return;
        };
        let Some(client) = attachment.client.clone() else {
            return;
        };
        let receiver_id = attachment.receiver_id.clone();
        self.cast_status_loading = true;
        self.last_cast_poll = Instant::now();
        let tx = self.cast_tx.clone();
        let _ = client.send(Box::new(move |transport| {
            if let Err(e) = transport.keep_alive() {
                log::warn!(target: "cast", "cast keep_alive failed: {e}");
            }
            let status = transport.status();
            let _ = tx.send(CastEvent::StatusUpdated {
                receiver_id,
                status,
            });
        }));
    }

    pub(super) fn apply_cast_status(
        &mut self,
        receiver_id: String,
        status: Result<CastStatus, String>,
    ) {
        self.cast_status_loading = false;
        let Some(attachment) = self.cast_attachment.as_mut() else {
            return;
        };
        if attachment.receiver_id != receiver_id {
            return;
        }
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
                // (`player_tab.queue`, untouched here) are left exactly as
                // they were; only progress reporting and further polling
                // stop.
                log::warn!(target: "cast", "cast status poll failed: {e}");
                attachment.disconnected = true;
                attachment.client = None;
            }
        }
    }

    /// `None` when no cast target is attached; the caller falls back to
    /// local/remote-session playback state.
    pub(super) fn cast_effective_playback_state(&self) -> Option<super::PlaybackState> {
        let cast = self.cast_attachment.as_ref()?;
        let Some(status) = cast.status.as_ref() else {
            // Attached and dispatched, but no status polled yet: present as
            // active so controls appear immediately (optimistic, matching
            // `attach_cast`'s optimistic-attach shape) rather than idle.
            return Some(super::PlaybackState {
                active: !cast.dispatched.is_empty(),
                active_idx: self.cast_active_queue_index(cast).unwrap_or(0),
                position_ticks: 0,
                runtime_ticks: 0,
                paused: false,
            });
        };
        let active = status.state != CastPlaybackState::Idle;
        let position_seconds = cast_extrapolate(status, cast.status_at).unwrap_or(0.0);
        let runtime_ticks =
            (status.duration_seconds.unwrap_or(0.0) as f64 * TICKS_PER_SECOND as f64) as i64;
        Some(super::PlaybackState {
            active,
            active_idx: self.cast_active_queue_index(cast).unwrap_or(0),
            position_ticks: (position_seconds as f64 * TICKS_PER_SECOND as f64) as i64,
            runtime_ticks,
            paused: status.state == CastPlaybackState::Paused,
        })
    }

    fn cast_active_queue_index(&self, cast: &CastAttachment) -> Option<usize> {
        let status = cast.status.as_ref()?;
        let item = match_cast_dispatched_item(cast, status)?;
        self.player_tab
            .queue
            .slots()
            .iter()
            .position(|slot| slot.item.content_id() == item.content_id)
    }

    /// Title text for the now-playing header while a cast target is attached
    /// (6.2). `None` when the receiver reports no active media.
    pub(super) fn cast_now_playing_title(&self, cast: &CastAttachment) -> Option<String> {
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

    pub(super) fn cast_extrapolated_position_seconds(&self) -> Option<f32> {
        let cast = self.cast_attachment.as_ref()?;
        cast_extrapolate(cast.status.as_ref()?, cast.status_at)
    }

    /// Reports progress to the matched item's provider (6.6). No-op when
    /// nothing is attached, no status has been reported yet, or the
    /// receiver's playing entry can't be matched back to a dispatched item.
    pub(super) fn report_cast_progress(&mut self) {
        let Some(cast) = self.cast_attachment.as_ref() else {
            return;
        };
        let Some(status) = cast.status.as_ref() else {
            return;
        };
        let Some((item, report)) = cast_progress_for_status(cast, status) else {
            return;
        };
        let position_ticks = (report.position_seconds as f64 * TICKS_PER_SECOND as f64) as i64;
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
                let item_id = mbv_core::ItemId::new(item_id.clone());
                let media_source_id = media_source_id.clone();
                let session_id = session_id.clone();
                let is_paused = report.is_paused;
                let _ = *runtime_ticks;
                std::thread::spawn(move || {
                    client.report_progress_http(
                        &item_id,
                        &media_source_id,
                        position_ticks,
                        is_paused,
                        &session_id,
                        "timeupdate",
                    );
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
                let position_seconds = report.position_seconds as f64;
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
    use crate::app::tests::make_app_stub;
    use crate::app::types_cast::CastProgressTarget as ProgressTarget;
    use mbv_core::cast_client::CastStatus;
    use mbv_core::playback_queue::QueueItemContentId;

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
    fn extrapolates_steady_playback_by_elapsed_time_and_rate() {
        let mut status = playing_status(10.0, "https://a");
        status.playback_rate = 2.0;
        let status_at = Instant::now() - Duration::from_secs(3);
        let position = cast_extrapolate(&status, status_at).unwrap();
        // 10s reported + 3s elapsed * 2.0 rate = 16s, with slack for test timing.
        assert!((15.5..=16.5).contains(&position), "position={position}");
    }

    #[test]
    fn holds_position_while_buffering() {
        let mut status = playing_status(10.0, "https://a");
        status.state = CastPlaybackState::Buffering;
        let status_at = Instant::now() - Duration::from_secs(5);
        assert_eq!(cast_extrapolate(&status, status_at), Some(10.0));
    }

    #[test]
    fn holds_position_while_paused() {
        let mut status = playing_status(10.0, "https://a");
        status.state = CastPlaybackState::Paused;
        let status_at = Instant::now() - Duration::from_secs(5);
        assert_eq!(cast_extrapolate(&status, status_at), Some(10.0));
    }

    #[test]
    fn a_fresh_status_report_corrects_drift() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        app.cast_attachment.as_mut().unwrap().dispatched = vec![dispatched("https://a")];
        app.apply_cast_status(
            "device-1".to_string(),
            Ok(playing_status(10.0, "https://a")),
        );
        // A later report disagrees with where extrapolation from the first
        // report would have put the position by now.
        app.apply_cast_status(
            "device-1".to_string(),
            Ok(playing_status(50.0, "https://a")),
        );
        let position = app.cast_extrapolated_position_seconds().unwrap();
        assert!((49.5..=50.5).contains(&position), "position={position}");
    }

    #[test]
    fn matched_item_produces_a_progress_report() {
        let cast = attachment(vec![dispatched("https://a")]);
        let status = playing_status(10.0, "https://a");
        assert!(cast_progress_for_status(&cast, &status).is_some());
    }

    #[test]
    fn unmatched_item_produces_no_progress_report() {
        let cast = attachment(vec![dispatched("https://a")]);
        let status = playing_status(10.0, "https://not-dispatched");
        assert!(cast_progress_for_status(&cast, &status).is_none());
    }

    #[test]
    fn now_playing_title_is_none_while_idle() {
        let mut cast = attachment(vec![dispatched("https://a")]);
        cast.status = Some(CastStatus {
            position_seconds: None,
            duration_seconds: None,
            playback_rate: 1.0,
            state: CastPlaybackState::Idle,
            playing_content_id: None,
        });
        let app = make_app_stub();
        assert_eq!(app.cast_now_playing_title(&cast), None);
    }

    #[test]
    fn now_playing_title_is_the_matched_dispatched_item_while_playing() {
        let mut cast = attachment(vec![dispatched("https://a")]);
        cast.status = Some(playing_status(10.0, "https://a"));
        let app = make_app_stub();
        assert_eq!(app.cast_now_playing_title(&cast).as_deref(), Some("Title"));
    }

    #[test]
    fn status_update_stores_position_duration_rate_state_and_playing_entry() {
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        app.apply_cast_status(
            "device-1".to_string(),
            Ok(playing_status(12.5, "https://a")),
        );
        let status = app
            .cast_attachment
            .as_ref()
            .unwrap()
            .status
            .clone()
            .unwrap();
        assert_eq!(status.position_seconds, Some(12.5));
        assert_eq!(status.duration_seconds, Some(100.0));
        assert_eq!(status.playback_rate, 1.0);
        assert_eq!(status.state, CastPlaybackState::Playing);
        assert_eq!(status.playing_content_id.as_deref(), Some("https://a"));
    }

    #[test]
    fn spawn_cast_status_poll_calls_keep_alive_and_status_on_the_transport() {
        use super::super::types_cast::{spawn_fake_cast_worker, FakeCastTransport};
        let mut app = make_app_stub();
        app.attach_cast("device-1".to_string());
        let (job_tx, calls) = spawn_fake_cast_worker(FakeCastTransport {
            status: Ok(playing_status(1.0, "https://a")),
            ..Default::default()
        });
        app.set_cast_client("device-1", job_tx);
        app.spawn_cast_status_poll();
        let event = app
            .cast_rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        app.handle_cast_event(event);
        let calls = calls.lock().unwrap().clone();
        assert!(calls.contains(&"keep_alive".to_string()));
        assert!(app.cast_attachment.unwrap().status.is_some());
    }

    #[test]
    fn a_dropped_connection_presents_disconnected_stops_polling_and_leaves_the_queue_intact() {
        let mut app = make_app_stub();
        let items = crate::app::tests::make_items(2);
        app.player_tab.set_items(items.clone(), 0);
        app.attach_cast("device-1".to_string());
        let (job_tx, _calls) = super::super::types_cast::spawn_fake_cast_worker(
            super::super::types_cast::FakeCastTransport::default(),
        );
        app.set_cast_client("device-1", job_tx);

        app.apply_cast_status("device-1".to_string(), Err("connection lost".to_string()));

        let attachment = app.cast_attachment.as_ref().unwrap();
        assert!(attachment.disconnected);
        assert!(attachment.client.is_none());
        assert_eq!(
            app.cast_now_playing_title(attachment).as_deref(),
            Some("Cast: disconnected")
        );
        assert_eq!(
            app.player_tab
                .queue
                .slots()
                .iter()
                .map(|slot| slot.item.content_id())
                .collect::<Vec<_>>(),
            items
                .iter()
                .map(|item| mbv_core::playback_queue::QueueItemContentId::Emby(item.id.clone()))
                .collect::<Vec<_>>()
        );
    }
}
