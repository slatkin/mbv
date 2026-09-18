impl App {
    /// Proactively fetches the full track list for `album_id` so the view's
    /// inline album detail pane (#145) can render it without the user
    /// drilling in first. A simple one-shot fetch (no throttle queue) —
    /// only one album is ever highlighted at a time, so there is no fan-out
    /// to bound.
    pub(super) fn fetch_album_tracks(&mut self, album_id: String) {
        if self.album_tracks_loading.contains(&album_id)
            || self.album_tracks_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_tracks_loading.insert(album_id.clone());
        let Some(client) = self.emby_snapshot() else {
            self.album_tracks_loading.remove(&album_id);
            return;
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let tracks = client
                .get_items_sorted(
                    &album_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "ParentIndexNumber,IndexNumber",
                    "Ascending",
                )
                .map(|(items, _total)| items)
                .unwrap_or_default();
            let _ = tx.send(LibEvent::AlbumTracksFetched { album_id, tracks });
        });
    }

    /// Proactively fetches TV series detail (seasons + episodes) so the
    /// Inline series detail pane can render without the user
    /// drilling in first.
    pub(super) fn fetch_series_detail(&mut self, series_id: String) {
        if series_id.is_empty() {
            return;
        }
        if self.series_detail_loading.contains(&series_id)
            || self.series_detail_cache.contains_key(&series_id)
        {
            return;
        }
        self.series_detail_loading.insert(series_id.clone());
        let Some(client) = self.emby_snapshot() else {
            self.series_detail_loading.remove(&series_id);
            return;
        };
        let tx = self.lib_tx.clone();
        let sid = series_id.clone();
        std::thread::spawn(move || {
            let (seasons, episodes) = client
                .get_items_sorted(&sid, None, false, 0, PAGE_SIZE, "SortName", "Ascending")
                .map(|(items, _total)| items)
                .map(|seasons| (seasons, std::collections::HashMap::new()))
                .unwrap_or_default();
            let _ = tx.send(LibEvent::SeriesDetailFetched {
                series_id: sid,
                seasons,
                episodes,
            });
        });
    }

    /// Fetches one season only after the complete ordered Series detail is in
    /// the cache. The detail event handler calls this for every uncached pill.
    pub(super) fn fetch_series_season_episodes(&mut self, series_id: String, season_id: String) {
        let key = (series_id.clone(), season_id.clone());
        let Some(detail) = self.series_detail_cache.get(&series_id) else {
            return;
        };
        if !detail.seasons.iter().any(|season| season.id == season_id)
            || detail.episodes.contains_key(&season_id)
            || self.series_season_loading.contains(&key)
        {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        self.series_detail_loading.insert(series_id.clone());
        self.series_season_loading.insert(key);
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let episodes = client
                .get_items_sorted(
                    &season_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "IndexNumber",
                    "Ascending",
                )
                .map(|(items, _)| items)
                .unwrap_or_default();
            let _ = tx.send(LibEvent::SeriesSeasonEpisodesFetched {
                series_id,
                season_id,
                episodes,
            });
        });
    }

    pub(super) fn fetch_album_artist(&mut self, album_id: String) {
        if self.album_artist_loading.contains(&album_id)
            || self.album_artist_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_artist_loading.insert(album_id.clone());
        if self.album_artist_fetches_active >= MAX_ALBUM_ARTIST_FETCHES {
            // Queue instead of dropping: a slot will pick it up on completion.
            self.pending_album_artist_fetches.push_back(album_id);
            return;
        }
        self.spawn_album_artist_fetch(album_id);
    }

    /// Spawn queued album-artist fetches until the in-flight limit is reached.
    /// Called whenever an in-flight fetch completes and frees a slot (see the
    /// `LibEvent::AlbumArtistFetched` handler in `actions.rs`).
    pub(super) fn drain_album_artist_fetches(&mut self) {
        while self.album_artist_fetches_active < MAX_ALBUM_ARTIST_FETCHES {
            let Some(album_id) = self.pending_album_artist_fetches.pop_front() else {
                break;
            };
            self.spawn_album_artist_fetch(album_id);
        }
    }

    fn spawn_album_artist_fetch(&mut self, album_id: String) {
        self.album_artist_fetches_active += 1;
        let (server_url, token) = {
            let Some(client) = self.emby_client() else {
                self.album_artist_loading.remove(&album_id);
                self.album_artist_fetches_active =
                    self.album_artist_fetches_active.saturating_sub(1);
                return;
            };
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let url = format!(
                "{}/Items?ParentId={}&IncludeItemTypes=Audio&Limit=5&SortBy=ParentIndexNumber,IndexNumber&SortOrder=Ascending&Fields=AlbumArtist,Artists&api_key={}",
                server_url, album_id, token
            );
            let items: Vec<serde_json::Value> = super::feed_parse::tls_agent(None)
                .get(&url)
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().read_json::<serde_json::Value>().ok())
                .and_then(|v| v["Items"].as_array().cloned())
                .unwrap_or_default();

            // Majority vote over up to 5 tracks' AlbumArtist (falling back to
            // Artists[0] per-track), so one outlier/mistagged track can't poison
            // the whole album's displayed artist.
            let mut counts: Vec<(String, usize)> = Vec::new();
            for item in &items {
                let candidate = item["AlbumArtist"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| {
                        item["Artists"]
                            .get(0)
                            .and_then(|a| a.as_str())
                            .map(|s| s.to_string())
                    })
                    .unwrap_or_default();
                if candidate.is_empty() {
                    continue;
                }
                match counts.iter_mut().find(|(c, _)| c == &candidate) {
                    Some(entry) => entry.1 += 1,
                    None => counts.push((candidate, 1)),
                }
            }
            // `max_by_key` breaks ties by keeping the *last* max; we want the
            // *first*-seen artist to win ties, since it corresponds to the
            // earliest track in the sample (closest to "read the first track").
            let artist = counts
                .into_iter()
                .enumerate()
                .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
                .map(|(_, (c, _))| c)
                .unwrap_or_default();

            let _ = tx.send(LibEvent::AlbumArtistFetched { album_id, artist });
        });
    }

    pub(super) fn fetch_card_image(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        self.queue_card_image_fetch(cache_key, item_id, series_id, types);
    }

    fn queue_card_image_fetch(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        if self.card_image_loading.contains(&cache_key)
            || self.card_image_states.contains_key(&cache_key)
        {
            return;
        }
        // Test-only instrumentation: counts every reservation that proceeds
        // past the dedup guard above, so a broken guard is visible even when
        // the fixture has no Emby client (`spawn_image_fetch` balances
        // `image_fetches_active` back to its prior value synchronously in
        // that case, hiding a redundant reservation from the other counters).
        // `project_hero_image` calls `fetch_card_image` unconditionally on
        // every sync pass, so counting entry into this function (rather than
        // past this guard) would also increment on every legitimate repaint.
        #[cfg(test)]
        {
            self.card_image_fetch_calls += 1;
        }
        // Reserve the key immediately so duplicate (and queued) requests dedupe.
        self.card_image_loading.insert(cache_key.clone());
        let req = ImageFetchReq {
            cache_key,
            item_id,
            series_id,
            types: types.iter().map(|s| s.to_string()).collect(),
            source: ImageSource::Emby,
        };
        if self.image_fetches_active >= MAX_IMAGE_FETCHES {
            // Queue instead of dropping: a slot will pick it up on completion.
            self.pending_image_fetches.push_back(req);
            return;
        }
        self.spawn_image_fetch(req);
    }

    /// Triggers the plain (uncropped) Audiobookshelf cover fetch for
    /// `item_id` — the queue card's entry. The Library hero uses the
    /// hero-scoped key instead (`App::audiobookshelf_cover_key`), because it
    /// re-encodes its entry from a cover-fit crop.
    pub(super) fn fetch_audiobookshelf_cover(&mut self, server_url: String, item_id: String) {
        let cache_key =
            audiobookshelf_cover_cache_key(&server_url, &item_id, self.current_protocol_suffix());
        self.fetch_audiobookshelf_image(cache_key, server_url, item_id);
    }

    /// Book-shaped sibling of `fetch_audiobookshelf_cover` using the isolated
    /// `:bookcover:` cache key.
    pub(super) fn fetch_audiobookshelf_book_cover(&mut self, server_url: String, item_id: String) {
        let cache_key = audiobookshelf_book_cover_cache_key(
            &server_url,
            &item_id,
            self.current_protocol_suffix(),
        );
        self.fetch_audiobookshelf_image(cache_key, server_url, item_id);
    }

    fn fetch_audiobookshelf_image(
        &mut self,
        cache_key: String,
        server_url: String,
        item_id: String,
    ) {
        if !self.image_protocol_enabled {
            return;
        }
        if self.card_image_loading.contains(&cache_key)
            || self.card_image_states.contains_key(&cache_key)
        {
            return;
        }
        let Some(api_key) =
            mbv_core::config::load_service_secret(mbv_core::config::ServiceKind::Audiobookshelf)
        else {
            return;
        };
        let req = ImageFetchReq {
            cache_key: cache_key.clone(),
            item_id,
            series_id: String::new(),
            types: Vec::new(),
            source: ImageSource::Audiobookshelf {
                server_url,
                api_key,
            },
        };
        self.card_image_loading.insert(cache_key);
        if self.image_fetches_active >= MAX_IMAGE_FETCHES {
            self.pending_image_fetches.push_back(req);
        } else {
            self.spawn_image_fetch(req);
        }
    }

    pub(in crate::app) fn list_image_fetches_allowed(&self) -> bool {
        self.last_nav_at.elapsed() >= NAV_IMAGE_FETCH_IDLE_DELAY
    }

    pub(super) fn mark_library_navigation(&mut self, at: Instant) {
        self.last_library_nav_at = at;
    }

    pub(super) fn fetch_list_card_image_when_idle(
        &mut self,
        cache_key: String,
        item_id: String,
        series_id: String,
        types: &[&str],
    ) {
        if !self.list_image_fetches_allowed() {
            return;
        }
        self.fetch_card_image(cache_key, item_id, series_id, types);
    }
}
