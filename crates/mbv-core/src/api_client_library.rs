impl EmbyClient {
    fn fetch_items(&self, path: &str, queries: &[(&str, &str)]) -> Result<Vec<EmbyItem>, String> {
        let mut req = self.get(path);
        for (k, v) in queries {
            req = req.query(k, v);
        }
        let resp: Value = req
            .call()
            .map_err(|e| e.to_string())?
            .body_mut()
            .read_json()
            .map_err(|e| e.to_string())?;
        Ok(resp["Items"]
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_views(&self) -> Result<Vec<EmbyItem>, String> {
        self.get_views_classified().map_err(|e| e.to_string())
    }

    pub fn get_views_classified(
        &self,
    ) -> Result<Vec<EmbyItem>, crate::service_runtime::EmbyFailure> {
        let vfolders: Value = self
            .get("/Library/VirtualFolders")
            .call()
            .map_err(|e| Self::service_failure("Emby views request failed", e))?
            .body_mut()
            .read_json()
            .map_err(|e| {
                crate::service_runtime::EmbyFailure::unavailable(format!(
                    "Emby views response failed: {e}"
                ))
            })?;

        let user_views: Value = self
            .get(&format!(
                "/Users/{}/Views",
                crate::encode_path_segment(&self.user_id)
            ))
            .call()
            .map_err(|e| Self::service_failure("Emby user views request failed", e))?
            .body_mut()
            .read_json()
            .map_err(|e| {
                crate::service_runtime::EmbyFailure::unavailable(format!(
                    "Emby user views response failed: {e}"
                ))
            })?;

        let mut items: Vec<EmbyItem> = Vec::new();
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

        if let Some(arr) = vfolders.as_array() {
            for f in arr {
                let id = f["ItemId"].as_str().unwrap_or("").to_string();
                let name = f["Name"].as_str().unwrap_or("").to_string();
                let ctype = f["CollectionType"].as_str().unwrap_or("").to_string();
                seen.insert(id.clone());
                items.push(EmbyItem::folder(id, name, ctype));
            }
        }

        if let Some(arr) = user_views["Items"].as_array() {
            for raw in arr {
                let id = raw["Id"].as_str().unwrap_or("").to_string();
                if !seen.contains(&id) {
                    items.push(parse_item(raw));
                }
            }
        }

        Ok(items)
    }

    pub fn get_user_views(&self) -> Result<Vec<EmbyItem>, String> {
        self.fetch_items(
            &format!("/Users/{}/Views", crate::encode_path_segment(&self.user_id)),
            &[],
        )
    }

    pub fn get_items_sorted(
        &self,
        parent_id: &str,
        item_types: Option<&str>,
        unplayed_only: bool,
        start_index: usize,
        limit: usize,
        sort_by: &str,
        sort_order: &str,
    ) -> Result<(Vec<EmbyItem>, usize), String> {
        self.get_items_sorted_ranged(
            parent_id,
            item_types,
            unplayed_only,
            start_index,
            limit,
            sort_by,
            sort_order,
            None,
            None,
        )
    }

    /// Like `get_items_sorted`, but additionally scopes the fetch to a
    /// SortName range via Emby's `NameStartsWithOrGreater` /
    /// `NameLessThan` filters (`name_ge`/`name_lt`, either or both
    /// optional) -- used by the letter-range pills so only the
    /// selected range is fetched from the server. Verified empirically
    /// against a live Emby server (2026-07-22): these filters key off
    /// SortName, not the raw display Name, matching the app's own
    /// letter-bucket header grouping (e.g. "The Harder They Come", SortName
    /// "Harder They Come", is included in an H-I range fetch and excluded
    /// from a T-U range fetch).
    #[allow(clippy::too_many_arguments)]
    pub fn get_items_sorted_ranged(
        &self,
        parent_id: &str,
        item_types: Option<&str>,
        unplayed_only: bool,
        start_index: usize,
        limit: usize,
        sort_by: &str,
        sort_order: &str,
        name_ge: Option<&str>,
        name_lt: Option<&str>,
    ) -> Result<(Vec<EmbyItem>, usize), String> {
        let mut req = self.get(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)))
            .query("ParentId", parent_id)
            .query("SortBy", sort_by)
            .query("SortOrder", sort_order)
            .query("StartIndex", start_index.to_string())
            .query("Limit", limit.to_string())
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear,EndDate,Overview,PremiereDate,DateCreated,ChildCount,RecursiveItemCount,Container,People,MediaStreams,Genres")
            .query("EnableUserData", "true");
        if let Some(types) = item_types {
            req = req
                .query("IncludeItemTypes", types)
                .query("Recursive", "true");
        }
        if unplayed_only {
            req = req.query("Filters", "IsUnplayed");
        }
        if let Some(v) = name_ge {
            req = req.query("NameStartsWithOrGreater", v);
        }
        if let Some(v) = name_lt {
            req = req.query("NameLessThan", v);
        }
        let call_started = std::time::Instant::now();
        let resp_result = req.call();
        let call_ms = call_started.elapsed().as_millis();
        let mut resp = resp_result.map_err(|e| {
            log::warn!(
                target: "api",
                "get_items_sorted: parent={parent_id} types={item_types:?} err after {call_ms}ms: {e}"
            );
            e.to_string()
        })?;
        let parse_started = std::time::Instant::now();
        let resp: Value = resp.body_mut().read_json().map_err(|e| e.to_string())?;
        let total = resp["TotalRecordCount"].as_u64().unwrap_or(0) as usize;
        let items: Vec<EmbyItem> = resp["Items"]
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default();
        let parse_ms = parse_started.elapsed().as_millis();
        log::info!(
            target: "api",
            "get_items_sorted: parent={parent_id} types={item_types:?} start={start_index} limit={limit} name_ge={name_ge:?} name_lt={name_lt:?} -> {} items (total={total}) http={call_ms}ms parse={parse_ms}ms",
            items.len()
        );
        Ok((items, total))
    }

    pub fn search_items(&self, term: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
        let limit = limit.to_string();
        self.fetch_items(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)), &[
            ("SearchTerm",  term),
            ("Recursive",   "true"),
            ("Limit",       &limit),
            ("Fields",      "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,ProductionYear"),
        ])
    }

    pub fn get_continue_watching(&self, limit: usize) -> Result<Vec<EmbyItem>, String> {
        let limit = limit.to_string();
        self.fetch_items(&format!("/Users/{}/Items/Resume", crate::encode_path_segment(&self.user_id)), &[
            ("UserId",     &self.user_id),
            ("Limit",      &limit),
            ("Fields",     "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,Overview,PremiereDate"),
            ("MediaTypes", "Video"),
        ])
    }

    pub fn get_latest(&self, parent_id: &str, limit: usize) -> Result<Vec<EmbyItem>, String> {
        let resp: Value = self.get(&format!("/Users/{}/Items/Latest", crate::encode_path_segment(&self.user_id)))
            .query("ParentId", parent_id)
            .query("Limit", limit.to_string())
            .query("GroupItems", "true")
            .query("Fields", "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists,AlbumId,Overview,PremiereDate")
            .call().map_err(|e| e.to_string())?
            .body_mut().read_json().map_err(|e| e.to_string())?;
        Ok(resp
            .as_array()
            .map(|arr| arr.iter().map(parse_item).collect())
            .unwrap_or_default())
    }

    pub fn get_latest_episodes(
        &self,
        parent_id: &str,
        limit: usize,
    ) -> Result<Vec<EmbyItem>, String> {
        let limit = limit.to_string();
        self.fetch_items(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)), &[
            ("ParentId",          parent_id),
            ("Limit",             &limit),
            ("IncludeItemTypes",  "Episode"),
            ("Recursive",         "true"),
            ("SortBy",            "DateCreated"),
            ("SortOrder",         "Descending"),
            ("IsPlayed",          "false"),
            ("Fields",            "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,Overview,PremiereDate"),
        ])
    }

    pub fn get_all_playable_recursive(&self, parent_id: &str) -> Result<Vec<EmbyItem>, String> {
        self.fetch_items(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)), &[
            ("ParentId",         parent_id),
            ("IncludeItemTypes", "Episode,Movie,Video,Audio"),
            ("Recursive",        "true"),
            ("SortBy",           "SortName"),
            ("SortOrder",        "Ascending"),
            ("Limit",            "2000"),
            ("Fields",           "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])
    }

    pub fn get_direct_playable(&self, parent_id: &str) -> Result<Vec<EmbyItem>, String> {
        self.fetch_items(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)), &[
            ("ParentId",         parent_id),
            ("IncludeItemTypes", "Episode,Movie,Video,Audio"),
            ("SortBy",           "SortName"),
            ("SortOrder",        "Ascending"),
            ("Limit",            "2000"),
            ("Fields",           "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])
    }

    pub fn get_all_videos_recursive(&self, parent_id: &str) -> Result<Vec<EmbyItem>, String> {
        self.fetch_items(&format!("/Users/{}/Items", crate::encode_path_segment(&self.user_id)), &[
            ("ParentId",         parent_id),
            ("IncludeItemTypes", "Episode,Movie,Video"),
            ("Recursive",        "true"),
            ("SortBy",           "SortName"),
            ("SortOrder",        "Ascending"),
            ("Limit",            "2000"),
            ("Fields",           "UserData,RunTimeTicks,MediaType,SeriesId,SeriesName,SortName,ParentIndexNumber,IndexNumber,Path,AlbumArtist,Artists"),
        ])
    }

    // ── Library actions ──────────────────────────────────────────────────────

    pub fn mark_played(&self, item_id: &str) -> Result<(), String> {
        self.post(&format!(
            "/Users/{}/PlayedItems/{}",
            crate::encode_path_segment(&self.user_id),
            crate::encode_path_segment(item_id)
        ))
        .send_empty()
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn mark_unplayed(&self, item_id: &str) -> Result<(), String> {
        self.delete(&format!(
            "/Users/{}/PlayedItems/{}",
            crate::encode_path_segment(&self.user_id),
            crate::encode_path_segment(item_id)
        ))
        .call()
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn hide_from_resume(&self, item_id: &str) -> Result<(), String> {
        self.post(&format!(
            "/Users/{}/Items/{}/HideFromResume",
            crate::encode_path_segment(&self.user_id),
            crate::encode_path_segment(item_id)
        ))
        .query("Hide", "true")
        .send_empty()
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn post_library_refresh(&self, library_id: &str) -> Result<(), String> {
        self.post(&format!(
            "/Items/{}/Refresh",
            crate::encode_path_segment(library_id)
        ))
        .query("Recursive", "true")
        .query("ImageRefreshMode", "Default")
        .query("MetadataRefreshMode", "Default")
        .query("ReplaceAllImages", "false")
        .query("ReplaceAllMetadata", "false")
        .send_empty()
        .map_err(|e| e.to_string())?;
        Ok(())
    }

    // ── Playback reporting ───────────────────────────────────────────────────

    pub fn ws_url(&self) -> String {
        let base = self
            .config
            .server_url
            .replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1);
        format!(
            "{}/embywebsocket?api_key={}&deviceId={}",
            base, self.token, self.device_id
        )
    }
}
