use super::*;

impl super::super::Model {
    pub(super) fn handle_music_request(
        &mut self,
        request: ShellRequest,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> Option<ShellRequest> {
        match request {
            request @ (ShellRequest::MusicAlbumActivate { .. }
            | ShellRequest::MusicArtistActivate { .. }) => self.handle_music_activation(request),
            ShellRequest::MusicNeighbourPrefetch { targets } => {
                // Task 6.5 (design D4): the artwork payload is the
                // tree's own ordered neighbour window from its
                // completed paint. The fetch applies the existing
                // idle gate. Source pagination for this album level
                // is unconditional (`maybe_fetch_next_page_sized`
                // loads it to completion), so it no longer rides
                // this payload.
                self.app.prefetch_neighbour_album_art(&targets);
            }
            ShellRequest::MusicArtistTracks { target } => {
                self.handle_music_artist_tracks(target, music_resize, tv_resize);
            }
            ShellRequest::MusicArtistArtwork { target } => {
                self.request_music_artist_artwork(target);
                self.push_music_workspace_content();
            }
            ShellRequest::MusicAlbumCursor { target, kind } => {
                self.handle_music_album_cursor(target, kind);
            }
            ShellRequest::MusicArtistAction {
                action,
                items,
                origin,
                unresolved_targets,
            } => self.handle_music_artist_action(action, items, origin, unresolved_targets),
            ShellRequest::MusicTreeTrackActivate {
                album_target,
                track_id,
            } => {
                self.app.set_panel_focus(crate::app::PanelFocus::Library);
                self.app.play_grouped_track(&album_target, &track_id);
                self.push_music_workspace_content();
            }
            // Inline album-track activation/enqueue/context-menu
            // target resolution: the component owns the cursor,
            // the shell resolves it to the cached track and runs
            // the App effect (task 5.3d, Album track focus).
            ShellRequest::MusicTrackActivate { album_id, track } => {
                self.app.set_panel_focus(crate::app::PanelFocus::Library);
                self.app.play_album_track(&album_id, &track);
                self.push_music_workspace_content();
            }
            ShellRequest::MusicArtistTrackActivate { target, track_id } => {
                self.handle_music_artist_track_activate(target, track_id);
            }
            ShellRequest::MusicGroupSwitch { delta } => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    self.app.switch_music_group(lib_idx, delta);
                }
                // A group switch replaces the album level; re-anchor the
                // workspace cursor at this nav event (mirrors the pill
                // click path in `ShellRequest::EmbyLibraryPillClick`).
                self.music_workspace_reanchor = true;
                self.push_music_workspace_content();
            }
            _ => return Some(request),
        }
        None
    }

    fn handle_music_activation(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::MusicAlbumActivate { item } => {
                let owner_has_target = self
                    .music_owner()
                    .and_then(|owner| owner.selected_item())
                    .is_some_and(|selected| selected.id == item.id);
                if self.app.tab.emby_library_index().is_some()
                    && !self.app.is_right_panel_wide()
                    && owner_has_target
                {
                    self.open_library_hero_overlay();
                }
            }
            ShellRequest::MusicArtistActivate { target } => {
                // Right on an already expanded artist root (task 6.4):
                // non-Wide geometry opens the artist's Library Hero overlay.
                if self.app.tab.emby_library_index().is_some()
                    && !self.app.is_right_panel_wide()
                    && self.music_owner().is_some_and(|owner| {
                        owner
                            .artist_detail_target()
                            .is_some_and(|current| current.same_source(&target))
                    })
                {
                    self.open_library_hero_overlay();
                }
            }
            _ => unreachable!("only music activation requests are handled here"),
        }
        self.push_music_workspace_content();
    }

    fn handle_music_artist_tracks(
        &mut self,
        target: super::super::components::msg::MusicArtistTarget,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) {
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            self.app
                .maybe_fetch_next_page_for_music_artist(lib_idx, &target.album_targets);
        }
        self.request_music_artist_tracks(target.clone());
        self.handle_terminal_message(
            Msg::Shell(Box::new(ShellRequest::MusicArtistArtwork { target })),
            music_resize,
            tv_resize,
        );
    }

    fn handle_music_album_cursor(
        &mut self,
        target: usize,
        kind: crate::app::components::msg::AlbumCursorKind,
    ) {
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            match kind {
                AlbumCursorKind::Move => {
                    let idle = self.app.list_image_fetches_allowed();
                    let now = Instant::now();
                    self.app.last_nav_at = now;
                    self.app.mark_library_navigation(now);
                    let moved = self.app.move_music_group_display_cursor(lib_idx, target);
                    if moved {
                        self.app.save_default_library_position(lib_idx);
                    }
                    if moved && idle {
                        self.app.maybe_fetch_next_page(lib_idx, target);
                    }
                }
                AlbumCursorKind::Jump => {
                    if self.app.jump_music_group_display_cursor(lib_idx, target) {
                        self.app.save_default_library_position(lib_idx);
                        self.app.maybe_fetch_next_page(lib_idx, target);
                    }
                }
                AlbumCursorKind::Page => {
                    self.app.page_grouped_album_cursor(lib_idx, target);
                }
            }
        }
        self.push_music_workspace_content();
    }

    fn handle_music_artist_action(
        &mut self,
        action: crate::app::components::msg::MusicTreeAction,
        items: Vec<mbv_core::api::EmbyItem>,
        origin: crate::app::components::media_list::SelectionOrigin,
        unresolved_targets: Vec<String>,
    ) {
        use crate::app::components::msg::MusicTreeAction;

        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        let had_items = !items.is_empty();
        match action {
            MusicTreeAction::Play | MusicTreeAction::Shuffle => {
                if had_items {
                    self.app
                        .play_music_albums(items, matches!(action, MusicTreeAction::Shuffle));
                }
            }
            MusicTreeAction::Enqueue => {
                for item in items {
                    self.handle_emby_library_request(ShellRequest::EmbyLibraryEnqueue { item });
                }
            }
        }
        self.clear_multi_selection_from_origin(origin);
        if unresolved_targets.is_empty() {
            if !had_items {
                self.app
                    .flash("No artist albums available".into(), ToastSeverity::Neutral);
            }
        } else {
            self.app.flash(
                format!(
                    "{} artist album{} unavailable",
                    unresolved_targets.len(),
                    if unresolved_targets.len() != 1 {
                        "s"
                    } else {
                        ""
                    }
                ),
                ToastSeverity::Warning,
            );
        }
        self.push_music_workspace_content();
    }

    fn handle_music_artist_track_activate(
        &mut self,
        target: super::super::components::msg::MusicArtistTarget,
        track_id: String,
    ) {
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        let resolved = self.music_owner().and_then(|owner| {
            let detail = owner.artist_detail_for_target(&target)?;
            let tracks: Vec<mbv_core::api::EmbyItem> = detail
                .track_groups
                .iter()
                .flat_map(|group| group.tracks.iter())
                .filter(|track| crate::app::ui_util::is_playable(track))
                .cloned()
                .collect();
            let start_idx = tracks.iter().position(|track| track.id == track_id)?;
            Some((tracks, start_idx))
        });
        if let Some((tracks, start_idx)) = resolved {
            self.app.play_artist_tracks(tracks, start_idx);
        } else {
            self.app.flash(
                "Library error: artist track is no longer available".into(),
                ToastSeverity::Error,
            );
        }
        self.push_music_workspace_content();
    }
}
