use super::*;

use super::level_artists::level_artists_from_items;

const MAX_LEVEL_ARTIST_WARMUPS: usize = 6;

impl App {
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
    pub(in crate::app) fn spawn_music_group_warmup(&mut self) {
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
                    crate::app::dispatch::library::browse::retain_grouped_music_items(
                        &mut items, true,
                    );
                    let _ = tx.send(LibEvent::MusicGroupWarmupListed {
                        generation,
                        groups: items,
                    });
                }
                // A failed listing fetch is silent: no level ids are known,
                // so there is nothing to mark `Failed`.
            }
        });
    }

    /// The libraries whose group levels warm up at Service Ready (design
    /// D5): every Emby music library while the configured music levels
    /// start with `"group"` — the same gate the group view itself uses.
    pub(in crate::app) fn music_group_warmup_library_ids(&self) -> Vec<String> {
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
    pub(in crate::app) fn enqueue_level_artist_warmup(&mut self, level_id: String) {
        if matches!(
            LevelFillState::action_for(self.album_artist_levels.get(&level_id)),
            LevelFillAction::NoWork
        ) || self.level_artist_warmups_in_flight.contains(&level_id)
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
    pub(in crate::app) fn drain_level_artist_warmups(&mut self) {
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
    pub(in crate::app) fn spawn_level_artist_fetch(
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
            let items: Vec<serde_json::Value> = crate::app::infra::feed_parse::tls_agent(None)
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
}
