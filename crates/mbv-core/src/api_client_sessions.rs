impl EmbyClient {
    /// Returns all episodes of a series starting from `from_item_id` (inclusive), in air order.
    /// Mirrors Emby Web's `getEpisodes(seriesId)` + filter pattern.
    pub fn get_episodes_from(&self, series_id: &ItemId, from_item_id: &ItemId) -> Vec<EmbyItem> {
        log::debug!(target: "api", "outbound: EpisodesFrom series={series_id} from={from_item_id}");
        let resp: Value = match self
            .get(&format!("/Shows/{}/Episodes", series_id))
            .query("UserId", &self.user_id)
            .query(
                "Fields",
                "UserData,RunTimeTicks,SeriesId,SeriesName,ParentIndexNumber,IndexNumber",
            )
            .call()
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(target: "api", "err: EpisodesFrom parse: {e}");
                    return vec![];
                }
            },
            Err(e) => {
                log::warn!(target: "api", "err: EpisodesFrom: {e}");
                return vec![];
            }
        };
        let Some(all) = resp["Items"].as_array() else {
            return vec![];
        };
        let mut found = false;
        let items: Vec<EmbyItem> = all
            .iter()
            .filter_map(|v| {
                if found {
                    return Some(parse_item(v));
                }
                if v["Id"].as_str().unwrap_or("") == from_item_id.as_str() {
                    found = true;
                    Some(parse_item(v))
                } else {
                    None
                }
            })
            .collect();
        if items.is_empty() {
            // from_item_id not in series — return everything as a fallback
            log::warn!(target: "api", "inbound: EpisodesFrom: from_item_id not found, returning all");
            return all.iter().map(parse_item).collect();
        }
        log::info!(target: "api", "inbound: EpisodesFrom: {} episodes from '{}'", items.len(), items[0].display_name());
        items
    }

    // ── Remote session control ───────────────────────────────────────────────

    fn get_sessions_with_active_within(
        &self,
        active_within_secs: Option<&str>,
    ) -> Result<Vec<SessionInfo>, String> {
        let mut req = self.get("/Sessions");
        if let Some(secs) = active_within_secs {
            req = req.query("ActiveWithinSeconds", secs);
        }
        let arr: Value = req
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let sessions = arr
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| {
                        if v["DeviceId"].as_str().unwrap_or("") == self.device_id {
                            return None;
                        }
                        if !v["SupportsRemoteControl"].as_bool().unwrap_or(false) {
                            return None;
                        }
                        let ps = &v["PlayState"];
                        let npi = &v["NowPlayingItem"];
                        let media_info = npi["MediaStreams"]
                            .as_array()
                            .map(|streams| parse_session_media_info(streams))
                            .unwrap_or_default();
                        let raw_host = v["RemoteEndPoint"].as_str().unwrap_or("");
                        let host = raw_host.rsplit(':').nth(1).unwrap_or(raw_host).to_string();
                        Some(SessionInfo {
                            id: v["Id"].as_str().unwrap_or("").to_string(),
                            device_name: v["DeviceName"].as_str().unwrap_or("").to_string(),
                            client: v["Client"].as_str().unwrap_or("").to_string(),
                            user_name: v["UserName"].as_str().unwrap_or("").to_string(),
                            host,
                            supported_commands: v["SupportedCommands"]
                                .as_array()
                                .map(|arr| {
                                    arr.iter()
                                        .filter_map(|value| value.as_str().map(str::to_string))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            now_playing: npi["Name"].as_str().map(str::to_string),
                            now_playing_item_id: npi["Id"].as_str().map(str::to_string),
                            position_ticks: ps["PositionTicks"].as_i64().unwrap_or(0),
                            runtime_ticks: npi["RunTimeTicks"].as_i64().unwrap_or(0),
                            position_s: ps["PositionTicks"].as_i64().unwrap_or(0)
                                / TICKS_PER_SECOND,
                            runtime_s: npi["RunTimeTicks"].as_i64().unwrap_or(0) / TICKS_PER_SECOND,
                            is_paused: ps["IsPaused"].as_bool().unwrap_or(false),
                            volume: ps["VolumeLevel"].as_i64().unwrap_or(100),
                            sub_index: ps["SubtitleStreamIndex"].as_i64().unwrap_or(-1),
                            audio_index: ps["AudioStreamIndex"].as_i64().unwrap_or(0),
                            muted: ps["IsMuted"].as_bool().unwrap_or(false),
                            media_info,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(sessions)
    }

    pub fn get_sessions(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_with_active_within(Some("600"))
    }

    /// Like `get_sessions`, but without the `ActiveWithinSeconds=600` filter:
    /// a device that's been idle-but-still-connected for more than 10 minutes
    /// wouldn't show up in the filtered list, which would make a live-session
    /// lookup wrongly conclude the device is gone. Used by
    /// `App::try_auto_reconnect`'s `DirectSession` lookup (#236) and by
    /// library-route device resolution (#239) -- the Sessions-panel (F3) UI
    /// should keep using the filtered `get_sessions` above.
    pub fn get_sessions_unfiltered(&self) -> Result<Vec<SessionInfo>, String> {
        self.get_sessions_with_active_within(None)
    }

    pub fn session_transport(&self, id: &str, cmd: &str) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing/{cmd}"))
            .send_string("")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_seek(&self, id: &str, ticks: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing/Seek"))
            .query("SeekPositionTicks", &ticks.to_string())
            .send_string("")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_volume(&self, id: &str, vol: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Command/SetVolume"))
            .send_json(ureq::json!({"Arguments":{"Volume": vol.to_string()}}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_subtitle_index(&self, id: &str, index: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Command/SetSubtitleStreamIndex"))
            .send_json(ureq::json!({"Arguments":{"Index": index.to_string()}}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_audio_index(&self, id: &str, index: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Command/SetAudioStreamIndex"))
            .send_json(ureq::json!({"Arguments":{"Index": index.to_string()}}))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_set_mute(&self, id: &str, muted: bool) -> Result<(), String> {
        let cmd = if muted { "Mute" } else { "Unmute" };
        self.post(&format!("/Sessions/{id}/Command/{cmd}"))
            .send_string("")
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_play(&self, id: &str, item_id: &str, start_ticks: i64) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing"))
            .send_json(ureq::json!({
                "PlayCommand": "PlayNow",
                "ItemIds": [item_id],
                "StartPositionTicks": start_ticks
            }))
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn session_play_items(
        &self,
        id: &str,
        item_ids: &[String],
        start_idx: usize,
        start_ticks: i64,
    ) -> Result<(), String> {
        self.post(&format!("/Sessions/{id}/Playing"))
            .send_json(ureq::json!({
                "PlayCommand": "PlayNow",
                "ItemIds": item_ids,
                "StartIndex": start_idx,
                "StartPositionTicks": start_ticks
            }))
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}
