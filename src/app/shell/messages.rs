mod music;
mod navigation;
mod selection;
#[cfg(test)]
mod tests;

use super::*;
use crate::app::components::library_panel::LibraryPanel;
use std::time::Instant;

impl Model {
    pub(crate) fn handle_terminal_message(
        &mut self,
        msg: Msg,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> bool {
        self.refresh_visual_selection();
        let mut quit = false;
        match msg {
            Msg::TerminalEvent(event) => {
                apply_terminal_observer(self, event, music_resize, tv_resize)
            }
            Msg::Shell(request) => {
                let request = *request;
                let request = self.handle_music_request(request, music_resize, tv_resize);
                let request =
                    request.and_then(|request| self.handle_navigation_request(request, &mut quit));
                if let Some(request) = request {
                    let request_quit = self.handle_shell_request(request, music_resize, tv_resize);
                    return quit || request_quit;
                }
            }
            Msg::Queue(request) => {
                self.handle_queue_request(request);
            }
            Msg::Playback(request) => {
                self.handle_playback_request(request);
            }
            Msg::Service(request) => {
                if self.handle_service_request(request) {
                    quit = true;
                }
            }
        }
        if self.drain_deferred_library_message(music_resize, tv_resize) {
            quit = true;
        }
        quit
    }

    fn handle_shell_request(
        &mut self,
        request: ShellRequest,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> bool {
        let mut quit = false;
        match request {
            ShellRequest::LibraryPanelFocus => {
                self.app.set_panel_focus(crate::app::PanelFocus::Library);
            }
            ShellRequest::SelectionProjection(summary) => {
                self.visual_selection = (summary.count > 0)
                    .then_some((self.app.effective_panel_focus(), summary.count));
            }
            ShellRequest::ClearMultiSelection(origin) => {
                // Route by the origin captured when the pill was
                // projected, never a re-derivation from dispatch-time
                // focus (design D6).
                self.clear_multi_selection_from_origin(origin);
            }
            ShellRequest::AudiobookshelfPodcastShowMove { library_item_id } => {
                self.handle_podcast_show_move_request(library_item_id);
            }
            request @ (ShellRequest::AudiobookshelfBookMove(_)
            | ShellRequest::AudiobookshelfBookIntent(_)) => {
                self.handle_audiobookshelf_book_request(request);
            }
            // Emby-browser requests preserve their per-effect projection policy.
            request @ (ShellRequest::EmbyLibraryActivate { .. }
            | ShellRequest::EmbyLibraryPlay { .. }
            | ShellRequest::EmbyLibraryEnqueue { .. }
            | ShellRequest::EmbyLibraryToggleWatched { .. }
            | ShellRequest::EmbyLibraryShuffle { .. }
            | ShellRequest::EmbyLibraryRefresh
            | ShellRequest::EmbyLibraryRescan
            | ShellRequest::EmbyLibraryBack
            | ShellRequest::EmbyLibraryCursorIndex { .. }
            | ShellRequest::EmbyLibraryPillClick { .. }
            | ShellRequest::EmbyLibraryLatestSelected
            | ShellRequest::EmbyLibraryLatestExit { .. }
            | ShellRequest::EmbyLibraryRowClick { .. }
            | ShellRequest::EmbyLibraryRowActivate { .. }) => {
                self.handle_emby_shell_request(request);
            }
            ShellRequest::OpenUrl(url) => {
                if crate::app::components::library_panel::sanitize_url(&url).is_some() {
                    if let Err(error) = crate::app::open_url(&url) {
                        log::warn!(target: "library_link", "Failed to open provider link {url:?}: {error}");
                        self.app.flash(
                            format!("Unable to open link: {error}"),
                            ToastSeverity::Neutral,
                        );
                    }
                }
            }
            ShellRequest::LibraryScroll { key, index, scroll } => {
                if self
                    .handle_library_scroll_request(key, index, scroll)
                    .is_none()
                {
                    // Preserve the legacy short-circuit: this path skips the deferred-message drain.
                    return quit;
                }
            }
            ShellRequest::HomeRowClick { .. } => {
                self.app.set_panel_focus(crate::app::PanelFocus::Library);
            }
            ShellRequest::HomeRowActivate { target } => {
                self.app.set_panel_focus(crate::app::PanelFocus::Library);
                if let Some((item, from_cw)) = self.home_stable_target(&target) {
                    self.app.home_play_target(item, from_cw);
                }
            }
            // Home typed effects (task 5.3d, Home typed-effect
            // prep): `HomeComponent` owns the cursor and reports the
            // flat target index it resolved; the shell forwards it
            // straight to the `App` effect so the requested target
            // is acted on directly (no App-owned flat cursor remains).
            request @ (ShellRequest::HomePlay(_)
            | ShellRequest::HomeEnqueue(_)
            | ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Home(_),
                _,
            )
            | ShellRequest::HomeDelete(_)
            | ShellRequest::HomeToggleWatched(_)) => self.handle_home_request(request),
            ShellRequest::QueueScopeClick { scope } => {
                self.app.handle_mouse_selector_click_queue(scope);
                self.queue_click_reproject();
            }
            ShellRequest::QueueRowClick { slot_id } => {
                self.app.handle_mouse_single_click_queue(slot_id);
                self.queue_click_reproject();
            }
            ShellRequest::QueueRowActivate { slot_id } => {
                self.app.handle_mouse_double_click_queue(slot_id);
                self.queue_click_reproject();
            }
            ShellRequest::RowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Queue(slot_ids),
                anchor,
            ) => self.handle_queue_row_context_menu(slot_ids, anchor),
            ShellRequest::MusicRowContextMenu(targets, anchor) => {
                self.app.set_panel_focus(crate::app::PanelFocus::Library);
                // Reuse the generic resolver after applying Music's
                // focus policy; the component still emitted only one
                // semantic request and the shell never re-resolves a
                // pointer coordinate.
                quit |= self.handle_shell_request(
                    ShellRequest::RowContextMenu(targets, anchor),
                    music_resize,
                    tv_resize,
                );
            }
            // Other destination payloads are converted in later slices.
            ShellRequest::RowContextMenu(targets, anchor) => {
                self.handle_library_row_context_menu(targets, anchor);
            }
            request @ (ShellRequest::TvMoveRows { .. }
            | ShellRequest::TvJumpCursor { .. }
            | ShellRequest::TvActivate { .. }
            | ShellRequest::TvEpisodeActivate { .. }
            | ShellRequest::TvBack
            | ShellRequest::TvCycleLetterPill { .. }
            | ShellRequest::TvEpisodeMove { .. }
            | ShellRequest::TvSeasonMove { .. }
            | ShellRequest::TvTreeExpand { .. }
            | ShellRequest::TvHitClick { .. }
            | ShellRequest::TvHitDoubleClick { .. }
            | ShellRequest::PlaylistsBack
            | ShellRequest::PlaylistsOpen(_)
            | ShellRequest::PlaylistsActivate { .. }
            | ShellRequest::PlaylistsRename(_)
            | ShellRequest::PlaylistsDelete(_)
            | ShellRequest::PlaylistsRefresh
            | ShellRequest::DismissPlaylists
            | ShellRequest::SettingsIntent(_)) => {
                quit |= self.handle_tv_playlist_settings_request(request);
            }
            ShellRequest::SavePlaylistIntent(intent) => {
                self.handle_save_playlist_intent(intent);
            }
            ShellRequest::QueueIntent(intent) => {
                self.handle_queue_intent(intent);
            }
            // Wide hero split resize (add-mouse-wide-split-resize
            // design.md): the gap boundary component owns the gesture and
            // the resolved list-pane width; the shell clamps it against
            // the active surface's content width and stores the session
            // override. Live positions do not write preferences.
            ShellRequest::ResizeListPaneLive(width) => {
                if let Some(content_area) = self.library_panel_content_area() {
                    self.app.list_pane_width =
                        crate::app::state::list_pane_width::normalize_list_pane_width(
                            Some(width),
                            content_area.width,
                        );
                }
            }
            // The panel emits End only after a changed drag. Persist
            // once at release, mirroring the Queue boundary's split.
            ShellRequest::ResizeListPaneEnd(width) => {
                if let Some(content_area) = self.library_panel_content_area() {
                    self.app.list_pane_width =
                        crate::app::state::list_pane_width::normalize_list_pane_width(
                            Some(width),
                            content_area.width,
                        );
                }
                self.app.save_prefs();
            }
            // Emitted only from SettingsComponent::handle_mouse (settings.rs:318); the
            // keyboard dismiss is SettingsIntent::Back. Mouse-only, inert under D16
            // (migrate-tui-to-tuirealm design D16, #628).
            ShellRequest::DismissSettings => {}
            _ => unreachable!("request handled by message sub-dispatchers"),
        }
        if self.drain_deferred_library_message(music_resize, tv_resize) {
            quit = true;
        }
        quit
    }

    fn handle_emby_shell_request(&mut self, request: ShellRequest) {
        match request {
            // Browser selected-item typed effects (task 5.3d, Emby
            // browser effect decoupling): forward the explicit target without
            // re-reading the App cursor, then re-project all destination owners.
            request @ (ShellRequest::EmbyLibraryActivate { .. }
            | ShellRequest::EmbyLibraryPlay { .. }
            | ShellRequest::EmbyLibraryEnqueue { .. }
            | ShellRequest::EmbyLibraryToggleWatched { .. }
            | ShellRequest::EmbyLibraryShuffle { .. }
            | ShellRequest::EmbyLibraryRefresh
            | ShellRequest::EmbyLibraryRescan
            | ShellRequest::EmbyLibraryBack) => {
                self.handle_emby_library_request(request);
                self.reproject_all_owners();
            }
            // Cursor movement already carries the component-resolved index;
            // unlike content-changing effects, it does not re-project.
            request @ ShellRequest::EmbyLibraryCursorIndex { .. } => {
                self.handle_emby_library_request(request);
            }
            ShellRequest::EmbyLibraryPillClick { target } => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    self.app.handle_mouse_selector_click_emby(lib_idx, target);
                }
                // A music-group pill switch replaces the album level;
                // re-anchor the workspace cursor at this nav event.
                self.music_workspace_reanchor = true;
                self.push_active_emby_library_owner_content();
            }
            ShellRequest::EmbyLibraryLatestSelected => {
                self.handle_emby_library_latest_selected();
            }
            ShellRequest::EmbyLibraryLatestExit { target } => {
                self.handle_emby_library_latest_exit(target);
            }
            ShellRequest::EmbyLibraryRowClick { target } => {
                if let (Some(lib_idx), Some(target)) = (self.app.tab.emby_library_index(), target) {
                    self.app.handle_mouse_single_click_emby(lib_idx, target);
                }
                self.push_active_emby_library_owner_content();
            }
            ShellRequest::EmbyLibraryRowActivate { target } => {
                if let (Some(lib_idx), Some(target)) = (self.app.tab.emby_library_index(), target) {
                    self.app.handle_mouse_double_click_emby(lib_idx, target);
                }
                self.push_active_emby_library_owner_content();
            }
            _ => unreachable!("request is an Emby library request"),
        }
    }

    fn handle_tv_playlist_settings_request(&mut self, request: ShellRequest) -> bool {
        match request {
            // TV keyboard requests are resolved by the mounted workspace
            // component. Cursor and pane movement remain component-local.
            request @ (ShellRequest::TvMoveRows { .. }
            | ShellRequest::TvJumpCursor { .. }
            | ShellRequest::TvActivate { .. }
            | ShellRequest::TvEpisodeActivate { .. }
            | ShellRequest::TvBack
            | ShellRequest::TvCycleLetterPill { .. }
            | ShellRequest::TvEpisodeMove { .. }
            | ShellRequest::TvSeasonMove { .. }
            | ShellRequest::TvTreeExpand { .. }) => {
                self.handle_tv_request(request);
            }
            ShellRequest::TvHitClick { hit } => self.handle_tv_hit_click(hit),
            ShellRequest::TvHitDoubleClick { hit } => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    self.app.handle_mouse_double_click_tv(lib_idx, hit);
                }
                self.push_tv_workspace_content();
            }
            request @ (ShellRequest::PlaylistsBack
            | ShellRequest::PlaylistsOpen(_)
            | ShellRequest::PlaylistsActivate { .. }
            | ShellRequest::PlaylistsRename(_)
            | ShellRequest::PlaylistsDelete(_)
            | ShellRequest::PlaylistsRefresh
            | ShellRequest::DismissPlaylists) => self.handle_playlists_request(request),
            ShellRequest::SettingsIntent(intent) => return self.handle_settings_intent(intent),
            _ => unreachable!("request is a TV, playlist, or settings request"),
        }
        false
    }

    pub(crate) fn drain_deferred_library_message(
        &mut self,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> bool {
        let Some(deferred) = self
            .application
            .get_component_mut(&ComponentId::Library)
            .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
            .and_then(LibraryPanel::take_deferred_msg)
        else {
            return false;
        };
        self.handle_terminal_message(deferred, music_resize, tv_resize)
    }

    /// Resolved podcast show-list cursor
    /// (split-audiobookshelf-cursor-ownership D1). The
    /// component already resolved its own movement and
    /// carries the landed index; apply it directly through
    /// the index-taking entry point (clamp, `state.select`, detail-fetch), never recomputing from a delta. The
    /// episode-selection guard lives only on the component
    /// now (D2).
    fn handle_podcast_show_move_request(&mut self, library_item_id: Option<String>) {
        // Click-to-focus (task 4.5): a mouse-driven (or already
        // focused keyboard) show move pulls panel focus to the
        // Library.
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        if let Some(library_item_id) = library_item_id.as_deref() {
            self.app.select_audiobookshelf_show_target(library_item_id);
        } else {
            // A state-pill scope (or its list interaction): the
            // fan-out's required shows are every listed show
            // (design D5).
            self.app.commit_audiobookshelf_podcast_state_scope();
        }
        // The component owns the painted cursor; persist the
        // active tab's slot once after the movement lands so
        // the saved position tracks the moved cursor (B3).
        if let Some(index) = self.app.tab.audiobookshelf_index() {
            self.app.save_audiobookshelf_position(index);
        }
        // `select_audiobookshelf_show` rewrote the active browse
        // state (cursor/selection); re-project (5.3d.11 U6).
        self.push_audiobookshelf_podcast_content();
    }

    /// Apply a `LibraryScroll` to the active Emby library owner. Returns
    /// `None` for the two early-exit paths that previously returned `quit`
    /// straight out of `handle_terminal_message` (skipping the deferred
    /// drain); `Some(())` continues normal flow.
    fn handle_library_scroll_request(
        &mut self,
        key: crate::app::components::library_panel::LibraryKey,
        index: usize,
        scroll: usize,
    ) -> Option<()> {
        let active_key = self
            .active_emby_library_owner()
            .map(|(_, active, _)| active);
        if active_key.as_ref() != Some(&key) {
            return Some(());
        }
        let lib_idx = self
            .app
            .libs
            .iter()
            .position(|lib| {
                matches!(&key, crate::app::components::library_panel::LibraryKey::Service { library_id, .. } if lib.library.id == *library_id)
            })?;
        if self.app.tab.emby_library_index() != Some(lib_idx) {
            return None;
        }
        self.app.persist_library_scroll(lib_idx, scroll);
        // The owner already applied the movement; retain
        // the resolved cursor for App-side effects only.
        if index
            < self.app.libs[lib_idx]
                .nav_stack
                .last()
                .map_or(0, |level| level.items.len())
        {
            self.app.libs[lib_idx]
                .nav_stack
                .last_mut()
                .expect("validated nav stack")
                .set_resting_cursor(index);
        }
        Some(())
    }

    /// The Queue `RowContextMenu` arm: bulk selection menu or single-row
    /// right-click/keyboard menu, then the queue re-projection.
    fn handle_queue_row_context_menu(
        &mut self,
        slot_ids: Vec<mbv_core::playback_queue::QueueSlotId>,
        anchor: Option<(u16, u16)>,
    ) {
        self.context_menu_origin = Some(crate::app::components::media_list::SelectionOrigin::Queue);
        self.context_action_snapshot = Some(
            crate::app::state::types::context_menu::ContextActionSnapshot {
                origin: crate::app::components::media_list::SelectionOrigin::Queue,
                values: vec![
                    crate::app::state::types::context_menu::ContextMenuTargets::Queue(
                        slot_ids.clone(),
                    ),
                ],
            },
        );
        if slot_ids.len() > 1 {
            let scope = self.app.viewed_queue_scope();
            let (items, remove_targets, capabilities) = slot_ids
                .iter()
                .filter_map(|sid| {
                    let slot = self
                        .app
                        .queue_for_scope(scope)
                        .slots()
                        .iter()
                        .find(|s| s.slot_id == *sid)?;
                    let item = slot.item.as_emby().cloned();
                    let remove =
                        crate::app::state::types::context_menu::BulkRemoveTarget::Queue(*sid);
                    let capability =
                        crate::app::state::context_menu_capabilities::queue_item_capabilities(
                            &slot.item,
                        );
                    Some((item, remove, capability))
                })
                .fold(
                    (Vec::new(), Vec::new(), Vec::new()),
                    |(mut items, mut removes, mut capabilities), (item, remove, capability)| {
                        if let Some(item) = item {
                            items.push(item);
                        }
                        removes.push(remove);
                        capabilities.push(capability);
                        (items, removes, capabilities)
                    },
                );
            self.app.open_context_menu_for_selection(
                items,
                anchor,
                crate::app::PanelFocus::Queue,
                capabilities,
                remove_targets,
            );
        } else {
            let slot_id = slot_ids.into_iter().next();
            let cw_selected = self.home_continue_watching_selected();
            if let Some((x, y)) = anchor {
                self.app
                    .handle_mouse_right_click_queue(slot_id, x, y, cw_selected);
            } else {
                self.app
                    .handle_keyboard_context_menu_queue(slot_id, cw_selected);
            }
        }
        self.queue_click_reproject();
    }

    /// The generic (non-Queue, non-Music) `RowContextMenu` arm: record the
    /// selection origin/snapshot, open the destination-appropriate menu, and
    /// re-project all destination owners.
    fn handle_library_row_context_menu(
        &mut self,
        targets: crate::app::state::types::context_menu::ContextMenuTargets,
        anchor: Option<(u16, u16)>,
    ) {
        // The origin is the active library's stable identity
        // (design D6): a bulk clear routes to the list the menu
        // was opened from, not the dispatch-time focus.
        if let Some(origin) = self.active_library_selection_origin() {
            self.context_menu_origin = Some(origin.clone());
            self.context_action_snapshot = Some(
                crate::app::state::types::context_menu::ContextActionSnapshot {
                    origin,
                    values: vec![targets.clone()],
                },
            );
        }
        match targets {
            crate::app::state::types::context_menu::ContextMenuTargets::Emby(mut items) => {
                if items.len() > 1 {
                    let capabilities = items
                        .iter()
                        .map(crate::app::state::context_menu_capabilities::emby_item_capabilities)
                        .collect();
                    self.app.open_context_menu_for_selection(
                        items,
                        anchor,
                        crate::app::PanelFocus::Library,
                        capabilities,
                        Vec::new(),
                    );
                } else if let Some(item) = items.pop() {
                    self.focus_emby_context_item(&item);
                    if let Some((x, y)) = anchor {
                        self.app.open_context_menu_for_at(item, x, y);
                    } else {
                        self.app.open_context_menu_for(item);
                    }
                }
            }
            crate::app::state::types::context_menu::ContextMenuTargets::Browser(targets) => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    let items = self
                        .app
                        .libs
                        .get(lib_idx)
                        .and_then(|lib| lib.nav_stack.last())
                        .map(|level| {
                            targets
                                .iter()
                                .filter_map(|target| {
                                    level.items.iter().find(|item| item.id == *target).cloned()
                                })
                                .collect::<Vec<mbv_core::api::EmbyItem>>()
                        })
                        .unwrap_or_default();
                    if items.len() > 1 {
                        let capabilities = items
                            .iter()
                            .map(crate::app::state::context_menu_capabilities::emby_item_capabilities)
                            .collect();
                        self.app.open_context_menu_for_selection(
                            items,
                            anchor,
                            crate::app::PanelFocus::Library,
                            capabilities,
                            Vec::new(),
                        );
                    } else if let Some(item) = items.into_iter().next() {
                        self.focus_emby_context_item(&item);
                        if let Some((x, y)) = anchor {
                            self.app.open_context_menu_for_at(item, x, y);
                        } else {
                            self.app.open_context_menu_for(item);
                        }
                    }
                }
            }
            crate::app::state::types::context_menu::ContextMenuTargets::Feeds(entries) => {
                self.app.open_feeds_context_menu(entries, anchor);
            }
            _ => {}
        }
        self.reproject_all_owners();
    }

    fn reproject_all_owners(&mut self) {
        self.push_active_emby_library_owner_content();
        self.push_music_workspace_content();
        self.push_tv_workspace_content();
    }

    /// The `EmbyLibraryLatestSelected` arm: snapshot the destination Latest
    /// shelf, record the acknowledgement on the music owner, re-project.
    fn handle_emby_library_latest_selected(&mut self) {
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            self.app.spawn_destination_latest_snapshot(lib_idx);
        }
        if let (Some(_), Some(lib_idx)) =
            (self.music_owner_key(), self.app.tab.emby_library_index())
        {
            if let Some(library) = self.app.libs.get(lib_idx) {
                self.record_home_latest_acknowledgement(DestinationLatestSource::Emby(
                    library.library.id.clone(),
                ));
            }
            self.push_music_workspace_content();
        } else {
            self.push_active_emby_library_owner_content();
        }
    }

    /// The `EmbyLibraryLatestExit` arm: leave the Latest shelf, restoring the
    /// letter filter or the home-video group selection.
    fn handle_emby_library_latest_exit(&mut self, target: usize) {
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            if target == usize::MAX {
                self.clear_emby_letter_filter(lib_idx);
            } else {
                let returning_to_selected_home_video_group =
                    self.app.is_feed_home_video_group_view(lib_idx)
                        && self.app.feed_home_video_selected_group_index(lib_idx) == target;
                if !returning_to_selected_home_video_group {
                    self.app.handle_mouse_selector_click_emby(lib_idx, target);
                }
            }
        }
        self.push_active_emby_library_owner_content();
    }

    /// The `TvHitClick` arm: single-click the TV hit, acknowledge the Latest
    /// marker for letter pills, and always repaint the TV owner.
    fn handle_tv_hit_click(&mut self, hit: crate::app::components::msg::TvHit) {
        let acknowledge_latest = matches!(hit, crate::app::components::msg::TvHit::LetterPill(0));
        if let Some(lib_idx) = self.app.tab.emby_library_index() {
            self.app.handle_mouse_single_click_tv(lib_idx, hit);
        }
        if acknowledge_latest {
            self.acknowledge_active_tv_latest();
        }
        // Always repaint the TV owner from the mutated App
        // browse state: the acknowledgement path pushes inside
        // `acknowledge_home_latest`, but its early returns
        // (non-TV library, non-Latest mode) would otherwise
        // leave a swallowed click's mutation unpainted.
        self.push_tv_workspace_content();
    }

    /// Re-project the Emby browser after queue actions that may mutate it.
    fn queue_click_reproject(&mut self) {
        self.push_active_emby_library_owner_content();
    }

    /// Context-menu targets are resolved by a component, but retain the old
    /// library click side effects: a library row context menu focuses Library
    /// and pins the persistence-facing cursor to that row. This is shared by
    /// keyboard and pointer requests so actions cannot fall back to a stale
    /// cursor in another panel.
    fn focus_emby_context_item(&mut self, item: &mbv_core::api::EmbyItem) {
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        let Some(lib_idx) = self.app.tab.emby_library_index() else {
            return;
        };
        let Some(level) = self
            .app
            .libs
            .get_mut(lib_idx)
            .and_then(|lib| lib.nav_stack.last_mut())
        else {
            return;
        };
        if let Some(index) = level
            .items
            .iter()
            .position(|candidate| candidate.id == item.id)
        {
            level.set_resting_cursor(index);
            self.app.save_default_library_position(lib_idx);
        }
    }
}
