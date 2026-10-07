use super::EmbyClient;
use mbv_emby_model::{EmbyItem, TICKS_PER_SECOND};
use mbv_ids::{EmbySessionId, ItemId, MediaSourceId};

/// One progress report for both progress transports. The two transports
/// previously took the same adjacent same-type args (`position_ticks` /
/// `runtime_ticks`) positionally, so a swap compiled silently and corrupted
/// reported progress; named fields make that impossible. `runtime_ticks`
/// is only read by the ws transport (log line); http ignores it.
#[derive(Debug)]
pub struct ProgressReport {
    pub item_id: ItemId,
    pub media_source_id: MediaSourceId,
    pub position_ticks: i64,
    pub runtime_ticks: i64,
    pub is_paused: bool,
    pub session_id: EmbySessionId,
    pub event_name: String,
}

impl EmbyClient {
    #[must_use]
    pub fn report_start(
        &self,
        item: &EmbyItem,
        media_source_id: &MediaSourceId,
        session_id: &EmbySessionId,
    ) -> bool {
        let body = serde_json::json!({
            "UserId": self.user_id,
            "ItemId": item.id,
            "MediaSourceId": media_source_id.as_str(),
            "PlaySessionId": session_id.as_str(),
            "CanSeek": true,
            "IsPaused": false,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "PositionTicks": item.playback_position_ticks,
            "RunTimeTicks": item.runtime_ticks,
            "QueueableMediaTypes": ["Audio", "Video"],
        });
        tracing::info!(name: "emby.playback_start.requested", target: "api", item = %item.id, media_source = %media_source_id, position_ticks = item.playback_position_ticks, "reporting playback start");
        match self.post("/Sessions/Playing").send_json(body.clone()) {
            Ok(r) => {
                tracing::info!(name: "emby.playback_start.reported", target: "api", http_response_status_code = r.status().as_u16(), "playback start reported");
                true
            }
            Err(e) => self.retry_report_start(body, &e),
        }
    }

    /// Second attempt for [`Self::report_start`], covering both the first
    /// failure's log line and the retry outcome.
    fn retry_report_start(&self, body: serde_json::Value, first_error: &ureq::Error) -> bool {
        tracing::warn!(name: "emby.playback_start.report_failed", target: "api", error = %first_error, "playback start report failed; retrying");
        std::thread::sleep(std::time::Duration::from_millis(500));
        match self.post("/Sessions/Playing").send_json(body) {
            Ok(r) => {
                tracing::info!(name: "emby.playback_start.retry_reported", target: "api", http_response_status_code = r.status().as_u16(), "playback start retry reported");
                true
            }
            Err(e) => {
                tracing::warn!(name: "emby.playback_start.retry_failed", target: "api", error = %e, "playback start retry failed");
                false
            }
        }
    }

    pub fn report_progress_ws(&self, report: &ProgressReport, ws_tx: &mbv_ws::WsSender) {
        let ProgressReport {
            item_id,
            media_source_id,
            position_ticks,
            runtime_ticks,
            is_paused,
            session_id,
            event_name,
        } = report;
        let data = serde_json::json!({
            "UserId": self.user_id,
            "ItemId": item_id.as_str(),
            "MediaSourceId": media_source_id.as_str(),
            "PlaySessionId": session_id.as_str(),
            "CanSeek": true,
            "IsPaused": is_paused,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "PositionTicks": position_ticks,
            "EventName": event_name,
            "QueueableMediaTypes": ["Audio", "Video"],
        });
        let msg = serde_json::json!({
            "MessageType": "ReportPlaybackProgress",
            "Data": data,
        })
        .to_string();
        let pos_s = position_ticks / TICKS_PER_SECOND;
        let run_s = runtime_ticks / TICKS_PER_SECOND;
        tracing::debug!(name: "emby.progress.websocket_sent", target: "api", position_seconds = pos_s, runtime_seconds = run_s, is_paused = *is_paused, playback_event = %event_name, "sent WebSocket progress");
        if ws_tx.send_text(msg).is_err() {
            tracing::warn!(name: "emby.progress.websocket_disconnected", target: "api", "WebSocket channel disconnected; falling back to HTTP");
            self.report_progress_http(report);
        }
    }

    pub fn report_progress_http(&self, report: &ProgressReport) {
        let ProgressReport {
            item_id,
            media_source_id,
            position_ticks,
            is_paused,
            session_id,
            event_name,
            ..
        } = report;
        let body = serde_json::json!({
            "UserId": self.user_id,
            "ItemId": item_id.as_str(),
            "MediaSourceId": media_source_id.as_str(),
            "PlaySessionId": session_id.as_str(),
            "CanSeek": true,
            "IsPaused": is_paused,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "PositionTicks": position_ticks,
            "EventName": event_name,
            "QueueableMediaTypes": ["Audio", "Video"],
        });
        tracing::debug!(name: "emby.progress.requested", target: "api", position_ticks, is_paused = *is_paused, playback_event = %event_name, "reporting progress");
        match self.post("/Sessions/Playing/Progress").send_json(body) {
            Ok(r) => {
                tracing::debug!(name: "emby.progress.reported", target: "api", http_response_status_code = r.status().as_u16(), "progress reported");
            }
            Err(e) => {
                tracing::warn!(name: "emby.progress.report_failed", target: "api", error = %e, "progress report failed");
            }
        }
    }

    pub fn report_ping(&self, session_id: &EmbySessionId) {
        tracing::debug!(name: "emby.ping.requested", target: "api", play_session = %session_id, "reporting playback ping");
        match self
            .post("/Sessions/Playing/Ping")
            .query("PlaySessionId", session_id.as_str())
            .send("")
        {
            Ok(r) => {
                tracing::debug!(name: "emby.ping.reported", target: "api", http_response_status_code = r.status().as_u16(), "playback ping reported");
            }
            Err(e) => {
                tracing::warn!(name: "emby.ping.report_failed", target: "api", error = %e, "playback ping report failed");
            }
        }
    }

    #[must_use]
    pub fn report_stopped(
        &self,
        item_id: &ItemId,
        media_source_id: &MediaSourceId,
        position_ticks: i64,
        session_id: &EmbySessionId,
        runtime_ticks: i64,
    ) -> bool {
        let body = self.stopped_request_body(
            item_id,
            media_source_id,
            position_ticks,
            session_id,
            runtime_ticks,
        );
        tracing::info!(name: "emby.playback_stop.requested", target: "api", position_ticks, "reporting playback stopped");
        match self
            .post("/Sessions/Playing/Stopped")
            .send_json(body.clone())
        {
            Ok(r) => {
                tracing::info!(name: "emby.playback_stop.reported", target: "api", http_response_status_code = r.status().as_u16(), "playback stop reported");
                true
            }
            Err(e) => self.retry_report_stopped(body, &e),
        }
    }

    /// Second attempt for [`Self::report_stopped`], covering both the first
    /// failure's log line and the retry outcome.
    fn retry_report_stopped(&self, body: serde_json::Value, first_error: &ureq::Error) -> bool {
        tracing::warn!(name: "emby.playback_stop.report_failed", target: "api", error = %first_error, "playback stop report failed; retrying");
        std::thread::sleep(std::time::Duration::from_millis(500));
        match self.post("/Sessions/Playing/Stopped").send_json(body) {
            Ok(r) => {
                tracing::info!(name: "emby.playback_stop.retry_reported", target: "api", http_response_status_code = r.status().as_u16(), "playback stop retry reported");
                true
            }
            Err(e) => {
                tracing::warn!(name: "emby.playback_stop.retry_failed", target: "api", error = %e, "playback stop retry failed");
                false
            }
        }
    }

    fn stopped_request_body(
        &self,
        item_id: &ItemId,
        media_source_id: &MediaSourceId,
        position_ticks: i64,
        session_id: &EmbySessionId,
        runtime_ticks: i64,
    ) -> serde_json::Value {
        serde_json::json!({
            "UserId": self.user_id,
            "ItemId": item_id.as_str(),
            "MediaSourceId": media_source_id.as_str(),
            "PlaySessionId": session_id.as_str(),
            "PositionTicks": position_ticks,
            "RunTimeTicks": runtime_ticks,
            "CanSeek": true,
            "IsPaused": false,
            "IsMuted": false,
            "PlayMethod": "DirectPlay",
            "QueueableMediaTypes": ["Audio", "Video"],
        })
    }

    #[must_use]
    pub fn report_stopped_for_shutdown(
        &self,
        item_id: &ItemId,
        media_source_id: &MediaSourceId,
        position_ticks: i64,
        session_id: &EmbySessionId,
        runtime_ticks: i64,
        hard_bound: std::time::Duration,
    ) -> bool {
        let body = self.stopped_request_body(
            item_id,
            media_source_id,
            position_ticks,
            session_id,
            runtime_ticks,
        );
        let client = self.with_request_timeout(hard_bound);
        let started = std::time::Instant::now();
        tracing::info!(
            name: "emby.playback_stop.shutdown_requested",
            target: "api",
            position_ticks,
            timeout_ms = hard_bound.as_millis(),
            "reporting playback stopped for shutdown"
        );
        let result = Self::bounded_stopped_report(client, body, hard_bound);
        let elapsed_ms = started.elapsed().as_millis();
        Self::shutdown_report_outcome(result, elapsed_ms)
    }

    /// Runs the shutdown stop report under its hard bound, mapping the HTTP
    /// status to a plain code so callers need no response type.
    fn bounded_stopped_report(
        client: Self,
        body: serde_json::Value,
        hard_bound: std::time::Duration,
    ) -> Result<u16, crate::EmbyError> {
        mbv_net::bounded::run_with_hard_bound_or_error(
            move || -> Result<_, crate::EmbyError> {
                let status = client
                    .post("/Sessions/Playing/Stopped")
                    .send_json(body)?
                    .status();
                Ok(status.as_u16())
            },
            {
                let secs = hard_bound.as_secs();
                move || crate::EmbyError::bounded_timeout(format!("timed out after {secs}s"))
            },
            hard_bound,
        )
    }

    fn shutdown_report_outcome(result: Result<u16, crate::EmbyError>, elapsed_ms: u128) -> bool {
        match result {
            Ok(status) => {
                tracing::info!(name: "emby.playback_stop.shutdown_reported", target: "api", http_response_status_code = status, duration_ms = elapsed_ms, "playback stop reported for shutdown");
                true
            }
            Err(e) => Self::shutdown_report_failure(&e, elapsed_ms),
        }
    }

    fn shutdown_report_failure(error: &crate::EmbyError, elapsed_ms: u128) -> bool {
        if error.is_bounded_timeout() {
            tracing::warn!(name: "emby.playback_stop.shutdown_timed_out", target: "api", duration_ms = elapsed_ms, error = %error, "playback stop shutdown report timed out");
        } else {
            tracing::warn!(name: "emby.playback_stop.shutdown_failed", target: "api", duration_ms = elapsed_ms, error = %error, "playback stop shutdown report failed");
        }
        false
    }

    /// Register with the client's configured audio-pipe setting.
    pub fn register_capabilities(&self) {
        self.register_capabilities_with_options(&[], self.config.audio_pipe_enabled);
    }

    pub fn register_capabilities_with_options(&self, extra_commands: &[String], audio_only: bool) {
        let media_types: &[&str] = if audio_only {
            &["Audio"]
        } else {
            &["Audio", "Video"]
        };
        let mut commands: Vec<String> = vec![
            "Play",
            "Stop",
            "Pause",
            "Unpause",
            "NextTrack",
            "PreviousTrack",
            "Seek",
            "SetVolume",
            "VolumeUp",
            "VolumeDown",
            "Mute",
            "Unmute",
            "ToggleMute",
            "SetAudioStreamIndex",
            "SetSubtitleStreamIndex",
        ]
        .into_iter()
        .map(str::to_string)
        .collect();
        if audio_only {
            // No video window ever opens in audio-pipe mode, so subtitles can
            // never be displayed — don't advertise a command that can't work.
            commands.retain(|c| c != "SetSubtitleStreamIndex");
        }
        commands.extend(extra_commands.iter().cloned());
        let body = serde_json::json!({
            "PlayableMediaTypes": media_types,
            "SupportedCommands": commands,
            "SupportsMediaControl": true,
            "SupportsSync": false
        });
        tracing::debug!(name: "emby.capabilities.requested", target: "api", "registering playback capabilities");
        match self.post("/Sessions/Capabilities/Full").send_json(body) {
            Ok(r) => {
                tracing::debug!(name: "emby.capabilities.registered", target: "api", http_response_status_code = r.status().as_u16(), "playback capabilities registered");
            }
            Err(e) => {
                tracing::warn!(name: "emby.capabilities.registration_failed", target: "api", error = %e, "playback capabilities registration failed");
            }
        }
    }
}
