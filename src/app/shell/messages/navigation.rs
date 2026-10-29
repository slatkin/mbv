use super::{DestinationLatestSource, ShellRequest, ToastSeverity};
use crate::app::state::queue_transfer::QueueTransferDirection;

impl super::super::Model {
    pub(super) fn handle_navigation_request(
        &mut self,
        request: ShellRequest,
        quit: &mut bool,
    ) -> Option<ShellRequest> {
        match request {
            request @ (ShellRequest::Quit
            | ShellRequest::TabSelect(_)
            | ShellRequest::DismissHelp
            | ShellRequest::OpenSettings
            | ShellRequest::OpenSessions
            | ShellRequest::OpenPlaylists
            | ShellRequest::ConfirmIntent(_)
            | ShellRequest::DaemonLostIntent(_)
            | ShellRequest::ContextMenuIntent(_)
            | ShellRequest::ContextMenuSelect(_)
            | ShellRequest::ContextMenuDismiss
            | ShellRequest::DismissSearch
            | ShellRequest::SearchActivate { .. }
            | ShellRequest::OpenInlineSearch
            | ShellRequest::InlineSearchQueryStarted
            | ShellRequest::InlineSearchActivate { .. }
            | ShellRequest::MultiselectCommit { .. }) => {
                self.handle_navigation_overlay(request, quit)
            }
            request @ (ShellRequest::DismissSessions
            | ShellRequest::RefreshSessions
            | ShellRequest::SelectSession(_)
            | ShellRequest::DetachSessions
            | ShellRequest::HandOffSession(_)
            | ShellRequest::TakeSession(_)) => {
                self.handle_sessions_navigation(request);
                None
            }
            request @ (ShellRequest::RefreshFeeds
            | ShellRequest::FeedsPlay(_)
            | ShellRequest::FeedsRowClick
            | ShellRequest::FeedsEnqueue(_)
            | ShellRequest::FeedsManageIntent(_)
            | ShellRequest::FeedsLatestSelected) => {
                self.handle_feeds_navigation(request);
                None
            }
            request @ (ShellRequest::AudiobookshelfPodcastEpisodeIntent(_)
            | ShellRequest::AudiobookshelfPodcastLatestSelected) => {
                self.handle_podcast_navigation(request);
                None
            }
            request @ (ShellRequest::LibraryRoutesEnter | ShellRequest::LibraryRoutesEsc) => {
                self.handle_library_routes_request(&request);
                None
            }
            _ => Some(request),
        }
    }

    fn handle_navigation_overlay(
        &mut self,
        request: ShellRequest,
        quit: &mut bool,
    ) -> Option<ShellRequest> {
        match request {
            // Help overlay cross-boundary requests (design D4).
            ShellRequest::Quit => *quit = true,
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
                self.toggle_sidebar(super::super::SidebarId::Playlists);
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
                    *quit = true;
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
                    Some(mbv_ui_model::overlay::OverlayRequest::DismissContextMenu);
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
                self.activate_inline_search_item(&id, &item_type);
            }
            ShellRequest::MultiselectCommit { .. } => {
                self.handle_multiselect_commit();
                // The committed visibility settings may change Emby browser content.
                // Re-project that destination owner.
                self.push_active_emby_library_owner_content();
            }
            _ => unreachable!("request is a navigation overlay request"),
        }
        None
    }

    fn handle_sessions_navigation(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::DismissSessions => {
                self.dismiss_sidebar(super::super::SidebarId::Sessions);
            }
            ShellRequest::RefreshSessions => {
                self.app.spawn_sessions_load();
                self.app.spawn_cast_discovery();
            }
            ShellRequest::SelectSession(key) => {
                if let Some(target) = crate::app::state::panel_targets::resolve_session_target(
                    &self.app.panel_targets,
                    &key,
                ) {
                    self.app.select_panel_target(target);
                }
            }
            ShellRequest::DetachSessions => {
                // The owner's output decides (design D13): when a cast target is
                // the output, detach sends `SelectPlaybackOutput(Local)` and the
                // owner tears the receiver app down (D12). An Emby Session output
                // detaches the same way (emby-session-owner-output design D12).
                let cast_output = self.app.output_is_cast();
                let session_output = self.app.output_is_emby_session();
                let remote_output = cast_output || session_output;
                // Skip disconnect_remote's "No session selected" toast when a
                // remote output is the detach target; the output hand-back
                // below is the real action in that case.
                if self.app.can_disconnect_remote() || !remote_output {
                    self.app.disconnect_remote();
                }
                if remote_output {
                    self.app
                        .select_playback_output(mbv_ctrl::OutputSelection::Local);
                    let message = if cast_output {
                        "Detached from cast target"
                    } else {
                        "Detached from Emby Session"
                    };
                    self.app.flash(message.to_string(), ToastSeverity::Success);
                }
                self.dismiss_sidebar(super::super::SidebarId::Sessions);
            }
            // The Client-run Hand off sequence lands here in row 7.2 and
            // the Take sequence in row 8.1 (design D1/D2). D6: the shell
            // resolves the key against the current snapshot and does nothing
            // when the target is gone. Either action replaces any pending
            // transfer (design D2 cancel).
            ShellRequest::HandOffSession(key) => {
                self.begin_queue_transfer_action(QueueTransferDirection::HandOff, &key);
            }
            ShellRequest::TakeSession(key) => {
                self.begin_queue_transfer_action(QueueTransferDirection::Take, &key);
            }
            _ => unreachable!("request is a sessions navigation request"),
        }
    }

    /// Resolve an F3 transfer target (design D6) and start its action. A key
    /// the current snapshot no longer names does nothing.
    fn begin_queue_transfer_action(
        &mut self,
        direction: QueueTransferDirection,
        key: &mbv_ui_model::panel_targets::SessionTargetKey,
    ) {
        if crate::app::state::panel_targets::resolve_session_target(&self.app.panel_targets, key)
            .is_some()
        {
            self.app.begin_queue_transfer(direction, key);
        }
    }

    fn handle_feeds_navigation(&mut self, request: ShellRequest) {
        match request {
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
                // focus to the Library (task 4.5, mirrors `HomeRequest::RowClick`).
                self.app
                    .set_panel_focus(mbv_ui_model::settings::PanelFocus::Library);
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
            ShellRequest::FeedsManageIntent(intent) => {
                self.handle_feeds_manage_intent(intent);
            }
            ShellRequest::FeedsLatestSelected => {
                self.record_home_latest_acknowledgement(DestinationLatestSource::Feeds);
                self.sync_feeds();
            }
            _ => unreachable!("request is a feeds navigation request"),
        }
    }

    fn handle_podcast_navigation(&mut self, request: ShellRequest) {
        match request {
            ShellRequest::AudiobookshelfPodcastEpisodeIntent(intent) => {
                // Typed podcast episode action intent (task 5.3d.7).
                // The shell resolves the episode-selection and
                // wide/narrow conditions from App state/layout and
                // runs the existing App play/enter/modal/enqueue
                // effects (D17); re-project after the effect.
                self.handle_audiobookshelf_podcast_episode_intent(intent);
                self.push_audiobookshelf_podcast_content();
            }
            ShellRequest::AudiobookshelfPodcastLatestSelected => {
                let library_id = self
                    .app
                    .tab
                    .audiobookshelf_index()
                    .and_then(|index| self.app.audiobookshelf_libraries.get(index))
                    .map(|library| library.id.clone());
                if let Some(library_id) = library_id {
                    self.select_audiobookshelf_latest(&library_id);
                }
                self.push_audiobookshelf_podcast_content();
            }
            _ => unreachable!("request is a podcast navigation request"),
        }
    }
}
