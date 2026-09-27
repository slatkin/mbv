use crate::app::state::app_struct::{LevelFillAction, LevelFillState};
use crate::app::ui_model::music_grouping::{
    build_grouped_album_catalog, MusicGroupCandidate, MusicGroupingState,
};
use crate::app::App;
use mbv_emby_model::EmbyItem;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

const SETTLE_WINDOW: Duration = Duration::from_secs(3);

impl App {
    /// Starts (or supersedes) the grouping candidate for the current music
    /// album level when its items change: on load, refresh, or page append.
    /// Albums already carrying an artist identity (item tag or a cached
    /// resolved artist) are terminal up front; the rest wait on one level
    /// fill (design D4), deduped on the level-fill state through the single
    /// shared decision (`LevelFillState::action_for`). A prior settled
    /// catalog stays visible while the replacement resolves.
    pub(in crate::app) fn start_or_supersede_music_grouping(&mut self, lib_idx: usize) {
        if !self.is_music_group_view(lib_idx) {
            return;
        }
        let (needs_fetch, level_request): (bool, Option<(String, Vec<EmbyItem>)>) = {
            let lib = &mut self.libs[lib_idx];
            let Some(level) = lib.nav_stack.last_mut() else {
                return;
            };
            let state = level
                .music_grouping
                .get_or_insert_with(MusicGroupingState::new);
            state.revision = state.revision.saturating_add(1);
            let mut candidate = MusicGroupCandidate {
                revision: state.revision,
                parent_id: level.parent_id.clone(),
                unresolved: HashSet::new(),
                resolved: HashMap::new(),
                created_at: Instant::now(),
            };
            for item in &level.items {
                if !item.artist.is_empty() {
                    continue;
                }
                match self.album_artist_cache.get(&item.id) {
                    Some(cached) if !cached.is_empty() => {
                        candidate.resolved.insert(item.id.clone(), cached.clone());
                    }
                    Some(_) => {}
                    None => {
                        candidate.unresolved.insert(item.id.clone());
                    }
                }
            }
            // One level fill serves every unresolved album (design D4),
            // deduped on the level-fill state through the single shared
            // decision (`LevelFillState::action_for`): `Loading`/`Filled`
            // levels do no work; a fresh or `Failed` level (re)starts the
            // fill. A candidate-driven `Filled` level with still-unresolved
            // albums is terminal — its fill had the level's album paths —
            // while a warm-up `Filled` level gets one path-aware upgrade.
            let mut level_request = None;
            if !candidate.unresolved.is_empty() {
                let level_state = self.album_artist_levels.get(&candidate.parent_id);
                match LevelFillState::action_for(level_state) {
                    LevelFillAction::Request => {
                        level_request = Some((candidate.parent_id.clone(), level.items.clone()));
                    }
                    LevelFillAction::NoWork => match level_state {
                        // Warm-up had no album paths, so one browse-triggered
                        // upgrade is needed to attribute nested-disc orphan
                        // buckets. Keep the candidate unresolved while that
                        // fill runs; its arrival can resolve every album.
                        Some(LevelFillState::Filled { orphan_risk: true }) => {
                            level_request =
                                Some((candidate.parent_id.clone(), level.items.clone()));
                        }
                        // A candidate-driven fill already had the level's
                        // album paths and is terminal, preserving the
                        // existing immediate fallback behavior.
                        Some(LevelFillState::Filled { orphan_risk: false }) => {
                            candidate.unresolved.clear();
                        }
                        // Loading is shared with another candidate/warm-up;
                        // Failed is unreachable under NoWork but remains
                        // explicit for exhaustive state handling.
                        Some(LevelFillState::Loading { .. } | LevelFillState::Failed) | None => {}
                    },
                }
            }
            let needs_fetch = !candidate.unresolved.is_empty();
            state.candidate = Some(candidate);
            (needs_fetch, level_request)
        };
        if !needs_fetch {
            self.commit_music_grouping_candidate(lib_idx);
            return;
        }
        if let Some((level_id, albums)) = level_request {
            self.spawn_level_artist_fetch(level_id, albums);
        }
    }

    /// Advances the current music album level's candidate with an arriving
    /// artist result. Only the candidate whose revision still matches the
    /// active browse level may commit; superseded candidates are discarded.
    pub(in crate::app) fn advance_music_grouping_candidates(
        &mut self,
        album_id: &str,
        artist: &str,
    ) {
        for lib_idx in 0..self.libs.len() {
            if !self.is_music_group_view(lib_idx) {
                continue;
            }
            let to_commit = {
                let lib = &mut self.libs[lib_idx];
                let Some(level) = lib.nav_stack.last_mut() else {
                    continue;
                };
                let Some(state) = level.music_grouping.as_mut() else {
                    continue;
                };
                let Some(candidate) = state.candidate.as_mut() else {
                    continue;
                };
                if !candidate.unresolved.remove(album_id) {
                    continue;
                }
                candidate
                    .resolved
                    .insert(album_id.to_string(), artist.to_string());
                if candidate.created_at.elapsed() >= SETTLE_WINDOW {
                    candidate.unresolved.clear();
                }
                candidate.unresolved.is_empty()
            };
            if to_commit {
                self.commit_music_grouping_candidate(lib_idx);
            }
        }
    }

    /// Force-settles candidates whose lookup window expired, including the
    /// case where every lookup failed before producing an event.
    pub(in crate::app) fn expire_music_grouping_candidates(&mut self) {
        let mut expired = Vec::new();
        for lib_idx in 0..self.libs.len() {
            if !self.is_music_group_view(lib_idx) {
                continue;
            }
            let should_commit = {
                let lib = &mut self.libs[lib_idx];
                let Some(level) = lib.nav_stack.last_mut() else {
                    continue;
                };
                let Some(state) = level.music_grouping.as_mut() else {
                    continue;
                };
                let Some(candidate) = state.candidate.as_mut() else {
                    continue;
                };
                if candidate.created_at.elapsed() < SETTLE_WINDOW {
                    false
                } else {
                    candidate.unresolved.clear();
                    true
                }
            };
            if should_commit {
                expired.push(lib_idx);
            }
        }
        for lib_idx in expired {
            self.commit_music_grouping_candidate(lib_idx);
        }
    }

    /// Commits the candidate's settled catalog when its source revision still
    /// matches the active browse level, anchoring the cursor by album identity
    /// so a replacement keeps the selection in view.
    fn commit_music_grouping_candidate(&mut self, lib_idx: usize) {
        let lib = &mut self.libs[lib_idx];
        let Some(level) = lib.nav_stack.last_mut() else {
            return;
        };
        let resting_cursor = level.resting().cursor();
        let Some(state) = level.music_grouping.as_mut() else {
            return;
        };
        let Some(candidate) = state.candidate.take() else {
            return;
        };
        if candidate.revision != state.revision || candidate.parent_id != level.parent_id {
            return;
        }
        let anchor_album_id = state
            .settled
            .is_some()
            .then(|| level.items.get(resting_cursor).map(|item| item.id.clone()));
        let mut catalog = build_grouped_album_catalog(&level.items, &candidate.resolved);
        catalog.revision = candidate.revision;
        catalog.parent_id = candidate.parent_id;
        state.settled = Some(catalog);
        let catalog = state.settled.as_ref().expect("catalog just inserted");
        let anchored_cursor = match anchor_album_id {
            Some(Some(id)) => catalog
                .id_to_entry
                .get(&id)
                .map(|&pos| catalog.entries[pos].album_index)
                .or_else(|| catalog.entries.first().map(|first| first.album_index)),
            _ => catalog.entries.first().map(|first| first.album_index),
        };
        if let Some(cursor) = anchored_cursor {
            level.set_resting_cursor(cursor);
        }
    }
}
