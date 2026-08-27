use super::*;
use std::time::Instant;

impl Model {
    pub(super) fn handle_terminal_message(
        &mut self,
        msg: Msg,
        focused: Option<&ComponentId>,
        music_resize: &mut bool,
        tv_resize: &mut bool,
    ) -> bool {
        let mut quit = false;
        match msg {
            Msg::TerminalEvent(event) => {
                apply_terminal_observer(self, event, focused, music_resize, tv_resize, &mut quit)
            }
            // Media surfaces forward unmatched keys through this
            // typed adapter so App retains its global shortcuts.
            Msg::Shell(ShellRequest::GlobalViewKey(key)) => {
                if self.handle_legacy_key(key) {
                    quit = true;
                }
            }
            Msg::Shell(ShellRequest::MusicAlbumCursor { target, kind }) => {
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
                                    self.app.maybe_fetch_next_page(lib_idx);
                                }
                            }
                        }
                        AlbumCursorKind::Jump => {
                            if self.app.jump_music_group_display_cursor(lib_idx, target) {
                                self.app.save_default_library_position(lib_idx);
                                self.app.maybe_fetch_next_page(lib_idx);
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
            Msg::Shell(ShellRequest::MusicTrackActivate) => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    if let Some((album_id, track)) = self.focused_music_track(lib_idx) {
                        self.app.play_album_track(&album_id, &track);
                    }
                }
                self.push_music_workspace_content();
            }
            Msg::Shell(ShellRequest::MusicTrackEnqueue) => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    if let Some((_, track)) = self.focused_music_track(lib_idx) {
                        self.app.enqueue_lib_item(lib_idx, track);
                    }
                }
                self.push_music_workspace_content();
            }
            Msg::Shell(ShellRequest::MusicTrackContextMenu) => {
                if let Some((_, track)) = self
                    .app
                    .tab
                    .emby_library_index()
                    .and_then(|lib_idx| self.focused_music_track(lib_idx))
                {
                    self.app.open_context_menu_for(track);
                }
                self.push_music_workspace_content();
            }
            // Help overlay cross-boundary requests (design D4).
            Msg::Shell(ShellRequest::Quit) => quit = true,
            Msg::Shell(ShellRequest::DismissHelp) => self.umount_help(),
            Msg::Shell(ShellRequest::OpenSettings) => {
                self.umount_help();
                self.mount_sidebar(super::super::SidebarId::Settings);
            }
            Msg::Shell(ShellRequest::OpenSessions) => {
                self.umount_help();
                self.mount_sidebar(super::super::SidebarId::Sessions);
            }
            Msg::Shell(ShellRequest::OpenPlaylists) => {
                self.umount_help();
                self.mount_sidebar(super::super::SidebarId::Playlists);
                self.app.open_playlists_panel();
            }
            Msg::Shell(ShellRequest::ConfirmKey(key)) => {
                self.handle_confirm_key(key);
                // Confirmations rewrite Home content/focus; re-project (5.3d).
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            Msg::Shell(ShellRequest::DaemonLostKey(key)) => {
                if self.handle_daemon_lost_key(key) {
                    quit = true;
                }
            }
            Msg::Shell(ShellRequest::RemoteReanchorKey(key)) => {
                self.handle_remote_reanchor_key(key);
            }
            // Context menu: the shell owns cursor navigation and
            // action execution; the component owns the key/click
            // forwarding (task 5.3c).
            Msg::Shell(ShellRequest::ContextMenuKey(key)) => {
                self.handle_context_menu_key(key);
                // Enter executes the action, which can refetch Home; re-project (5.3d).
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            Msg::Shell(ShellRequest::ContextMenuSelect(idx)) => {
                self.handle_context_menu_select(idx);
                // A selected action can refetch Home; re-project (5.3d).
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            Msg::Shell(ShellRequest::ContextMenuDismiss) => {
                self.app.pending_overlay =
                    Some(super::super::types_overlay::OverlayRequest::DismissContextMenu);
            }
            // Search sidebar: dismiss (Esc/Backspace-on-empty).
            // The component owns the state; the shell unmounts it.
            Msg::Shell(ShellRequest::DismissSearch) => {
                self.dismiss_sidebar(super::super::SidebarId::Search);
            }
            // Search sidebar: activate result (Enter). The
            // component owns the cursor/results; the shell owns
            // the library tabs and navigation spawn (task 3.2).
            Msg::Shell(ShellRequest::SearchActivate { id, item_type }) => {
                self.app.activate_search_result(id, item_type);
            }
            Msg::Shell(ShellRequest::OpenInlineSearch) => {
                self.open_inline_search();
            }
            Msg::Shell(ShellRequest::InlineSearchDismiss) => {
                self.dismiss_inline_search();
            }
            Msg::Shell(ShellRequest::InlineSearchActivate { id, item_type }) => {
                self.activate_inline_search_item(id, item_type);
            }
            Msg::Shell(ShellRequest::DismissSessions) => {
                self.dismiss_sidebar(super::super::SidebarId::Sessions);
            }
            Msg::Shell(ShellRequest::RefreshSessions) => {
                self.app.spawn_sessions_load();
                self.app.spawn_cast_discovery();
            }
            Msg::Shell(ShellRequest::SelectSession(index)) => {
                if let Some(target) = self.app.panel_targets.get(index).cloned() {
                    self.app.select_panel_target(target);
                }
            }
            Msg::Shell(ShellRequest::DetachSessions) => {
                self.app.disconnect_remote();
                if self.app.is_cast_attached() {
                    self.app.detach_cast();
                    self.app.flash(
                        "Detached from cast target".to_string(),
                        ToastSeverity::Success,
                    );
                }
                self.dismiss_sidebar(super::super::SidebarId::Sessions);
            }
            Msg::Shell(ShellRequest::RefreshFeeds) => {
                self.app.refresh_feeds();
            }
            Msg::Shell(ShellRequest::FeedsPlay(guid)) => {
                self.app.feed_tab_play_guid(&guid);
            }
            Msg::Shell(ShellRequest::FeedsEnqueue(guid)) => {
                self.app.feed_tab_enqueue_guid(&guid);
            }
            Msg::Shell(request @ ShellRequest::DismissSelectionModal)
            | Msg::Shell(request @ ShellRequest::SelectionModalFilterSelected)
            | Msg::Shell(request @ ShellRequest::SelectionModalActivate(_)) => {
                self.handle_selection_modal_request(request);
                // Selection-modal changes to the ABS episode filter
                // must reach the mounted component (5.3d.11 U6).
                self.push_audiobookshelf_podcast_content();
            }
            Msg::Shell(ShellRequest::MultiselectCommit { .. }) => {
                self.handle_multiselect_commit();
                // Hiding libraries/pills refetches Home inside the commit; re-project (5.3d).
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            Msg::Shell(request @ ShellRequest::LibraryRoutesEnter)
            | Msg::Shell(request @ ShellRequest::LibraryRoutesEsc) => {
                self.handle_library_routes_request(request);
            }
            Msg::Shell(ShellRequest::FeedsManageKey(key)) => {
                self.handle_feeds_manage_request(key);
            }
            Msg::Shell(ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent)) => {
                // Typed podcast episode action intent (task 5.3d.7).
                // The shell resolves the episode-selection and
                // wide/narrow conditions from App state/layout and
                // runs the existing App play/enter/modal/enqueue
                // effects (D17); re-project after the effect.
                self.handle_audiobookshelf_podcast_episode_intent(intent);
                self.push_audiobookshelf_podcast_content();
            }
            Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove(movement)) => {
                // Typed podcast show-list movement (task 5.3d.5). The
                // component already mutated its local cursor; map onto
                // the legacy App show-move operations so the painted
                // cursor and the position-save/detail-fetch target
                // both stay unchanged (D17). Compute the page size
                // before the move call to avoid a borrow conflict.
                match movement {
                    PodcastShowMove::PreviousRow => {
                        self.app.move_audiobookshelf_show_rows(-1);
                    }
                    PodcastShowMove::NextRow => {
                        self.app.move_audiobookshelf_show_rows(1);
                    }
                    PodcastShowMove::PreviousItem => {
                        self.app.move_audiobookshelf_show_cursor(-1);
                    }
                    PodcastShowMove::NextItem => {
                        self.app.move_audiobookshelf_show_cursor(1);
                    }
                    PodcastShowMove::PreviousPage => {
                        let page = self.app.lib_page_size() as i64;
                        self.app.move_audiobookshelf_show_rows(-page);
                    }
                    PodcastShowMove::NextPage => {
                        let page = self.app.lib_page_size() as i64;
                        self.app.move_audiobookshelf_show_rows(page);
                    }
                    PodcastShowMove::First => {
                        self.app.jump_audiobookshelf_show_cursor(false);
                    }
                    PodcastShowMove::Last => {
                        self.app.jump_audiobookshelf_show_cursor(true);
                    }
                }
                // The component owns the painted cursor; persist the
                // active tab's slot once after any movement lands so
                // the saved position tracks the moved cursor (B3).
                if let Some(index) = self.app.tab.audiobookshelf_index() {
                    self.app.save_audiobookshelf_position(index);
                }
                // The App move ops above rewrote the active browse
                // state (cursor/selection); re-project (5.3d.11 U6).
                self.push_audiobookshelf_podcast_content();
            }
            Msg::Shell(ShellRequest::AudiobookshelfBookMove(movement)) => {
                self.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookMove(
                    movement,
                ));
            }
            Msg::Shell(ShellRequest::AudiobookshelfBookIntent(intent)) => {
                self.handle_audiobookshelf_book_request(ShellRequest::AudiobookshelfBookIntent(
                    intent,
                ));
            }
            // Browser (generic Emby) mouse geometry lives in
            // `BrowserComponent`, which forwards the hit region; the
            // shell decides *when* it counts via `App`'s 400ms
            // double-click / 30ms wheel fields (task 5.3d, correction to b5799185).
            Msg::Shell(ShellRequest::BrowserScroll { delta }) => {
                if self.app.note_browse_scroll() {
                    self.app.handle_mouse_scroll_browse(delta);
                }
            }
            // Browser selected-item typed effects (task 5.3d, Emby
            // browser effect decoupling): the component reports the
            // explicit `EmbyItem` target; the shell forwards it
            // straight to the App effect (no App-cursor re-read).
            Msg::Shell(
                request @ (ShellRequest::BrowserActivate { .. }
                | ShellRequest::BrowserPlay { .. }
                | ShellRequest::BrowserEnqueue { .. }
                | ShellRequest::BrowserToggleWatched { .. }
                | ShellRequest::BrowserContextMenu { .. }
                | ShellRequest::BrowserShuffle { .. }
                | ShellRequest::BrowserRefresh
                | ShellRequest::BrowserRescan
                | ShellRequest::BrowserBack
                | ShellRequest::BrowserCycleLetterPill { .. }
                | ShellRequest::BrowserMoveRows { .. }
                | ShellRequest::BrowserMoveColumn { .. }
                | ShellRequest::BrowserJumpCursor { .. }),
            ) => {
                self.handle_browser_request(request);
                // Browser navigation/effects change library content; re-project (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            Msg::Shell(ShellRequest::BrowserClick { region, col, row }) => {
                match region {
                    BrowserHitRegion::SelectorTab(target) => {
                        self.app.last_click_time = Instant::now();
                        self.app.last_click_pos = (col, row);
                        if let Some(lib_idx) = self.app.tab.emby_library_index() {
                            self.app.handle_mouse_selector_click_emby(lib_idx, target);
                        }
                    }
                    BrowserHitRegion::ContextMenu(target) => {
                        if let Some(lib_idx) = self.app.tab.emby_library_index() {
                            self.app
                                .handle_mouse_right_click_emby(lib_idx, target, col, row);
                        }
                    }
                    BrowserHitRegion::LeftRow(target) | BrowserHitRegion::InlineHero(target) => {
                        if self.app.note_browse_double_click(col, row) {
                            if let Some(lib_idx) = self.app.tab.emby_library_index() {
                                self.app.handle_mouse_double_click_emby(lib_idx, target);
                            }
                        } else {
                            if let Some(lib_idx) = self.app.tab.emby_library_index() {
                                self.app.handle_mouse_single_click_emby(lib_idx, target);
                            }
                        }
                    }
                }
                // Selector-tab / item clicks mutate library state; re-project (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            // Home (cross-Service) mouse geometry lives in
            // `HomeComponent`, which forwards the hit region; the
            // shell decides *when* it counts via `App`'s 400ms
            // double-click / 30ms wheel fields (task 5.3d, home
            // hit_test). Accepted wheel scroll is routed at the
            // Model boundary, which moves the mounted component's
            // section-local cursor and, as a preserved legacy
            // quirk, the Continue Watching column's independent
            // cursor (task 5.3d, Home wheel-scroll ownership).
            Msg::Shell(ShellRequest::HomeScroll { delta }) => {
                self.handle_home_scroll(delta);
            }
            Msg::Shell(ShellRequest::HomeClick { region, col, row }) => {
                self.handle_home_click(region, col, row);
            }
            // Home typed effects (task 5.3d, Home typed-effect
            // prep): `HomeComponent` owns the cursor and reports the
            // flat target index it resolved; the shell forwards it
            // straight to the `App` effect so the requested target
            // is acted on directly (no App-owned flat cursor remains).
            Msg::Shell(
                request @ (ShellRequest::HomePlay(_)
                | ShellRequest::HomeEnqueue(_)
                | ShellRequest::HomeDelete(_)
                | ShellRequest::HomeToggleWatched
                | ShellRequest::HomeSectionSelected(_)),
            ) => self.handle_home_request(request),
            // Queue mouse geometry lives in `QueueComponent`;
            // the shell decides *when* a row click is a double-click
            // and shares App's 30ms wheel throttle with browse/home.
            Msg::Shell(ShellRequest::QueueScroll { delta }) => {
                if self.app.note_browse_scroll() {
                    self.app.handle_mouse_scroll_queue(delta);
                }
            }
            Msg::Shell(ShellRequest::QueueClick { region, col, row }) => {
                match region {
                    QueueHitRegion::ScopeLocal => {
                        self.app.last_click_time = Instant::now();
                        self.app.last_click_pos = (col, row);
                        self.app
                            .handle_mouse_selector_click_queue(QueueScope::Local);
                    }
                    QueueHitRegion::ScopeRemote => {
                        self.app.last_click_time = Instant::now();
                        self.app.last_click_pos = (col, row);
                        self.app
                            .handle_mouse_selector_click_queue(QueueScope::Remote);
                    }
                    QueueHitRegion::ContextMenu(slot_id) => {
                        // The authoritative Continue-Watching-selected
                        // fact is resolved here (Model boundary) and
                        // passed into the App builder, so the odd
                        // queue→Home coupling reflects the mounted
                        // Home component's section (task 5.3d).
                        self.app.handle_mouse_right_click_queue(
                            slot_id,
                            col,
                            row,
                            self.home_continue_watching_selected(),
                        );
                    }
                    QueueHitRegion::Row(slot_id) => {
                        if self.app.note_browse_double_click(col, row) {
                            self.app.handle_mouse_double_click_queue(slot_id);
                        } else {
                            self.app.handle_mouse_single_click_queue(slot_id);
                        }
                    }
                }
                // Queue clicks move panel focus to the Queue panel;
                // re-project the Home focus flag (task 5.3d, sync_home deletion).
                self.push_home_content();
                // Emby browser content may have changed (5.3d.15/M2).
                self.push_emby_browser_content();
            }
            // TV keyboard requests are resolved by the mounted
            // workspace component. Cursor and pane movement remain
            // component-local; the shell handles only cross-boundary
            // effects such as activation, back, and letter pills.
            Msg::Shell(
                request @ (ShellRequest::TvMoveRows { .. }
                | ShellRequest::TvMoveColumn { .. }
                | ShellRequest::TvJumpCursor { .. }
                | ShellRequest::TvActivate
                | ShellRequest::TvEpisodeActivate
                | ShellRequest::TvBack
                | ShellRequest::TvCycleLetterPill { .. }
                | ShellRequest::TvEpisodeMove { .. }
                | ShellRequest::TvSeasonMove { .. }),
            ) => self.handle_tv_request(request),
            // TV workspace mouse geometry lives in
            // `TvWorkspaceComponent`, which resolves the pane +
            // hit (two focusable panes); the shell decides *when*
            // a click is a double-click via App's 400ms window
            // and shares the 30ms wheel throttle (task 5.3d,
            // tv_workspace hit_test).
            Msg::Shell(ShellRequest::TvScroll { delta }) => {
                if self.app.note_browse_scroll() {
                    self.app.handle_mouse_scroll_browse(delta);
                }
                self.push_tv_workspace_content();
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    self.mirror_tv_workspace_cursor(lib_idx);
                }
            }
            Msg::Shell(ShellRequest::TvClick { region, col, row }) => {
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    match region {
                        TvHitRegion::ContextMenu(hit) => {
                            self.app.handle_mouse_right_click_tv(lib_idx, hit, col, row);
                        }
                        TvHitRegion::Hit(hit) => {
                            if self.app.note_browse_double_click(col, row) {
                                self.app.handle_mouse_double_click_tv(lib_idx, hit);
                            } else {
                                self.app.handle_mouse_single_click_tv(lib_idx, hit);
                            }
                        }
                    }
                }
                self.push_tv_workspace_content();
                if let Some(lib_idx) = self.app.tab.emby_library_index() {
                    self.mirror_tv_workspace_cursor(lib_idx);
                }
            }
            Msg::Shell(
                request @ (ShellRequest::PlaylistsBack
                | ShellRequest::PlaylistsOpen(_)
                | ShellRequest::PlaylistsActivate { .. }
                | ShellRequest::PlaylistsRename(_)
                | ShellRequest::PlaylistsDelete(_)
                | ShellRequest::PlaylistsRefresh
                | ShellRequest::DismissPlaylists),
            ) => self.handle_playlists_request(request),
            Msg::Shell(ShellRequest::DismissSettings) => self.app.close_settings(),
            Msg::Shell(ShellRequest::SavePlaylistKey(key)) => {
                self.handle_save_playlist_key(key);
            }
            Msg::Shell(ShellRequest::QueueKey(key)) => {
                if self.app.handle_queue_key(key) {
                    quit = true;
                }
            }
            Msg::Queue(request) => {
                self.handle_queue_request(request);
            }
            Msg::Playback(request) => {
                self.handle_playback_request(request);
            }
            Msg::Shell(ShellRequest::PlaybackPromptKey(key)) => {
                if self.app.skip_intro_end_ticks.is_some() {
                    self.app.handle_key_confirm_skip_intro(key, false, None);
                } else if self.app.next_up_item.is_some() {
                    self.app.handle_key_confirm_next_up(key, false, None);
                }
            }
            Msg::Service(request) => {
                if self.handle_service_request(request) {
                    quit = true;
                }
            }
            Msg::Persist(request) => {
                if self.handle_persist_request(request) {
                    quit = true;
                }
            }
            // No other Msg variants are produced yet.
            _ => {}
        }
        quit
    }
}
