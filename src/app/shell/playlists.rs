use super::Model;
use crate::app::state::playback::{PendingQueueAction, ReplacementExecutor};
use mbv_components::{PlaylistsComponent, PlaylistsContent};
use mbv_queue::{QueueItem, QueueSource};
use mbv_ui_msg::{ComponentId, ModalId, MusicTreeAction, OverlayId, ShellRequest};
use rand::seq::SliceRandom;

impl Model {
    pub(in crate::app) fn update_playlists_content(&mut self) {
        let id = ComponentId::Overlay(OverlayId::Playlists);
        if !self.application.mounted(&id) {
            return;
        }
        let panel = crate::app::shell::chrome_panels::sync_panel_area(&self.app);
        if let Some(comp) = self.application.get_component_mut(&id)
            && let Some(playlists) = comp.as_any_mut().downcast_mut::<PlaylistsComponent>()
        {
            playlists.set_content(PlaylistsContent {
                playlists: self.app.playlists.clone(),
                cursor: self.app.playlists_cursor,
                scroll: self.app.playlists_scroll,
                loading: self.app.playlists_loading,
                open: self.app.playlists_open.clone(),
                open_items: self.app.playlists_open_items.clone(),
                open_cursor: self.app.playlists_open_cursor,
                open_scroll: self.app.playlists_open_scroll,
                open_loading: self.app.playlists_open_loading,
                loaded_id: match self.app.playback_queue().source() {
                    mbv_queue::QueueSource::Playlist { id: Some(id), .. } => Some(id.clone()),
                    _ => None,
                },
            });
            playlists.set_panel_area(panel);
        }
    }

    pub(in crate::app) fn render_playlists_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Overlay(OverlayId::Playlists);
        if !self.application.mounted(&id) {
            return;
        }
        self.application.view(&id, frame, frame.area());
    }

    pub(in crate::app) fn render_save_playlist_overlay(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Modal(ModalId::SavePlaylist);
        if self.application.mounted(&id) {
            self.application.view(&id, frame, frame.area());
        }
    }

    pub(in crate::app) fn handle_playlists_request(&mut self, request: &ShellRequest) {
        match request {
            ShellRequest::PlaylistsBack => {
                self.app.playlists_open = None;
                self.app.playlists_open_items.clear();
            }
            ShellRequest::PlaylistsOpen(index) => {
                if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app.spawn_open_playlist(&playlist);
                }
            }
            ShellRequest::PlaylistsAction {
                open,
                index,
                action,
            } => self.handle_playlists_action(*open, *index, *action),
            ShellRequest::PlaylistsRename(index) => {
                if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app
                        .open_save_playlist_dialog(crate::app::SavePlaylistDialog {
                            input: playlist.name,
                            stage: crate::app::SavePlaylistStage::RenamePlaylist {
                                id: playlist.id,
                            },
                        });
                }
            }
            ShellRequest::PlaylistsDelete(index) => {
                if let Some(playlist) = self.app.playlists.get(*index).cloned() {
                    self.app.ask_confirm(crate::app::ConfirmModal::two_button(
                        format!(
                            "Delete playlist '{}'?",
                            mbv_ui_model::ui_util::trunc_str(&playlist.name, 40)
                        ),
                        "Confirm",
                        "Cancel",
                        crate::app::ConfirmAction::DeletePlaylist {
                            id: playlist.id,
                            name: playlist.name,
                        },
                    ));
                }
            }
            ShellRequest::PlaylistsRefresh => {
                if let Some(playlist) = self.app.playlists_open.clone() {
                    self.app.playlists_open = None;
                    self.app.playlists_open_items.clear();
                    self.app.spawn_open_playlist(&playlist);
                } else {
                    self.app.spawn_load_playlists();
                }
            }
            ShellRequest::DismissPlaylists => {
                self.dismiss_sidebar(super::SidebarId::Playlists);
            }
            // unreachable: shell/messages.rs routes only the Playlists* group
            // (Back/Open/Action/Rename/Delete/Refresh/DismissPlaylists) here;
            // every one has an arm above.
            _ => {}
        }
    }

    /// Resolves one Enter/`s`/`a` on the Playlists panel (design D2). Play and
    /// Shuffle replace the queue (gated by the populated-queue confirmation);
    /// Enqueue appends once and leaves the sidebar open.
    fn handle_playlists_action(&mut self, open: bool, index: usize, action: MusicTreeAction) {
        // `cursor` is the open-view item under the cursor; `None` in the list view.
        let (playlist_id, name, items, cursor) = if open {
            let Some(cursor) = self.app.playlists_open_items.get(index).cloned() else {
                return;
            };
            let Some(playlist) = self.app.playlists_open.as_ref() else {
                return;
            };
            let items: Vec<_> = self
                .app
                .playlists_open_items
                .iter()
                .filter(|item| !item.is_folder)
                .cloned()
                .collect();
            (
                playlist.id.clone(),
                playlist.name.clone(),
                items,
                Some(cursor),
            )
        } else {
            let Some(playlist) = self.app.playlists.get(index).cloned() else {
                return;
            };
            let Some(items) = self.app.playlist_playable_items(&playlist.id) else {
                return;
            };
            (playlist.id, playlist.name, items, None)
        };
        if items.is_empty() {
            return;
        }
        match action {
            MusicTreeAction::Play => {
                let start_idx = cursor
                    .and_then(|cursor| items.iter().position(|item| item.id == cursor.id))
                    .unwrap_or(0);
                self.app.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx,
                        source: QueueSource::Playlist {
                            id: Some(playlist_id),
                            name,
                        },
                    },
                    ReplacementExecutor::PlaylistsSidebar,
                );
            }
            MusicTreeAction::Shuffle => {
                let mut items = items;
                items.shuffle(&mut rand::rng());
                self.app.request_queue_replacement(
                    PendingQueueAction::PlayItems {
                        items,
                        start_idx: 0,
                        source: QueueSource::Shuffle,
                    },
                    ReplacementExecutor::PlaylistsSidebar,
                );
            }
            MusicTreeAction::Enqueue => {
                let items = match cursor {
                    Some(cursor) if cursor.is_folder => return,
                    Some(cursor) => vec![cursor],
                    None => items,
                };
                let items = items
                    .into_iter()
                    .map(|item| QueueItem::Emby(Box::new(item)))
                    .collect();
                let scope = self.app.viewed_queue_scope();
                self.app.append_on_owner(scope, items);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_local_daemon_app_stub_with_cmd_rx};
    use mbv_emby_model::test_support::make_item;
    use mbv_ui_msg::{Msg, TerminalObserverEvent};
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn playlists_shell_mounts_and_routes_component() {
        let mut app = make_app_stub();
        app.pending_overlay = Some(mbv_ui_model::overlay::OverlayRequest::OpenSidebar(
            mbv_ui_model::overlay::SidebarId::Playlists,
        ));
        let mut model = Model::new(app);
        model.sync_modal_requests();
        let id = ComponentId::Overlay(OverlayId::Playlists);
        let message = model
            .application
            .get_component_mut(&id)
            .expect("Playlists component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        assert!(matches!(
            message,
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
    }

    /// A model with playlist "pl" open over audio items a, b, c and an empty
    /// queue, plus the owner command channel.
    fn open_playlist_model() -> (Model, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
        let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(Vec::new());
        let mut playlist = make_item("Playlist", "Playlist");
        playlist.id = "pl".into();
        app.playlists_open = Some(playlist);
        app.playlists_open_items = ["a", "b", "c"]
            .into_iter()
            .map(|id| {
                let mut item = make_item(id, "Audio");
                item.id = id.into();
                item.media_type = "Audio".into();
                item
            })
            .collect();
        (Model::new(app), cmd_rx)
    }

    fn open_action(index: usize, action: MusicTreeAction) -> ShellRequest {
        ShellRequest::PlaylistsAction {
            open: true,
            index,
            action,
        }
    }

    /// Saved playlist order is untouched: Shuffle replaces the queue with
    /// every playlist item under `QueueSource::Shuffle`, never the playlist.
    #[test]
    fn shuffle_replaces_the_queue_with_a_shuffle_source() {
        let _guard = crate::config::TestStateDirGuard::new();
        let (mut model, cmd_rx) = open_playlist_model();

        model.handle_playlists_request(&open_action(1, MusicTreeAction::Shuffle));

        let replaces: Vec<_> = cmd_rx
            .try_iter()
            .filter_map(|command| match command {
                mbv_ctrl::CtrlCmd::UnifiedQueueReplace { items, source, .. } => {
                    Some((items.len(), source))
                }
                _ => None,
            })
            .collect();
        assert!(matches!(replaces.as_slice(), [(3, QueueSource::Shuffle)]));
    }

    /// Enqueue one item from an open playlist: one append holding only the
    /// cursor item, and no replace.
    #[test]
    fn open_view_enqueue_appends_only_the_cursor_item() {
        let _guard = crate::config::TestStateDirGuard::new();
        let (mut model, cmd_rx) = open_playlist_model();

        model.handle_playlists_request(&open_action(1, MusicTreeAction::Enqueue));

        let commands: Vec<_> = cmd_rx.try_iter().collect();
        let appended: Vec<Vec<String>> = commands
            .iter()
            .filter_map(|command| match command {
                mbv_ctrl::CtrlCmd::UnifiedQueueAppend { items, .. } => {
                    Some(items.iter().map(|item| item.id().to_string()).collect())
                }
                _ => None,
            })
            .collect();
        assert_eq!(appended, vec![vec!["b".to_string()]]);
        assert!(
            !commands
                .iter()
                .any(|command| matches!(command, mbv_ctrl::CtrlCmd::UnifiedQueueReplace { .. }))
        );
    }
}
