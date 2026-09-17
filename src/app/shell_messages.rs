use super::*;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::types_settings::PanelFocus;
use std::time::Instant;

impl Model {
    fn refresh_visual_selection(&mut self) {
        let summary = if self.app.effective_panel_focus() == PanelFocus::Queue {
            self.application
                .get_component(&crate::app::components::ComponentId::Queue)
                .and_then(|component| {
                    component
                        .as_any()
                        .downcast_ref::<crate::app::components::QueueComponent>()
                })
                .map(|queue| queue.selection_summary())
        } else {
            self.application
                .get_component_mut(&crate::app::components::ComponentId::Library)
                .and_then(|component| component.as_any_mut().downcast_mut::<LibraryPanel>())
                .and_then(|panel| panel.focused_summary())
        };
        self.visual_selection = summary
            .filter(|summary| summary.count > 0)
            .map(|summary| (self.app.effective_panel_focus(), summary.count));
    }

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
                match request {
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
                        self.push_music_workspace_content();
                    }
                    ShellRequest::MusicAlbumCursor { target, kind } => {
                        // Click-to-focus: a pointer-driven album-cursor move pulls
                        // panel focus to the Library. Keyboard moves only reach
                        // this arm while the Library is already focused, so this is
                        // idempotent there.
                        self.app.set_panel_focus(crate::app::PanelFocus::Library);
                        if let Some(lib_idx) = self.app.tab.emby_library_index() {
                            match kind {
                                AlbumCursorKind::Move => {
                                    let idle = self.app.list_image_fetches_allowed();
                                    let now = Instant::now();
                                    self.app.last_nav_at = now;
                                    self.app.mark_library_navigation(now);
                                    if self.app.move_music_group_display_cursor(lib_idx, target) {
                                        self.app.save_default_library_position(lib_idx);
                                        if idle {
                                            self.app.maybe_fetch_next_page(lib_idx, target);
                                        }
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
                    // Inline album-track activation/enqueue/context-menu
                    // target resolution: the component owns the cursor,
                    // the shell resolves it to the cached track and runs
                    // the App effect (task 5.3d, Album track focus).
                    ShellRequest::MusicTrackActivate { album_id, track } => {
                        self.app.play_album_track(&album_id, &track);
                        self.push_music_workspace_content();
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
                    // Help overlay cross-boundary requests (design D4).
                    ShellRequest::Quit => quit = true,
                    // Tab bar click: the mounted `TabPanel` resolved the tab from
                    // its own painted hit regions (task 2.1); the shell runs the
                    // same tab-switch entry point the keyboard path uses.
                    ShellRequest::TabSelect(tab_pos) => {
                        self.dismiss_active_inline_search();
                        self.visual_selection = None;
                        self.app.set_library_tab(tab_pos);
                    }
                    ShellRequest::DismissHelp => self.umount_help(),
                    ShellRequest::OpenSettings => {
                        self.umount_help();
                        self.mount_sidebar(super::super::SidebarId::Settings);
                    }
                    ShellRequest::OpenSessions => {
                        self.umount_help();
                        self.mount_sidebar(super::super::SidebarId::Sessions);
                    }
                    ShellRequest::OpenPlaylists => {
                        self.umount_help();
                        self.mount_sidebar(super::super::SidebarId::Playlists);
                        self.app.open_playlists_panel();
                    }
                    ShellRequest::ConfirmIntent(intent) => {
                        self.handle_confirm_intent(intent);
                        // Confirmations rewrite Home content/focus; re-project (5.3d).
                        self.push_home_content();
                        // Emby browser content may have changed (5.3d.15/M2).
                        self.push_active_emby_library_owner_content();
                    }
                    ShellRequest::DaemonLostIntent(intent) => {
                        if self.handle_daemon_lost_intent(intent) {
                            quit = true;
                        }
                    }
                    // Context menu: the shell owns cursor navigation and
                    // action execution; the component owns key interpretation
                    // (task 5.1).
                    ShellRequest::ContextMenuIntent(intent) => {
                        self.handle_context_menu_intent(intent);
                        // Enter executes the action, which can refetch Home; re-project (5.3d).
                        self.push_home_content();
                        // Emby browser content may have changed (5.3d.15/M2).
                        self.push_active_emby_library_owner_content();
                    }
                    ShellRequest::ContextMenuSelect(idx) => {
                        self.handle_context_menu_select(idx);
                        // A selected action can refetch Home; re-project (5.3d).
                        self.push_home_content();
                        // Emby browser content may have changed (5.3d.15/M2).
                        self.push_active_emby_library_owner_content();
                    }
                    ShellRequest::ContextMenuDismiss => {
                        self.app.pending_overlay =
                            Some(super::super::types_overlay::OverlayRequest::DismissContextMenu);
                    }
                    // Search sidebar: dismiss (Esc/Backspace-on-empty).
                    // The component owns the state; the shell unmounts it.
                    ShellRequest::DismissSearch => {
                        self.dismiss_sidebar(super::super::SidebarId::Search);
                    }
                    // Search sidebar: activate result (Enter). The
                    // component owns the cursor/results; the shell owns
                    // the library tabs and navigation spawn (task 3.2).
                    ShellRequest::SearchActivate { id, item_type } => {
                        self.app.activate_search_result(id, item_type);
                    }
                    ShellRequest::OpenInlineSearch => {
                        self.open_inline_search();
                    }
                    ShellRequest::InlineSearchQueryStarted => {
                        self.inline_search_query_started();
                    }
                    ShellRequest::InlineSearchActivate { id, item_type } => {
                        self.activate_inline_search_item(id, item_type);
                    }
                    ShellRequest::DismissSessions => {
                        self.dismiss_sidebar(super::super::SidebarId::Sessions);
                    }
                    ShellRequest::RefreshSessions => {
                        self.app.spawn_sessions_load();
                        self.app.spawn_cast_discovery();
                    }
                    ShellRequest::SelectSession(index) => {
                        if let Some(target) = self.app.panel_targets.get(index).cloned() {
                            self.app.select_panel_target(target);
                        }
                    }
                    ShellRequest::DetachSessions => {
                        let cast_attached = self.app.is_cast_attached();
                        // Skip disconnect_remote's "No session selected" toast when
                        // only a cast target is attached; the cast detach below is
                        // the real action in that case.
                        if self.app.can_disconnect_remote() || !cast_attached {
                            self.app.disconnect_remote();
                        }
                        if cast_attached {
                            self.app.detach_cast();
                            self.app.flash(
                                "Detached from cast target".to_string(),
                                ToastSeverity::Success,
                            );
                        }
                        self.dismiss_sidebar(super::super::SidebarId::Sessions);
                    }
                    ShellRequest::RefreshFeeds => {
                        self.app.refresh_feeds();
                    }
                    ShellRequest::FeedsPlay(entries) => {
                        if entries.is_empty() {
                            self.app
                                .flash("No feed entry selected".into(), ToastSeverity::Neutral);
                        } else {
                            self.app.play_feed_entries(entries);
                        }
                    }
                    ShellRequest::FeedsRowClick => {
                        // A Feeds list row the user clicked: the component already
                        // resolved and selected the row; the shell pulls panel
                        // focus to the Library (task 4.5, mirrors `HomeRowClick`).
                        self.app.set_panel_focus(crate::app::PanelFocus::Library);
                        self.sync_feeds();
                    }
                    ShellRequest::FeedsEnqueue(entries) => {
                        if entries.is_empty() {
                            self.app
                                .flash("No feed entry selected".into(), ToastSeverity::Neutral);
                        } else {
                            self.app.enqueue_feed_entries(entries);
                        }
                    }
                    ShellRequest::MultiselectCommit { .. } => {
                        self.handle_multiselect_commit();
                        // Hiding libraries/pills refetches Home inside the commit; re-project (5.3d).
                        self.push_home_content();
                        // Emby browser content may have changed (5.3d.15/M2).
                        self.push_active_emby_library_owner_content();
                    }
                    request @ ShellRequest::LibraryRoutesEnter
                    | request @ ShellRequest::LibraryRoutesEsc => {
                        self.handle_library_routes_request(request);
                    }
                    ShellRequest::FeedsManageIntent(intent) => {
                        self.handle_feeds_manage_intent(intent);
                    }
                    ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent) => {
                        // Typed podcast episode action intent (task 5.3d.7).
                        // The shell resolves the episode-selection and
                        // wide/narrow conditions from App state/layout and
                        // runs the existing App play/enter/modal/enqueue
                        // effects (D17); re-project after the effect.
                        self.handle_audiobookshelf_podcast_episode_intent(intent);
                        self.push_audiobookshelf_podcast_content();
                    }
                    ShellRequest::AudiobookshelfPodcastShowMove { library_item_id } => {
                        // Resolved podcast show-list cursor
                        // (split-audiobookshelf-cursor-ownership D1). The
                        // component already resolved its own movement and
                        // carries the landed index; apply it directly through
                        // the index-taking entry point (clamp + `state.select`
                        // + detail-fetch), never recomputing from a delta. The
                        // episode-selection guard lives only on the component
                        // now (D2).
                        // Click-to-focus (task 4.5): a mouse-driven (or already
                        // focused keyboard) show move pulls panel focus to the
                        // Library.
                        self.app.set_panel_focus(crate::app::PanelFocus::Library);
                        if let Some(library_item_id) = library_item_id.as_deref() {
                            self.app.select_audiobookshelf_show_target(library_item_id);
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
                    request @ (ShellRequest::AudiobookshelfBookMove(_)
                    | ShellRequest::AudiobookshelfBookIntent(_)) => {
                        self.handle_audiobookshelf_book_request(request);
                    }
                    // Browser selected-item typed effects (task 5.3d, Emby
                    // browser effect decoupling): the component reports the
                    // explicit `EmbyItem` target; the shell forwards it
                    // straight to the App effect (no App-cursor re-read).
                    request @ (ShellRequest::EmbyLibraryActivate { .. }
                    | ShellRequest::EmbyLibraryPlay { .. }
                    | ShellRequest::EmbyLibraryEnqueue { .. }
                    | ShellRequest::EmbyLibraryToggleWatched { .. }
                    | ShellRequest::EmbyLibraryShuffle { .. }
                    | ShellRequest::EmbyLibraryRefresh
                    | ShellRequest::EmbyLibraryRescan
                    | ShellRequest::EmbyLibraryBack
                    | ShellRequest::EmbyLibraryCycleLetterPill { .. }
                    | ShellRequest::EmbyLibraryCycleGroup { .. }) => {
                        self.handle_emby_library_request(request);
                        // Library navigation/effects change content; re-project all
                        // destination owners. Inactive owners are no-ops.
                        self.push_active_emby_library_owner_content();
                        self.push_music_workspace_content();
                        self.push_tv_workspace_content();
                    }
                    // Pure cursor movement: the component already resolved its own
                    // index, so apply the App-side nav effects but skip the content
                    // re-projection the effect requests above need.
                    request @ ShellRequest::EmbyLibraryCursorIndex { .. } => {
                        self.handle_emby_library_request(request);
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
                    ShellRequest::LibraryScroll {
                        key,
                        index,
                        scroll,
                        pagination_index,
                    } => {
                        let active_key = self
                            .active_emby_library_owner()
                            .map(|(_, active, _)| active);
                        if active_key.as_ref() == Some(&key) {
                            let Some(lib_idx) = self
                                .app
                                .libs
                                .iter()
                                .position(|lib| {
                                    matches!(&key, crate::app::components::library_panel::LibraryKey::Service { library_id, .. } if lib.library.id == *library_id)
                                })
                            else {
                                return quit;
                            };
                            if self.app.tab.emby_library_index() != Some(lib_idx) {
                                return quit;
                            }
                            {
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
                            }
                            // The wheel's reached position also feeds
                            // pagination (design D8): the owner resolved the
                            // window's last visible display row to the item
                            // index; the pending-fetch guard keeps repeated
                            // reports at the loaded end to one in-flight
                            // fetch.
                            if let Some(index) = pagination_index {
                                self.app.maybe_fetch_next_page(lib_idx, index);
                            }
                        }
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
                    ShellRequest::EmbyLibraryRowClick { target } => {
                        if let (Some(lib_idx), Some(target)) =
                            (self.app.tab.emby_library_index(), target)
                        {
                            self.app.handle_mouse_single_click_emby(lib_idx, target);
                        }
                        self.push_active_emby_library_owner_content();
                    }
                    ShellRequest::EmbyLibraryRowActivate { target } => {
                        if let (Some(lib_idx), Some(target)) =
                            (self.app.tab.emby_library_index(), target)
                        {
                            self.app.handle_mouse_double_click_emby(lib_idx, target);
                        }
                        self.push_active_emby_library_owner_content();
                    }
                    ShellRequest::HomeRowClick { .. } => {
                        self.app.set_panel_focus(crate::app::PanelFocus::Library);
                        self.push_home_content();
                    }
                    ShellRequest::HomeRowActivate { target } => {
                        self.app.set_panel_focus(crate::app::PanelFocus::Library);
                        if let Some((item, from_cw)) = self.home_stable_target(&target) {
                            self.app.home_play_target(item, from_cw);
                        }
                        self.push_home_content();
                    }
                    ShellRequest::HomePillClick { target } => {
                        self.select_home_section_from_component(target);
                    }
                    // Home typed effects (task 5.3d, Home typed-effect
                    // prep): `HomeComponent` owns the cursor and reports the
                    // flat target index it resolved; the shell forwards it
                    // straight to the `App` effect so the requested target
                    // is acted on directly (no App-owned flat cursor remains).
                    request @ (ShellRequest::HomePlay(_)
                    | ShellRequest::HomeEnqueue(_)
                    | ShellRequest::RowContextMenu(
                        crate::app::types_context_menu::ContextMenuTargets::Home(_),
                        _,
                    )
                    | ShellRequest::HomeDelete(_)
                    | ShellRequest::HomeToggleWatched(_)
                    | ShellRequest::HomeSectionSelected(_)) => self.handle_home_request(request),
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
                        crate::app::types_context_menu::ContextMenuTargets::Queue(slot_ids),
                        anchor,
                    ) => {
                        self.context_menu_origin =
                            Some(crate::app::components::media_list::SelectionOrigin::Queue);
                        self.context_action_snapshot =
                            Some(crate::app::types_context_menu::ContextActionSnapshot {
                                origin: crate::app::components::media_list::SelectionOrigin::Queue,
                                values: vec![
                                    crate::app::types_context_menu::ContextMenuTargets::Queue(
                                        slot_ids.clone(),
                                    ),
                                ],
                            });
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
                                    let remove = crate::app::types_context_menu::BulkRemoveTarget::Queue(*sid);
                                    let capability =
                                        crate::app::context_menu_capabilities::queue_item_capabilities(
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
                    // Other destination payloads are converted in later slices.
                    ShellRequest::RowContextMenu(targets, anchor) => {
                        // The origin is the active library's stable identity
                        // (design D6): a bulk clear routes to the list the menu
                        // was opened from, not the dispatch-time focus.
                        if let Some(origin) = self.active_library_selection_origin() {
                            self.context_menu_origin = Some(origin.clone());
                            self.context_action_snapshot =
                                Some(crate::app::types_context_menu::ContextActionSnapshot {
                                    origin,
                                    values: vec![targets.clone()],
                                });
                        }
                        match targets {
                            crate::app::types_context_menu::ContextMenuTargets::Emby(mut items) => {
                                if items.len() > 1 {
                                    let capabilities = items
                                        .iter()
                                        .map(crate::app::context_menu_capabilities::emby_item_capabilities)
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
                            crate::app::types_context_menu::ContextMenuTargets::Browser(
                                targets,
                            ) => {
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
                                                    level
                                                        .items
                                                        .iter()
                                                        .find(|item| item.id == *target)
                                                        .cloned()
                                                })
                                                .collect::<Vec<mbv_core::api::EmbyItem>>()
                                        })
                                        .unwrap_or_default();
                                    if items.len() > 1 {
                                        let capabilities = items
                                            .iter()
                                            .map(crate::app::context_menu_capabilities::emby_item_capabilities)
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
                            crate::app::types_context_menu::ContextMenuTargets::Feeds(entries) => {
                                self.app.open_feeds_context_menu(entries, anchor);
                            }
                            _ => {}
                        }
                        self.push_active_emby_library_owner_content();
                        self.push_music_workspace_content();
                        self.push_tv_workspace_content();
                    }
                    // TV keyboard requests are resolved by the mounted
                    // workspace component. Cursor and pane movement remain
                    // component-local; the shell handles only cross-boundary
                    // effects such as activation, back, and letter pills.
                    request @ (ShellRequest::TvMoveRows { .. }
                    | ShellRequest::TvJumpCursor { .. }
                    | ShellRequest::TvActivate { .. }
                    | ShellRequest::TvEpisodeActivate { .. }
                    | ShellRequest::TvBack
                    | ShellRequest::TvCycleLetterPill { .. }
                    | ShellRequest::TvEpisodeMove { .. }
                    | ShellRequest::TvSeasonMove { .. }) => self.handle_tv_request(request),
                    ShellRequest::TvHitClick { hit } => {
                        if let Some(lib_idx) = self.app.tab.emby_library_index() {
                            self.app.handle_mouse_single_click_tv(lib_idx, hit);
                        }
                        self.push_tv_workspace_content();
                    }
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
                    ShellRequest::SettingsIntent(intent) => {
                        if self.handle_settings_intent(intent) {
                            quit = true;
                        }
                    }
                    ShellRequest::SavePlaylistIntent(intent) => {
                        self.handle_save_playlist_intent(intent);
                    }
                    ShellRequest::QueueIntent(intent) => {
                        self.handle_queue_intent(intent);
                    }
                    // Live-only Wide hero split resize (add-mouse-wide-split-resize
                    // design.md): the gap boundary component owns the gesture and
                    // the resolved list-pane width; the shell clamps it against
                    // the active surface's content width and stores the session
                    // override. There is no end/persist arm -- nothing is
                    // persisted.
                    ShellRequest::ResizeListPaneLive(width) => {
                        if let Some(content_area) = self.library_panel_content_area() {
                            self.app.list_pane_width =
                                crate::app::list_pane_width::normalize_list_pane_width(
                                    Some(width),
                                    content_area.width,
                                );
                        }
                    }
                    // Component owns episode-pane focus/episode_filter; mutated locally in
                    // PodcastContent::on_key before the request is emitted, and
                    // handle_audiobookshelf_podcast_episode_intent resolves the target from the
                    // component, not App state (commit 0227d748, migrate-tui-to-tuirealm task
                    // 5.3d.11 U2). No shell effect remains.
                    ShellRequest::AudiobookshelfPodcastEpisodeTransition(_) => {}
                    // Emitted only from SettingsComponent::handle_mouse (settings.rs:318); the
                    // keyboard dismiss is SettingsIntent::Back. Mouse-only, inert under D16
                    // (migrate-tui-to-tuirealm design D16, #628).
                    ShellRequest::DismissSettings => {}
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

    /// Re-project after a Queue click: the click moves panel focus to the
    /// Queue panel (re-project the Home focus flag) and may mutate Emby
    /// browser content (5.3d.15/M2).
    fn queue_click_reproject(&mut self) {
        self.push_home_content();
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
