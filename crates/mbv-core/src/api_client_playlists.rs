impl EmbyClient {
    pub fn get_playback_info(&self, item_id: &str) -> PlaybackInfo {
        let body = ureq::json!({
            "UserId": self.user_id,
            "MaxStreamingBitrate": 140000000,
            "EnableDirectPlay": true,
            "EnableDirectStream": false,
            "IsPlayback": true,
        });
        log::info!(target: "api", "outbound: PlaybackInfo item={item_id}");
        let resp: Value = match self
            .post(&format!("/Items/{item_id}/PlaybackInfo"))
            .send_json(body)
        {
            Ok(r) => match r.into_json() {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(target: "api", "err: PlaybackInfo parse: {e}");
                    return PlaybackInfo {
                        session_id: gen_session_id(),
                        media_source_id: item_id.to_string(),
                        external_subtitle_urls: vec![],
                    };
                }
            },
            Err(e) => {
                log::warn!(target: "api", "err: PlaybackInfo: {e}");
                return PlaybackInfo {
                    session_id: gen_session_id(),
                    media_source_id: item_id.to_string(),
                    external_subtitle_urls: vec![],
                };
            }
        };
        let sid = resp["PlaySessionId"].as_str().unwrap_or("").to_string();
        let msid = resp["MediaSources"][0]["Id"]
            .as_str()
            .unwrap_or(item_id)
            .to_string();
        let sub_urls: Vec<String> = resp["MediaSources"][0]["MediaStreams"]
            .as_array()
            .map(|a| a.as_slice())
            .unwrap_or(&[])
            .iter()
            .filter(|s| {
                s["Type"].as_str() == Some("Subtitle")
                    && s["DeliveryMethod"].as_str() == Some("External")
            })
            .filter_map(|s| s["DeliveryUrl"].as_str())
            .map(|u| format!("{}{}", self.config.server_url, u))
            .collect();
        log::info!(target: "api", "inbound: PlaybackInfo sid={sid} msid={msid} ext_subs={}", sub_urls.len());
        let session_id = if sid.is_empty() {
            gen_session_id()
        } else {
            sid
        };
        PlaybackInfo {
            session_id,
            media_source_id: msid,
            external_subtitle_urls: sub_urls,
        }
    }

    // ── Playlists ────────────────────────────────────────────────────────────

    pub fn get_playlists(&self) -> Result<Vec<MediaItem>, String> {
        self.fetch_items(
            &format!("/Users/{}/Items", self.user_id),
            &[
                ("IncludeItemTypes", "Playlist"),
                ("Recursive", "true"),
                ("Fields", ""),
            ],
        )
    }

    pub fn create_playlist(&self, name: &str, item_ids: &[String]) -> Result<String, String> {
        let body = ureq::json!({
            "Name": name,
            "Ids": item_ids.join(","),
            "UserId": self.user_id,
        });
        let resp: Value = self
            .post("/Playlists")
            .send_json(body)
            .map_err(|e| match e {
                ureq::Error::Status(code, r) => {
                    let body = r.into_string().unwrap_or_default();
                    format!("HTTP {code}: {body}")
                }
                e => e.to_string(),
            })?
            .into_json()
            .map_err(|e| e.to_string())?;
        resp["Id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no Id in response".to_string())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), String> {
        self.delete(&format!("/Items/{}", playlist_id))
            .call()
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Replace a playlist's contents with the given item ids (in order).
    /// Fetches current entry ids, deletes them all, then adds the new set.
    pub fn get_playlist_items(&self, playlist_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: serde_json::Value = self.get(&format!("/Playlists/{}/Items", playlist_id))
            .query("UserId", &self.user_id)
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear,EndDate,Overview,PremiereDate,DateCreated,ChildCount,RecursiveItemCount,Container,People,MediaStreams,Genres")
            .query("EnableUserData", "true")
            .call().map_err(|e| e.to_string())?
            .into_json().map_err(|e| e.to_string())?;
        Ok(resp["Items"]
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn update_playlist_items(
        &self,
        playlist_id: &str,
        item_ids: &[String],
    ) -> Result<(), String> {
        // Get current playlist entry ids
        let resp: serde_json::Value = self
            .get(&format!("/Playlists/{}/Items", playlist_id))
            .query("UserId", &self.user_id)
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        let entry_ids: Vec<String> = resp["Items"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v["PlaylistItemId"].as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default();
        // Delete existing entries
        if !entry_ids.is_empty() {
            self.delete(&format!("/Playlists/{}/Items", playlist_id))
                .query("EntryIds", &entry_ids.join(","))
                .call()
                .map_err(|e| e.to_string())?;
        }
        // Add new items in order
        if !item_ids.is_empty() {
            self.post(&format!("/Playlists/{}/Items", playlist_id))
                .query("Ids", &item_ids.join(","))
                .query("UserId", &self.user_id)
                .call()
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // ── Series / episodes / chapters ────────────────────────────────────────

    pub fn get_items_by_ids(&self, ids: &[String]) -> Result<Vec<MediaItem>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let joined = ids.join(",");
        let mut items = self.fetch_items(&format!("/Users/{}/Items", self.user_id), &[
            ("Ids",    &joined),
            ("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])?;
        // Emby returns items in server sort order, not input order. Restore input order.
        let order: std::collections::HashMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        items.sort_by_key(|item| order.get(item.id.as_str()).copied().unwrap_or(usize::MAX));
        Ok(items)
    }

    pub fn get_ancestors(&self, item_id: &str) -> Result<Vec<MediaItem>, String> {
        let resp: Value = self
            .get(&format!("/Items/{}/Ancestors", item_id))
            .query("Fields", "SortName")
            .call()
            .map_err(|e| e.to_string())?
            .into_json()
            .map_err(|e| e.to_string())?;
        Ok(resp
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    /// Probes for the Chapter API plugin. Sets `chapter_api_available` on self.
    /// Any HTTP response (even 500 for a bad id) means the plugin is installed;
    /// only a 404 or connection failure means it's absent.
    pub fn probe_chapter_api(&mut self) {
        log::info!(target: "api", "outbound: ChapterAPI probe");
        match self
            .get("/chapter_api/get_chapters")
            .query("id", "0")
            .call()
        {
            Ok(_) | Err(ureq::Error::Status(_, _)) => {
                self.chapter_api_available = true;
                log::info!(target: "api", "inbound: ChapterAPI available");
            }
            Err(e) => {
                log::info!(target: "api", "err: ChapterAPI not available: {e}");
            }
        }
    }

    /// Returns `(intro_start_ticks, intro_end_ticks)` for an item if the Chapter API
    /// exposes IntroStart and IntroEnd markers.
    pub fn get_intro_times(&self, item_id: &str) -> Option<(i64, i64)> {
        log::debug!(target: "api", "outbound: ChapterAPI get_chapters item={item_id}");
        let resp = self
            .get("/chapter_api/get_chapters")
            .query("id", item_id)
            .call()
            .ok()?;
        let body: serde_json::Value = resp.into_json().ok()?;
        let chapters = body["chapters"].as_array()?;
        let start = chapters
            .iter()
            .find(|c| c["MarkerType"].as_str() == Some("IntroStart"))?["StartPositionTicks"]
            .as_i64()?;
        let end = chapters
            .iter()
            .find(|c| c["MarkerType"].as_str() == Some("IntroEnd"))?["StartPositionTicks"]
            .as_i64()?;
        log::info!(target: "api", "inbound: ChapterAPI intro start={start} end={end}");
        Some((start, end))
    }

}
