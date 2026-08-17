impl EmbyClient {
    pub fn get_playback_info(&self, item_id: &str) -> PlaybackInfo {
        let body = serde_json::json!({
            "UserId": self.user_id,
            "MaxStreamingBitrate": 140000000,
            "EnableDirectPlay": true,
            "EnableDirectStream": false,
            "IsPlayback": true,
        });
        log::info!(target: "api", "outbound: PlaybackInfo item={item_id}");
        let resp: Value = match self
            .post(&format!(
                "/Items/{}/PlaybackInfo",
                crate::encode_path_segment(item_id)
            ))
            .send_json(body)
        {
            Ok(mut r) => match r.body_mut().read_json() {
                Ok(v) => v,
                Err(e) => {
                    log::warn!(target: "api", "err: PlaybackInfo parse: {e}");
                    return PlaybackInfo {
                        session_id: gen_session_id(),
                        media_source_id: MediaSourceId::new(item_id.to_string()),
                        external_subtitle_urls: vec![],
                    };
                }
            },
            Err(e) => {
                log::warn!(target: "api", "err: PlaybackInfo: {e}");
                return PlaybackInfo {
                    session_id: gen_session_id(),
                    media_source_id: MediaSourceId::new(item_id.to_string()),
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
            EmbySessionId::new(sid)
        };
        PlaybackInfo {
            session_id,
            media_source_id: MediaSourceId::new(msid),
            external_subtitle_urls: sub_urls,
        }
    }

    // ── Playlists ────────────────────────────────────────────────────────────

    pub fn get_playlists(&self) -> Result<Vec<EmbyItem>, String> {
        self.fetch_items(
            &format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)),
            &[
                ("IncludeItemTypes", "Playlist"),
                ("Recursive", "true"),
                ("Fields", ""),
            ],
        )
    }

    pub fn create_playlist(&self, name: &str, item_ids: &[String]) -> Result<String, String> {
        let body = serde_json::json!({
            "Name": name,
            "Ids": item_ids.join(","),
            "UserId": self.user_id,
        });
        let resp: Value = self
            .post("/Playlists")
            .send_json(body)
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?;
        resp["Id"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "no Id in response".to_string())
    }

    pub fn delete_playlist(&self, playlist_id: &str) -> Result<(), String> {
        self.delete(&format!(
            "/Items/{}",
            crate::encode_path_segment(playlist_id)
        ))
        .call()
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn rename_playlist(&self, playlist_id: &str, new_name: &str) -> Result<(), String> {
        let body = serde_json::json!({"Name": new_name});
        self.post(&format!(
            "/Items/{}",
            crate::encode_path_segment(playlist_id)
        ))
        .send_json(body)
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    /// Replace a playlist's contents with the given item ids (in order).
    /// Fetches current entry ids, deletes them all, then adds the new set.
    pub fn get_playlist_items(&self, playlist_id: &str) -> Result<Vec<EmbyItem>, String> {
        let resp: serde_json::Value = self.get(&format!("/Playlists/{}/Items", crate::encode_path_segment(playlist_id)))
            .query("UserId", &self.user_id)
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear,EndDate,Overview,PremiereDate,DateCreated,ChildCount,RecursiveItemCount,Container,People,MediaStreams,Genres")
            .query("EnableUserData", "true")
            .call().map_err(|e| e.to_string())?
            .body_mut().read_json().map_err(|e| e.to_string())?;
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
            .get(&format!(
                "/Playlists/{}/Items",
                crate::encode_path_segment(playlist_id)
            ))
            .query("UserId", &self.user_id)
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
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
            self.delete(&format!(
                "/Playlists/{}/Items",
                crate::encode_path_segment(playlist_id)
            ))
            .query("EntryIds", entry_ids.join(","))
            .call()
            .map_err(|e| e.to_string())?;
        }
        // Add new items in order
        if !item_ids.is_empty() {
            self.post(&format!(
                "/Playlists/{}/Items",
                crate::encode_path_segment(playlist_id)
            ))
            .query("Ids", item_ids.join(","))
            .query("UserId", &self.user_id)
            .send_empty()
            .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    // ── Series / episodes / chapters ────────────────────────────────────────

    pub fn get_items_by_ids(&self, ids: &[String]) -> Result<Vec<EmbyItem>, String> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let joined = ids.join(",");
        let mut items = self.fetch_items(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)), &[
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

    pub fn get_ancestors(&self, item_id: &str) -> Result<Vec<EmbyItem>, String> {
        let resp: Value = self
            .get(&format!(
                "/Items/{}/Ancestors",
                crate::encode_path_segment(item_id)
            ))
            .query("Fields", "SortName")
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?;
        Ok(resp
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }
}
