impl EmbyClient {
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
        log::info!(target: "api", "outbound: Playing item={} msid={media_source_id} pos={}", item.id, item.playback_position_ticks);
        match self.post("/Sessions/Playing").send_json(body.clone()) {
            Ok(r) => {
                log::info!(target: "api", "inbound: {} Playing", r.status());
                true
            }
            Err(e) => {
                log::warn!(target: "api", "err: Playing: {e}, retrying...");
                std::thread::sleep(std::time::Duration::from_millis(500));
                match self.post("/Sessions/Playing").send_json(body) {
                    Ok(r) => {
                        log::info!(target: "api", "inbound: {} Playing (retry)", r.status());
                        true
                    }
                    Err(e) => {
                        log::warn!(target: "api", "err: Playing retry failed: {e}");
                        false
                    }
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_progress_ws(
        &self,
        item_id: &ItemId,
        media_source_id: &MediaSourceId,
        position_ticks: i64,
        runtime_ticks: i64,
        is_paused: bool,
        session_id: &EmbySessionId,
        event_name: &str,
        ws_tx: &crate::ws::WsSender,
    ) {
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
        log::info!(target: "api", "outbound: ws Progress pos={pos_s}s/{run_s}s paused={is_paused} event={event_name}");
        if ws_tx.send_text(msg).is_err() {
            log::warn!(target: "api", "ws channel disconnected, falling back to HTTP");
            self.report_progress_http(
                item_id,
                media_source_id,
                position_ticks,
                is_paused,
                session_id,
                event_name,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn report_progress_http(
        &self,
        item_id: &ItemId,
        media_source_id: &MediaSourceId,
        position_ticks: i64,
        is_paused: bool,
        session_id: &EmbySessionId,
        event_name: &str,
    ) {
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
        log::debug!(target: "api", "outbound: Progress pos={position_ticks} paused={is_paused} event={event_name}");
        match self.post("/Sessions/Playing/Progress").send_json(body) {
            Ok(r) => log::debug!(target: "api", "inbound: {} Progress", r.status()),
            Err(e) => log::warn!(target: "api",  "err: Progress: {e}"),
        }
    }

    pub fn report_ping(&self, session_id: &EmbySessionId) {
        log::debug!(target: "api", "outbound: Ping session={session_id}");
        match self
            .post("/Sessions/Playing/Ping")
            .query("PlaySessionId", session_id.as_str())
            .send("")
        {
            Ok(r) => log::debug!(target: "api", "inbound: {} Ping", r.status()),
            Err(e) => log::warn!(target: "api",  "err: Ping: {e}"),
        }
    }

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
        log::info!(target: "api", "outbound: Stopped pos={position_ticks}");
        match self
            .post("/Sessions/Playing/Stopped")
            .send_json(body.clone())
        {
            Ok(r) => {
                log::info!(target: "api", "inbound: {} Stopped", r.status());
                true
            }
            Err(e) => {
                log::warn!(target: "api", "err: Stopped: {e}, retrying...");
                std::thread::sleep(std::time::Duration::from_millis(500));
                match self.post("/Sessions/Playing/Stopped").send_json(body) {
                    Ok(r) => {
                        log::info!(target: "api", "inbound: {} Stopped (retry)", r.status());
                        true
                    }
                    Err(e) => {
                        log::warn!(target: "api", "err: Stopped retry failed: {e}");
                        false
                    }
                }
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
        log::info!(
            target: "api",
            "outbound: Stopped shutdown pos={position_ticks} timeout={}ms",
            hard_bound.as_millis()
        );
        let result = crate::bounded::run_with_hard_bound(
            move || {
                client
                    .post("/Sessions/Playing/Stopped")
                    .send_json(body)
                    .map(|r| r.status())
                    .map_err(|e| e.to_string())
            },
            hard_bound,
        );
        let elapsed_ms = started.elapsed().as_millis();
        match result {
            Ok(status) => {
                log::info!(target: "api", "inbound: {status} Stopped shutdown in {elapsed_ms}ms");
                true
            }
            Err(e) if e.starts_with("timed out after ") => {
                log::warn!(target: "api", "err: Stopped shutdown timed out after {elapsed_ms}ms: {e}");
                false
            }
            Err(e) => {
                log::warn!(target: "api", "err: Stopped shutdown failed without retry after {elapsed_ms}ms: {e}");
                false
            }
        }
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
        log::info!(target: "api", "outbound: Capabilities");
        match self.post("/Sessions/Capabilities/Full").send_json(body) {
            Ok(r) => log::info!(target: "api", "inbound: {} Capabilities", r.status()),
            Err(e) => log::warn!(target: "api", "err: Capabilities: {e}"),
        }
    }
}
