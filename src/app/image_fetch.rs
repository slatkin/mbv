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
        if self.album_artist_fetch_inflight.contains(&album_id)
            || self.album_artist_cache.contains_key(&album_id)
        {
            return;
        }
        self.album_artist_fetch_inflight.insert(album_id.clone());
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
                self.album_artist_fetch_inflight.remove(&album_id);
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
            let artist = vote_album_artist(&items).unwrap_or_default();

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

/// Track rows per album that feed the majority vote (design D2): the
/// per-album request fetched `Limit=5`, so the batched level request samples
/// the same first ≤5 rows per bucket.
const ALBUM_ARTIST_VOTE_SAMPLE: usize = 5;

/// One track's artist candidate: `AlbumArtist`, falling back to the track's
/// first listed artist. `None` when neither field carries a usable name —
/// an `AlbumArtist` present but empty does *not* fall through to `Artists`,
/// matching the per-album vote this extracts.
fn track_artist_candidate(track: &serde_json::Value) -> Option<String> {
    let candidate = track["AlbumArtist"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            track["Artists"]
                .get(0)
                .and_then(|a| a.as_str())
                .map(str::to_string)
        })?;
    (!candidate.is_empty()).then_some(candidate)
}

/// Majority vote over an album's first `ALBUM_ARTIST_VOTE_SAMPLE` track rows
/// (design D2), extracted verbatim from the per-album fetch so one
/// outlier/mistagged track can't poison the whole album's displayed artist.
/// `tracks` must already be in request order
/// (`SortBy=ParentIndexNumber,IndexNumber`). Empty candidates are skipped and
/// the first-seen artist wins ties. `None` when no sampled track yields a
/// candidate.
fn vote_album_artist(tracks: &[serde_json::Value]) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for track in tracks.iter().take(ALBUM_ARTIST_VOTE_SAMPLE) {
        let Some(candidate) = track_artist_candidate(track) else {
            continue;
        };
        match counts.iter_mut().find(|(c, _)| c == &candidate) {
            Some(entry) => entry.1 += 1,
            None => counts.push((candidate, 1)),
        }
    }
    // `max_by_key` breaks ties by keeping the *last* max; we want the
    // *first*-seen artist to win ties, since it corresponds to the
    // earliest track in the sample (closest to "read the first track").
    counts
        .into_iter()
        .enumerate()
        .max_by_key(|(i, (_, n))| (*n, std::cmp::Reverse(*i)))
        .map(|(_, (c, _))| c)
}

/// True when `path` is `root` itself or lies underneath it — a
/// component-aligned prefix, so `/a/ab/1.flac` never attributes to `/a/a`.
#[allow(dead_code)] // applied by the level fetch (task 2.1, not wired yet)
fn path_within(path: &str, root: &str) -> bool {
    path.starts_with(root)
        && (path.len() == root.len()
            || root.ends_with('/')
            || path.as_bytes()[root.len()] == b'/')
}

/// Groups a level's Audio rows by the album they belong to (design D1/D3).
/// `albums` are the album items of the level listing. A track whose
/// `ParentId` is an album at the level attributes to it directly (the
/// measured 1:1 case); an orphan track (e.g. a multi-disc set's nested disc
/// subfolder) attributes to the album whose `Path` prefixes its `Path`, and
/// is dropped when none matches — the album then resolves through the
/// existing settle/fallback path. `tracks` must be in request order; each
/// bucket preserves that order.
#[allow(dead_code)] // applied by the level fetch (task 2.1, not wired yet)
fn bucket_tracks_by_album<'a>(
    tracks: &'a [serde_json::Value],
    albums: &[mbv_core::api::EmbyItem],
) -> std::collections::HashMap<String, Vec<&'a serde_json::Value>> {
    let album_ids: std::collections::HashSet<&str> =
        albums.iter().map(|a| a.id.as_str()).collect();
    let album_paths: Vec<(&str, &str)> = albums
        .iter()
        .filter(|a| !a.path.is_empty())
        .map(|a| (a.id.as_str(), a.path.as_str()))
        .collect();

    let mut buckets: std::collections::HashMap<String, Vec<&'a serde_json::Value>> =
        std::collections::HashMap::new();
    for track in tracks {
        let parent = track["ParentId"].as_str().unwrap_or("");
        if album_ids.contains(parent) {
            buckets.entry(parent.to_string()).or_default().push(track);
            continue;
        }
        // Orphan track (design D3): attribute by `Path` prefix against the
        // album listing, else drop. A multi-disc set surfaces as several
        // orphan tracks that all merge into their album's bucket.
        let track_path = track["Path"].as_str().unwrap_or("");
        if let Some((album_id, _)) = album_paths
            .iter()
            .find(|(_, album_path)| path_within(track_path, album_path))
        {
            buckets
                .entry((*album_id).to_string())
                .or_default()
                .push(track);
        }
    }
    buckets
}

#[cfg(test)]
mod album_artist_batch_tests {
    use super::*;
    use crate::app::tests::make_item;
    use rstest::rstest;

    fn track(parent: &str, path: &str, album_artist: Option<&str>, artists: &[&str]) -> serde_json::Value {
        let mut t = serde_json::json!({ "ParentId": parent, "Path": path });
        if let Some(a) = album_artist {
            t["AlbumArtist"] = serde_json::json!(a);
        }
        t["Artists"] = serde_json::json!(artists);
        t
    }

    fn album(id: &str, path: &str) -> mbv_core::api::EmbyItem {
        let mut a = make_item(id, "Folder");
        a.id = id.into();
        a.path = path.into();
        a.is_folder = true;
        a
    }

    #[rstest]
    #[case("majority_win", &["A", "B", "A", "A", "B"], Some("A"))]
    #[case("album_artist_present_but_empty_skipped", &["", ""], None)]
    #[case("empty_candidates_skipped", &["", "", "C", "C"], Some("C"))]
    #[case("first_seen_wins_tie", &["B", "A", "A", "B"], Some("B"))]
    #[case("sample_capped_at_five", &["A", "A", "A", "A", "A", "B", "B", "B"], Some("A"))]
    #[case("no_candidates", &["", ""], None)]
    fn vote_cases(
        #[case] _name: &str,
        #[case] album_artists: &[&str],
        #[case] expected: Option<&str>,
    ) {
        let tracks: Vec<serde_json::Value> = album_artists
            .iter()
            .map(|a| track("alb-1", "/m/a/1.flac", Some(a), &["Fallback"]))
            .collect();
        assert_eq!(
            vote_album_artist(&tracks).as_deref(),
            expected,
            "case {_name}"
        );
    }

    #[test]
    fn artists0_fallback_used_when_album_artist_absent() {
        let tracks = vec![
            track("alb-1", "/m/a/1.flac", None, &["X"]),
            track("alb-1", "/m/a/2.flac", None, &["X"]),
            track("alb-1", "/m/a/3.flac", None, &["Y"]),
        ];
        assert_eq!(vote_album_artist(&tracks).as_deref(), Some("X"));
    }

    #[test]
    fn empty_album_artist_does_not_fall_through_to_artists() {
        let tracks = vec![track("alb-1", "/m/a/1.flac", Some(""), &["X"])];
        assert_eq!(vote_album_artist(&tracks), None);
    }

    #[test]
    fn buckets_one_to_one_by_parent_id() {
        let tracks = vec![
            track("alb-1", "/m/a/1.flac", None, &["A"]),
            track("alb-2", "/m/b/1.flac", None, &["B"]),
            track("alb-1", "/m/a/2.flac", None, &["A"]),
        ];
        let albums = vec![album("alb-1", "/m/a"), album("alb-2", "/m/b")];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert_eq!(buckets.len(), 2);
        let a = &buckets["alb-1"];
        assert_eq!(a.len(), 2);
        assert_eq!(a[0]["Path"], "/m/a/1.flac");
        assert_eq!(a[1]["Path"], "/m/a/2.flac");
        assert_eq!(buckets["alb-2"].len(), 1);
    }

    #[test]
    fn multi_disc_orphan_bucket_attributed_via_path() {
        let tracks = vec![
            track("disc-1", "/m/a/Disc 1/1.flac", None, &["A"]),
            track("disc-1", "/m/a/Disc 1/2.flac", None, &["A"]),
            track("disc-2", "/m/a/Disc 2/1.flac", None, &["A"]),
        ];
        let albums = vec![album("alb-1", "/m/a")];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert_eq!(buckets.len(), 1);
        assert_eq!(buckets["alb-1"].len(), 3);
    }

    #[test]
    fn unmatched_orphan_bucket_dropped() {
        let tracks = vec![
            track("disc-9", "/m/other/x/1.flac", None, &["A"]),
            track("alb-1", "/m/a/1.flac", None, &["A"]),
        ];
        let albums = vec![album("alb-1", "/m/a")];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert_eq!(buckets.len(), 1);
        assert!(buckets.contains_key("alb-1"));
    }

    #[test]
    fn sibling_prefix_does_not_catch_unrelated_orphan() {
        // Component-aligned matching: `/m/ab` must not attribute to `/m/a`.
        let tracks = vec![track("disc-1", "/m/ab/1.flac", None, &["A"])];
        let albums = vec![album("alb-1", "/m/a")];
        assert!(bucket_tracks_by_album(&tracks, &albums).is_empty());
    }
}
