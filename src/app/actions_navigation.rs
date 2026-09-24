use super::{App, BrowseLevel, PendingQueueAction, ReplacementExecutor, RoutedReplacementPrep};
use crate::app::infra::ui_util::{is_playable, natural_sort_key, sort_audio_tracks};
use crate::app::state::types::browse::BrowseResting;
use mbv_core::api::EmbyItem;

use super::notify_actions::ToastSeverity;

impl App {
    /// Ctrl+P activation tail for an explicitly supplied library item (task
    /// 5.3d, Emby browser effect decoupling): folder items play the folder
    /// through the collection queue source and save the queue, non-folder
    /// items activate via `select_item`. Extracted verbatim from the legacy
    /// `handle_lib_key` Ctrl+P arm (the legacy arm resolves
    /// `current_lib_item` and calls this; the `EmbyLibraryContent` resolves its
    /// own selected item and routes it through the same tail) so the two
    /// paths share one body — the effect acts on the supplied item directly,
    /// never on a re-read App cursor.
    pub(super) fn play_or_activate_lib_item(&mut self, lib_idx: usize, item: EmbyItem) {
        if item.is_folder {
            let ct = self.libs[lib_idx].library.collection_type.clone();
            self.play_folder(&item.id.clone(), ct);
        } else {
            self.select_item(lib_idx, item);
        }
    }

    pub(super) fn select_item(&mut self, lib_idx: usize, item: EmbyItem) {
        if item.is_folder {
            let lib = &mut self.libs[lib_idx];
            lib.nav_stack.push(BrowseLevel {
                fetched_rows: 0,
                parent_id: item.id.clone(),
                title: item.name.clone(),
                items: vec![],
                total_count: 0,
                resting: BrowseResting::new(0, 0),
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: true,

                all_items: None,
                letter_filter: None,
                tv_content_mode: None,
                music_grouping: None,
            });
            self.save_default_library_position(lib_idx);
            self.spawn_browse(
                lib_idx,
                item.id,
                item.name,
                None,
                false,
                "SortName".into(),
                "Ascending".into(),
            );
        } else if is_playable(&item) {
            if self.is_feed_home_video_group_view(lib_idx) {
                let pos = self
                    .feed_home_video_selected_items(lib_idx)
                    .iter()
                    .position(|i| i.id == item.id);
                if let (Some(pos), Some(state)) = (pos, self.libs[lib_idx].feed_home_video.as_mut())
                {
                    state.video_cursor = pos;
                }
            } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                if let Some(pos) = lvl.items.iter().position(|i| i.id == item.id) {
                    lvl.set_resting_cursor(pos);
                }
            }
            let fresh = {
                let Some(client) = self.emby_client() else {
                    self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                    return;
                };
                let c = client.lock().unwrap();
                c.get_items_by_ids(std::slice::from_ref(&item.id))
                    .ok()
                    .and_then(|mut v| {
                        if v.is_empty() {
                            None
                        } else {
                            Some(v.remove(0))
                        }
                    })
                    .unwrap_or(item)
            };
            let autoload = self.config.lock().unwrap().autoload;
            if autoload {
                let parent_id = if self.is_feed_home_video_group_view(lib_idx) {
                    self.feed_home_video_selected_parent_id(lib_idx)
                } else {
                    self.libs[lib_idx]
                        .nav_stack
                        .last()
                        .map(|l| l.parent_id.clone())
                };
                if let Some(parent_id) = parent_id {
                    let Some(client) = self.emby_client() else {
                        self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
                        return;
                    };
                    let client = client.lock().unwrap();
                    match client.get_direct_playable(&parent_id) {
                        Ok(mut siblings) => {
                            siblings.retain(|i| !i.is_folder);
                            siblings.sort_by_key(|a| natural_sort_key(a.sort_key()));
                            if let Some(start_idx) = siblings.iter().position(|i| i.id == fresh.id)
                            {
                                let ct = self.libs[lib_idx].library.collection_type.clone();
                                drop(client);
                                self.replace_playback_queue(siblings.clone(), start_idx);
                                self.set_queue_source_if_not_local_daemon(
                                    crate::config::QueueSource::Collection {
                                        collection_type: ct,
                                    },
                                );
                                if !self.has_direct_remote_queue() {
                                    self.save_queue_state();
                                }
                                self.play_items_routed(
                                    siblings,
                                    start_idx,
                                    self.queue_source.clone(),
                                );
                                return;
                            }
                            drop(client);
                        }
                        Err(_) => {
                            drop(client);
                        }
                    }
                }
            }
            self.play_item(fresh);
        }
    }

    /// Candidate track sources for one album Workspace row, in resolution
    /// order (design D7, tasks 6.1–6.3). Ordinary album browsing fills
    /// `album_tracks_cache`; an artist root's Workspace rows come from the
    /// shell-owned `artist_detail_cache`, which the `ArtistIds` query populates
    /// without ever touching the album cache. `play_album_track` takes the
    /// first candidate holding the activated row's stable track ID, so a
    /// failed or truncated per-album page never hides the artist group the row
    /// actually came from. No cursor, no re-fetch, no component involvement.
    fn workspace_album_track_candidates(&self, album_id: &str) -> Vec<Vec<EmbyItem>> {
        let mut candidates = Vec::new();
        if let Some(tracks) = self.album_tracks_cache.get(album_id) {
            candidates.push(tracks.clone());
        }
        candidates.extend(
            self.artist_detail_cache
                .values()
                .filter(|entry| !entry.failed)
                .filter_map(|entry| {
                    let tracks: Vec<EmbyItem> = entry
                        .tracks
                        .iter()
                        .filter(|track| {
                            crate::app::state::music_artist_detail::track_matches_album(
                                track, album_id,
                            )
                        })
                        .cloned()
                        .collect();
                    (!tracks.is_empty()).then_some(tracks)
                }),
        );
        candidates
    }

    /// The one Grouped Music tree-track activation entry point (design D6),
    /// shared by the tree's Enter chord and its track double-click. Returns
    /// false when no cached candidate holds the track: resolution failure
    /// flashes the existing library error and leaves queue and playback
    /// unchanged.
    pub(super) fn play_grouped_track(&mut self, album_target: &str, track_id: &str) -> bool {
        let Some(action) = self.grouped_track_play_action(album_target, track_id) else {
            self.flash(
                "Library error: track is no longer available".into(),
                ToastSeverity::Error,
            );
            return false;
        };
        // The pending action is the only executable payload, so the queue
        // gate ahead of the executor cannot drift into a second playback
        // path: an empty target queue executes immediately, a populated one
        // asks before replacement and runs the stored action on confirmation.
        self.request_queue_replacement(action, ReplacementExecutor::Pending);
        true
    }

    /// Resolves one Grouped Music tree track from the settled album caches.
    /// The tree supplies only its stable album occurrence target and track ID;
    /// this shell-side resolver chooses the cached ordered queue according to
    /// the current autoload policy and returns the complete pending action.
    pub(super) fn grouped_track_play_action(
        &self,
        album_target: &str,
        track_id: &str,
    ) -> Option<PendingQueueAction> {
        let album_id = album_target.split('\0').next().unwrap_or(album_target);
        let (mut tracks, start_idx) = self.resolve_playable_album_tracks(album_id, track_id)?;
        let autoload = self.config.lock().unwrap().autoload;
        if autoload {
            Some(PendingQueueAction::PlayItems {
                items: tracks,
                start_idx,
                source: crate::config::QueueSource::Album,
                autostart: true,
            })
        } else {
            Some(PendingQueueAction::PlayItems {
                items: vec![tracks.remove(start_idx)],
                start_idx: 0,
                source: crate::config::QueueSource::Album,
                autostart: true,
            })
        }
    }

    fn replace_and_route_album_queue(&mut self, tracks: Vec<EmbyItem>, start_idx: usize) -> bool {
        if self.connected_session_id.is_none() && self.emby_snapshot().is_none() {
            self.flash("Emby is unavailable".into(), ToastSeverity::Warning);
            return false;
        }
        // The queue replacement is deferred behind the populated-queue gate;
        // its per-site prep (Album source, rebuild, conditional save) runs in
        // `run_routed_replacement` once the user confirms.
        let action = PendingQueueAction::PlayItems {
            items: tracks,
            start_idx,
            source: crate::config::QueueSource::Album,
            autostart: true,
        };
        self.request_queue_replacement(
            action,
            ReplacementExecutor::Routed(RoutedReplacementPrep::Album),
        );
        true
    }

    pub(super) fn play_album_track(&mut self, album_id: &str, track: &EmbyItem) -> bool {
        let Some((tracks, start_idx)) = self.resolve_playable_album_tracks(album_id, &track.id)
        else {
            return false;
        };
        self.replace_and_route_album_queue(tracks, start_idx)
    }

    /// The shared track-resolution sequence behind both `grouped_track_play_action`
    /// and `play_album_track`: find the cached candidate list containing
    /// `track_id`, keep only playable tracks, sort them, and locate `track_id`'s
    /// resulting position.
    fn resolve_playable_album_tracks(
        &self,
        album_id: &str,
        track_id: &str,
    ) -> Option<(Vec<EmbyItem>, usize)> {
        let mut tracks: Vec<EmbyItem> = self
            .workspace_album_track_candidates(album_id)
            .into_iter()
            .find(|tracks| tracks.iter().any(|track| track.id == track_id))?
            .into_iter()
            .filter(is_playable)
            .collect();
        sort_audio_tracks(&mut tracks);
        let start_idx = tracks.iter().position(|track| track.id == track_id)?;
        Some((tracks, start_idx))
    }

    /// Play the complete artist Workspace sequence from its selected track.
    /// The caller has resolved this ordered sequence from the shell-owned
    /// artist-detail projection; this method deliberately reuses the existing
    /// Album queue source and routed playback executor.
    pub(super) fn play_artist_tracks(&mut self, tracks: Vec<EmbyItem>, start_idx: usize) -> bool {
        if tracks.is_empty() || start_idx >= tracks.len() {
            return false;
        }
        self.replace_and_route_album_queue(tracks, start_idx)
    }

    pub(super) fn go_back(&mut self, lib_idx: usize) {
        // Defensive bounds check; see `move_lib_cursor_rows` in
        // `lib_cursor_actions.rs` for the stale index contract. Never
        // substitute library zero on a miss.
        if lib_idx >= self.libs.len() {
            return;
        }
        // Guard: don't pop when already at the root of a synthetic "group" view
        // (music groups: nav_stack[0]=groups, nav_stack[1]=albums; feed home
        // videos: nav_stack[0]=folders, nav_stack[1]=grouped videos) -- there is
        // no list above to go back to. Search-clearing still falls through
        // because this guard only fires when search is None.
        if self.libs[lib_idx].nav_stack.len() == 2
            && (self.is_music_group_view(lib_idx) || self.is_feed_home_video_group_view(lib_idx))
        {
            return;
        }

        // Primary pop -- scoped so the mutable borrow of libs[lib_idx] ends here.
        let did_pop = {
            let lib = &mut self.libs[lib_idx];
            if lib.nav_stack.len() > 1 {
                let child_folder_id = lib.nav_stack.last().map(|l| l.parent_id.clone());
                lib.nav_stack.pop();
                if let (Some(folder_id), Some(parent)) = (child_folder_id, lib.nav_stack.last_mut())
                {
                    if let Some(idx) = parent.items.iter().position(|i| i.id == folder_id) {
                        parent.set_resting_cursor(idx);
                    }
                }
                true
            } else {
                false
            }
        };

        if did_pop {
            self.save_default_library_position(lib_idx);

            // Skip past the auto-pushed Season level so a single Escape
            // takes the user back to the series list.
            let exposed_seasons = self.libs[lib_idx]
                .nav_stack
                .last()
                .map(|l| {
                    l.items
                        .first()
                        .map(|i| i.item_type == "Season")
                        .unwrap_or(false)
                })
                .unwrap_or(false);
            if exposed_seasons && self.libs[lib_idx].nav_stack.len() > 1 {
                let child_id2 = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.parent_id.clone());
                self.libs[lib_idx].nav_stack.pop();
                if let (Some(fid), Some(parent)) =
                    (child_id2, self.libs[lib_idx].nav_stack.last_mut())
                {
                    if let Some(idx) = parent.items.iter().position(|i| i.id == fid) {
                        parent.set_resting_cursor(idx);
                    }
                }
            }
        }
        self.save_default_library_position(lib_idx);
    }
}
