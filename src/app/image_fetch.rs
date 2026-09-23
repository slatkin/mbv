const MAX_LEVEL_ARTIST_WARMUPS: usize = 6;

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
            if !series_id.is_empty() && !season_id.is_empty() {
                self.pending_series_season_expansions.insert(key);
                self.fetch_series_detail(series_id);
            }
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

    /// Startup warm-up (design D5 of
    /// `fix-music-artist-resolution-batching`): once the Emby Service is
    /// Ready, fetch each configured music library's group-level listing (its
    /// root children — the same listing the group view's first level shows)
    /// on the existing worker-thread + `lib_tx` pattern. The arrival handler
    /// queues one level fill per group-level child and drains that queue
    /// through the bounded warm-up scheduler; fills dedupe through the shared
    /// `LevelFillState::action_for` decision (design D4), so a
    /// warm-up racing candidate creation — or a repeated Ready — does no
    /// double work. Best-effort and silent: a failed group-listing fetch
    /// names no levels, so it is a no-op; a failed per-level fill marks that
    /// level `Failed` through the arrival handler, and browsing proceeds via
    /// the existing settle/fallback path. Never gates startup.
    /// Warm-up bound: one `Recursive=true` request per group child (about 15
    /// on the reference library), with at most `MAX_LEVEL_ARTIST_WARMUPS` (6)
    /// in flight; design D1/D5 measured about 3.8 MB peak and 3 seconds.
    /// Re-warm only happens when the Service is replaced (`clear_emby_memory`
    /// clears level state); reconnects dedupe through `LevelFillState`.
    pub(super) fn spawn_music_group_warmup(&mut self) {
        let library_ids = self.music_group_warmup_library_ids();
        if library_ids.is_empty() {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            return;
        };
        let tx = self.lib_tx.clone();
        let generation = self.emby_runtime.generation();
        std::thread::spawn(move || {
            for library_id in library_ids {
                // The established root-children call, verbatim from the
                // music library's first browse level (`spawn_browse`'s
                // default arm: no item types, SortName ascending).
                if let Ok((mut items, _total)) = client.get_items_sorted(
                    &library_id,
                    None,
                    false,
                    0,
                    PAGE_SIZE,
                    "SortName",
                    "Ascending",
                ) {
                    super::library_browse_actions::retain_grouped_music_items(&mut items, true);
                    let _ = tx.send(LibEvent::MusicGroupWarmupListed { generation, groups: items });
                }
                // A failed listing fetch is silent: no level ids are known,
                // so there is nothing to mark `Failed`.
            }
        });
    }

    /// The libraries whose group levels warm up at Service Ready (design
    /// D5): every Emby music library while the configured music levels
    /// start with `"group"` — the same gate the group view itself uses.
    pub(super) fn music_group_warmup_library_ids(&self) -> Vec<String> {
        if !self
            .music_levels
            .first()
            .map(|s| s == "group")
            .unwrap_or(false)
        {
            return Vec::new();
        }
        self.libs
            .iter()
            .enumerate()
            .filter(|(lib_idx, _)| self.is_grouped_music_library(*lib_idx))
            .map(|(_, lib)| lib.library.id.clone())
            .collect()
    }

    /// Enqueues one background album-artist request per music level for the
    /// bounded startup warm-up. Candidate requests use
    /// `spawn_level_artist_fetch` directly and share its level-state dedupe.
    pub(super) fn enqueue_level_artist_warmup(&mut self, level_id: String) {
        if matches!(
            LevelFillState::action_for(self.album_artist_levels.get(&level_id)),
            LevelFillAction::NoWork
        ) || self
            .level_artist_warmups_in_flight
            .contains(&level_id)
            || self
                .pending_level_artist_warmups
                .iter()
                .any(|pending| pending == &level_id)
        {
            return;
        }
        self.pending_level_artist_warmups.push_back(level_id);
        self.drain_level_artist_warmups();
    }

    /// Starts queued warm-up fills while the bounded fan-out has capacity.
    /// `spawn_level_artist_fetch` remains the sole gate for actual requests,
    /// so a candidate that wins a race with a pending warm-up removes the
    /// pending duplicate and the drain skips already-loading levels.
    pub(super) fn drain_level_artist_warmups(&mut self) {
        while self.level_artist_warmups_in_flight.len() < MAX_LEVEL_ARTIST_WARMUPS {
            let Some(level_id) = self.pending_level_artist_warmups.pop_front() else {
                break;
            };
            if matches!(
                LevelFillState::action_for(self.album_artist_levels.get(&level_id)),
                LevelFillAction::NoWork
            ) {
                continue;
            }
            self.spawn_level_artist_fetch(level_id.clone(), Vec::new());
            if matches!(
                self.album_artist_levels.get(&level_id),
                Some(LevelFillState::Loading { .. })
            ) {
                self.level_artist_warmups_in_flight.insert(level_id);
            }
        }
    }

    /// Spawns the one background album-artist request per music level
    /// (design D1 of `fix-music-artist-resolution-batching`): a single
    /// recursive Audio query over the whole level, bucketed per album and
    /// majority-voted per bucket, arriving as one
    /// `LibEvent::AlbumArtistLevelFetched` that bulk-fills the cache.
    /// Deduped on the level-fill state (design D4) through the single
    /// shared decision (`LevelFillState::action_for`). `albums` (the
    /// level's album items already in hand from the listing) drive
    /// orphan-`Path` attribution only (design D3) — bucketing itself is by
    /// track `ParentId` verbatim, so the one fill covers every album in
    /// the level regardless of which page was listed when it was
    /// requested.
    pub(super) fn spawn_level_artist_fetch(
        &mut self,
        level_id: String,
        albums: Vec<mbv_core::api::EmbyItem>,
    ) {
        // Each group child gets one `Recursive=true` request (about 15 on the
        // reference library); the queue permits at most
        // `MAX_LEVEL_ARTIST_WARMUPS` (6) in flight, about 3.8 MB peak and
        // 3 seconds per design D1/D5. Re-warm follows Service replacement
        // (`clear_emby_memory` clears level state); reconnects dedupe through
        // `LevelFillState`.
        // A candidate may start a level while it is still pending in the
        // warm-up queue. Remove that stale queue entry before the shared
        // action gate so a failed candidate request cannot be repeated as a
        // second warm-up request on the same level.
        self.pending_level_artist_warmups
            .retain(|pending| pending != &level_id);
        let orphan_upgrade = !albums.is_empty()
            && matches!(
                self.album_artist_levels.get(&level_id),
                Some(LevelFillState::Filled { orphan_risk: true })
            );
        if matches!(
            LevelFillState::action_for(self.album_artist_levels.get(&level_id)),
            LevelFillAction::NoWork
        ) && !orphan_upgrade
        {
            return;
        }
        self.album_artist_levels.insert(
            level_id.clone(),
            LevelFillState::Loading {
                orphan_risk: albums.is_empty(),
            },
        );
        let (server_url, token) = {
            let Some(client) = self.emby_client() else {
                // No client: mark `Failed` so the next candidate creation
                // retries instead of waiting on a fill that can never start.
                self.album_artist_levels
                    .insert(level_id, LevelFillState::Failed);
                return;
            };
            let c = client.lock().unwrap();
            (c.config.server_url.clone(), c.token.clone())
        };
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let url = format!(
                "{}/Items?ParentId={}&IncludeItemTypes=Audio&Recursive=true&Fields=AlbumArtist,Artists,ParentId,Path&SortBy=ParentIndexNumber,IndexNumber&SortOrder=Ascending&Limit=100000&api_key={}",
                server_url, level_id, token
            );
            let items: Vec<serde_json::Value> = super::feed_parse::tls_agent(None)
                .get(&url)
                .call()
                .ok()
                .and_then(|mut r| r.body_mut().read_json::<serde_json::Value>().ok())
                .and_then(|v| v["Items"].as_array().cloned())
                .unwrap_or_default();

            let artists = level_artists_from_items(&items, &albums);
            let _ = tx.send(LibEvent::AlbumArtistLevelFetched { level_id, artists });
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

    /// Pre-warm the neighbour album artwork the Grouped Music tree resolved
    /// from its latest completed paint (task 6.5, design D4). The typed
    /// targets are the tree's ordered stable album identities (an opaque
    /// `id\0index` target for duplicate rows); the shell derives the album ID
    /// and walks the shared `{id}:P` album-art chain the hero projection
    /// consumes — it re-resolves no tree cursor or window. Each fetch keeps
    /// the existing idle gate (`fetch_list_card_image_when_idle`), so rapid
    /// navigation suppresses the whole window.
    pub(in crate::app) fn prefetch_neighbour_album_art(&mut self, targets: &[String]) {
        if !self.images_enabled() {
            return;
        }
        for target in targets {
            // The tree's duplicate-row targets are opaque `id\0index` strings;
            // the album ID before the occurrence suffix is what the `{id}:P`
            // album-art chain keys on (the same split
            // `music_artist_detail::target_album_id` performs for the track
            // caches).
            let album_id = target.split('\0').next().unwrap_or(target);
            if album_id.is_empty() {
                continue;
            }
            self.fetch_list_card_image_when_idle(
                format!("{album_id}:P"),
                album_id.to_string(),
                String::new(),
                crate::app::render::components::widgets::MUSIC_ALBUM_IMAGE_TYPES,
            );
        }
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
fn vote_album_artist<'a>(
    tracks: impl IntoIterator<Item = &'a serde_json::Value>,
) -> Option<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for track in tracks.into_iter().take(ALBUM_ARTIST_VOTE_SAMPLE) {
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
fn path_within(path: &str, root: &str) -> bool {
    std::path::Path::new(path).starts_with(root)
}

/// Groups a level's Audio rows by the album they belong to (design D1/D3).
/// Every track with a `ParentId` is bucketed by it verbatim: bucket keys
/// are album ids by Emby semantics, and a key need not be an album in the
/// in-hand `albums` snapshot — level listings paginate, so a whole-level
/// fill must also cover albums listed on pages after the one in hand when
/// the fill was requested (they are cached under their own id and looked
/// up when those albums' candidates run). `albums` exist here only to
/// attribute orphan tracks: a bucket keyed by an id that is not an in-hand
/// album (e.g. a multi-disc set's nested disc subfolder) is re-attributed
/// to the in-hand album whose `Path` prefixes its tracks' `Path`s, longest
/// match winning; tracks that match nothing keep their original key — an
/// inert cache row that nothing looks up and a Service reset clears.
/// Tracks with no `ParentId` go through the same `Path`-prefix map and are
/// dropped when nothing matches — the album then resolves through the
/// existing settle/fallback path. `tracks` must be in request order; each
/// bucket preserves that order. Orphan re-attribution is processed in each
/// orphan bucket's first-appearance order in `tracks` (not HashMap order),
/// so merged buckets append in request order and `vote_album_artist`'s
/// first-seen tie-break is stable across runs.
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
    // First-appearance index of each `ParentId` bucket in the request-order
    // `tracks` slice, so orphan re-attribution below can run in request
    // order instead of `HashMap` enumeration order.
    let mut bucket_first_seen: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for (index, track) in tracks.iter().enumerate() {
        let parent = track["ParentId"].as_str().unwrap_or("");
        if !parent.is_empty() {
            buckets.entry(parent.to_string()).or_default().push(track);
            bucket_first_seen.entry(parent.to_string()).or_insert(index);
            continue;
        }
        // Track without a `ParentId`: attribute by `Path` prefix against
        // the in-hand album listing (design D3), else drop.
        attribute_by_path(track, &album_paths, &mut buckets);
    }

    // Reattribute buckets keyed by an id that is not an in-hand album (a
    // nested disc subfolder): their tracks belong to the in-hand album
    // whose `Path` prefixes theirs, longest match winning so a nested
    // album folder claims its own disc tracks instead of donating them to
    // the outer album. Design D3's drop rule predates f4e9c44e: a bucket key
    // is a track `ParentId` (1:1 with albums except rare nested disc folders)
    // and `albums` is one page, so an unmatched key may be an album listed on
    // a later page. Dropping it loses whole-level coverage and makes the
    // empty-album-list warm-up resolve nothing; retain it as an inert cache
    // row nothing looks up. Buckets are processed in first-appearance order
    // in `tracks` so merged orphan tracks preserve request order and the
    // vote's first-seen tie-break does not depend on `HashMap` order.
    let mut orphan_keys: Vec<(String, usize)> = bucket_first_seen
        .into_iter()
        .filter(|(key, _)| !album_ids.contains(key.as_str()))
        .collect();
    orphan_keys.sort_by_key(|(_, index)| *index);
    for (key, _) in orphan_keys {
        let bucket = buckets.remove(&key).unwrap_or_default();
        let mut unattributed = Vec::new();
        for track in bucket {
            if !attribute_by_path(track, &album_paths, &mut buckets) {
                unattributed.push(track);
            }
        }
        if !unattributed.is_empty() {
            buckets.insert(key, unattributed);
        }
    }
    buckets
}

/// Attributes `track` to the in-hand album whose `Path` prefixes the
/// track's `Path` (design D3), longest match winning so a nested album
/// Path (e.g. a Deluxe-edition folder one level deeper) claims its own
/// disc tracks instead of donating them to the outer album. Returns
/// `false` when nothing matches. A multi-disc set surfaces as several
/// orphan tracks that all merge into their album's bucket.
fn attribute_by_path<'a>(
    track: &'a serde_json::Value,
    album_paths: &[(&str, &str)],
    buckets: &mut std::collections::HashMap<String, Vec<&'a serde_json::Value>>,
) -> bool {
    let track_path = track["Path"].as_str().unwrap_or("");
    if let Some((album_id, _)) = album_paths
        .iter()
        .filter(|(_, album_path)| path_within(track_path, album_path))
        .max_by_key(|(_, album_path)| album_path.len())
    {
        buckets
            .entry((*album_id).to_string())
            .or_default()
            .push(track);
        true
    } else {
        false
    }
}

/// Pure parse+bucket+vote pipeline behind `spawn_level_artist_fetch`
/// (design D1–D3): buckets the level's Audio rows per album (by track
/// `ParentId` verbatim) and majority-votes each bucket's artist.
/// `albums` (the listing items already in hand) drive orphan-`Path`
/// attribution only, so the result covers every album in the level —
/// including ones listed on pages after the page in hand when the fill
/// was requested. Albums with no resolvable artist are omitted — their
/// slots stay free for the settle/fallback path. Deterministic: results
/// are ordered by album id. `tracks` must be in request order.
fn level_artists_from_items(
    tracks: &[serde_json::Value],
    albums: &[mbv_core::api::EmbyItem],
) -> Vec<(String, String)> {
    let mut artists: Vec<(String, String)> = bucket_tracks_by_album(tracks, albums)
        .into_iter()
        .filter_map(|(album_id, bucket)| {
            vote_album_artist(bucket.iter().copied()).map(|artist| (album_id, artist))
        })
        .collect();
    artists.sort_by(|a, b| a.0.cmp(&b.0));
    artists
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
    fn fill_requested_from_page_one_covers_later_page_albums() {
        // Page-starvation regression: the fill is requested with only the
        // page-1 album in hand; bucketing by `ParentId` verbatim still
        // yields the page-2 album's artist from its tracks, so the one
        // whole-level fill covers every album in the level no matter which
        // page was listed when it was requested.
        let tracks = vec![
            track("alb-1", "/m/a/1.flac", Some("Alpha"), &["Alpha"]),
            track("alb-2", "/m/b/1.flac", Some("Beta"), &["Beta"]),
        ];
        let albums = vec![album("alb-1", "/m/a")];
        let artists = level_artists_from_items(&tracks, &albums);
        assert_eq!(
            artists,
            vec![
                ("alb-1".to_string(), "Alpha".to_string()),
                ("alb-2".to_string(), "Beta".to_string()),
            ]
        );
    }

    #[test]
    fn unmatched_orphan_key_stays_as_inert_bucket() {
        // An orphan whose `Path` matches no in-hand album keeps its own
        // key: an inert cache row nothing looks up (a Service reset clears
        // it); the album itself resolves via the settle/fallback path.
        let tracks = vec![
            track("disc-9", "/m/other/x/1.flac", None, &["A"]),
            track("alb-1", "/m/a/1.flac", None, &["A"]),
        ];
        let albums = vec![album("alb-1", "/m/a")];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert_eq!(buckets["alb-1"].len(), 1);
        assert_eq!(buckets["disc-9"].len(), 1);
    }

    #[test]
    fn nested_album_path_claims_its_orphan_tracks() {
        // A nested album Path (Deluxe edition one level deeper) must win the
        // longest-prefix attribution over its outer album.
        let tracks = vec![
            track("disc-1", "/m/a/Deluxe/1.flac", None, &["A"]),
            track("disc-9", "/m/a/1.flac", None, &["A"]),
        ];
        let albums = vec![album("alb-outer", "/m/a"), album("alb-deluxe", "/m/a/Deluxe")];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert_eq!(buckets["alb-deluxe"].len(), 1);
        assert_eq!(buckets["alb-deluxe"][0]["Path"], "/m/a/Deluxe/1.flac");
        assert_eq!(buckets["alb-outer"].len(), 1);
        assert_eq!(buckets["alb-outer"][0]["Path"], "/m/a/1.flac");
    }

    #[test]
    fn level_pipeline_buckets_votes_and_orders_deterministically() {
        // Mock JSON shaped like the level request's `Items` array: two
        // 1:1 album buckets plus a multi-disc orphan bucket, exercising the
        // full parse+bucket+vote pipeline without a live server.
        let tracks = vec![
            track("alb-b", "/m/b/2.flac", Some("Beta"), &["Beta"]),
            track("disc-1", "/m/a/Disc 1/1.flac", Some("Alpha"), &["Alpha"]),
            track("disc-2", "/m/a/Disc 2/1.flac", Some("Wrong"), &["Wrong"]),
            track("disc-2", "/m/a/Disc 2/2.flac", Some("Alpha"), &["Alpha"]),
            track("alb-b", "/m/b/1.flac", Some("Alpha"), &["Beta"]),
            track("alb-c", "/m/c/1.flac", Some(""), &[""]),
        ];
        let albums = vec![album("alb-a", "/m/a"), album("alb-b", "/m/b")];
        let artists = level_artists_from_items(&tracks, &albums);
        // `alb-c`'s bucket yields no candidate and is omitted entirely;
        // `alb-a`'s orphan bucket majority-votes to Alpha despite the outlier.
        assert_eq!(
            artists,
            vec![
                ("alb-a".to_string(), "Alpha".to_string()),
                ("alb-b".to_string(), "Beta".to_string()),
            ]
        );
    }

    #[test]
    fn level_pipeline_empty_tracks_yield_empty_artists() {
        // HTTP failure surfaces as an empty `Items` array upstream; the
        // pipeline must return no artists so the level marks `Failed`.
        let albums = vec![album("alb-a", "/m/a")];
        assert!(level_artists_from_items(&[], &albums).is_empty());
    }

    #[test]
    fn orphan_merge_order_follows_request_order_so_tie_break_is_stable() {
        // Two disc subfolders re-attributing to one album with a tied vote:
        // the winner must be the artist first seen in request order, no
        // matter how `HashMap` enumerates the orphan bucket keys. Two track
        // lists with different orphan key names (same first-appearance
        // ordering) must yield identical merged-bucket order and winner.
        let albums = vec![album("alb-1", "/m/a")];
        for (k1, k2) in [("disc-1", "disc-2"), ("cd-b", "cd-a")] {
            let tracks = vec![
                track(k1, "/m/a/Disc 1/1.flac", Some("Zed"), &["Zed"]),
                track(k2, "/m/a/Disc 2/1.flac", Some("Abc"), &["Abc"]),
            ];
            let buckets = bucket_tracks_by_album(&tracks, &albums);
            assert_eq!(buckets["alb-1"].len(), 2, "keys {k1}/{k2}");
            assert_eq!(buckets["alb-1"][0]["Path"], "/m/a/Disc 1/1.flac");
            assert_eq!(buckets["alb-1"][1]["Path"], "/m/a/Disc 2/1.flac");
            let artists = level_artists_from_items(&tracks, &albums);
            assert_eq!(
                artists,
                vec![("alb-1".to_string(), "Zed".to_string())],
                "keys {k1}/{k2}"
            );
        }
    }

    #[test]
    fn sibling_prefix_does_not_catch_unrelated_orphan() {
        // Component-aligned matching: `/m/ab` must not attribute to `/m/a`.
        // The unmatched orphan keeps its own (inert) key instead.
        let tracks = vec![track("disc-1", "/m/ab/1.flac", None, &["A"])];
        let albums = vec![album("alb-1", "/m/a")];
        let buckets = bucket_tracks_by_album(&tracks, &albums);
        assert!(!buckets.contains_key("alb-1"));
        assert_eq!(buckets["disc-1"].len(), 1);
    }
}
