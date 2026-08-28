use super::components::{
    ComponentId, QueueColumnResize, QueueComponent, QueueIntent, QueueMove, QueueRequest,
};
use super::shell::Model;
use super::{PanelFocus, QueueScope};
use crate::app::notify_actions::ToastSeverity;
use ratatui::layout::Rect;

impl Model {
    pub(super) fn sync_queue(&mut self) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            self.application
                .mount(id.clone(), Box::new(QueueComponent::new()), vec![])
                .expect("mount Queue");
        }

        let queue_focused = matches!(self.app.effective_panel_focus(), PanelFocus::Queue)
            && !self.blocking_overlay_active();
        if queue_focused {
            if self.application.focus() != Some(&id) {
                self.application.active(&id).expect("activate Queue");
            }
        } else if self.application.focus() == Some(&id) {
            self.application.blur().expect("blur Queue");
        }

        let scope = self.app.visible_queue_scope();
        let (slots, cursor, scroll) = {
            let queue = self.app.queue_for_scope(scope);
            (
                queue.slots().to_vec(),
                queue.queue_cursor,
                self.app.queue_scroll,
            )
        };
        let playback = self.app.displayed_queue_playback_state();
        let title = self.app.queue_title_model();
        let title_area = (self.app.layout.main.queue_scope_local_area.height > 0).then(|| {
            let area = self.app.layout.main.queue_area;
            Rect {
                x: area.x + 2,
                y: area.y.saturating_sub(2),
                width: area.width.saturating_sub(4),
                height: 1,
            }
        });
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>() {
                queue.set_content(slots, cursor, scroll, scope, queue_focused, playback, title);
                queue.set_area(self.app.layout.main.queue_area);
                queue.set_title_area(title_area);
            }
        }
    }

    pub(super) fn render_queue_component(&mut self, frame: &mut ratatui::Frame) {
        let id = ComponentId::Queue;
        if !self.application.mounted(&id) {
            return;
        }
        if let Some(comp) = self.application.get_component_mut(&id) {
            if let Some(queue) = comp.as_any_mut().downcast_mut::<QueueComponent>() {
                queue.set_area(self.app.layout.main.queue_area);
                let area = self.app.layout.main.queue_area;
                queue.set_title_area(
                    (self.app.layout.main.queue_scope_local_area.height > 0).then(|| Rect {
                        x: area.x + 2,
                        y: area.y.saturating_sub(2),
                        width: area.width.saturating_sub(4),
                        height: 1,
                    }),
                );
            }
        }
        self.application
            .view(&id, frame, self.app.layout.main.queue_area);
    }

    pub(super) fn handle_queue_request(&mut self, request: QueueRequest) {
        match request {
            QueueRequest::Scope(scope) => {
                if scope == QueueScope::Local || self.app.has_direct_remote_queue() {
                    self.app.set_queue_scope(scope);
                }
            }
            QueueRequest::Cursor { scope, slot_id } => {
                self.select_queue_slot(scope, slot_id);
            }
            QueueRequest::Play { scope, slot_id } => {
                if self.select_queue_slot(scope, slot_id) {
                    self.app.dispatch(super::action::Command::QueuePlayCursor);
                }
            }
            QueueRequest::Remove { scope, slot_id } => {
                if self.select_queue_slot(scope, slot_id) {
                    let cursor = self.app.queue_for_scope(scope).queue_cursor;
                    self.app.remove_from_queue(cursor);
                }
            }
            QueueRequest::Move {
                scope,
                slot_id,
                direction,
            } => {
                if self.select_queue_slot(scope, slot_id) {
                    match direction {
                        QueueMove::Up => self.app.move_queue_item_up(),
                        QueueMove::Down => self.app.move_queue_item_down(),
                    }
                }
            }
            QueueRequest::Undo { scope } => {
                if scope == QueueScope::Remote {
                    self.app.flash(
                        "Undo is not supported for remote queue edits".into(),
                        ToastSeverity::Error,
                    );
                } else {
                    self.app.undo_last_queue_edit(scope);
                }
            }
        }
    }

    pub(super) fn handle_queue_intent(&mut self, intent: QueueIntent) {
        match intent {
            QueueIntent::Clear => self.app.request_clear_queue(),
            QueueIntent::ResizeColumn(direction) => {
                if self.app.effective_panel_mode() == super::PanelMode::Both {
                    self.app
                        .resize_queue_column(direction == QueueColumnResize::Wider);
                }
            }
            QueueIntent::StopRemoteTracking => {
                if self.app.remote_tracker.is_some() {
                    self.app.stop_remote_tracking();
                }
            }
            QueueIntent::ReanchorRemoteTracking => {
                if self.app.remote_tracker.is_some() {
                    self.app.reanchor_remote_tracking();
                }
            }
            QueueIntent::PlayNow => {
                let (active, current_idx) = {
                    let status = self.app.player.status.lock().unwrap();
                    (status.active, status.current_idx)
                };
                if active {
                    self.app.playback_queue_mut().queue_cursor = current_idx;
                    if self.app.player.is_remote() {
                        self.app.set_queue_scope(QueueScope::Remote);
                    }
                } else {
                    self.app
                        .flash("Nothing is playing".into(), ToastSeverity::Error);
                }
            }
            QueueIntent::SavePlaylist => {
                if self.app.player_tab.total_queue_len() > 0 {
                    self.app
                        .open_save_playlist_dialog(super::SavePlaylistDialog {
                            input: self.app.queue_playlist_name().to_string(),
                            stage: super::SavePlaylistStage::EnterName,
                        });
                }
            }
            QueueIntent::Navigate { scope, slot_id } => {
                if !self.select_queue_slot(scope, slot_id) {
                    return;
                }
                let cursor = self.app.queue_for_scope(scope).queue_cursor;
                let Some(item) = self.app.queue_for_scope(scope).emby_item_at(cursor) else {
                    return;
                };
                let item_id = item.id.clone();
                let item_type = item.item_type.clone();
                let libs = self
                    .app
                    .libs
                    .iter()
                    .enumerate()
                    .map(|(i, lib)| {
                        (
                            i,
                            lib.library.id.clone(),
                            lib.library.collection_type.clone(),
                        )
                    })
                    .collect();
                self.app.spawn_navigate_to_item(item_id, item_type, libs);
            }
        }
    }

    fn select_queue_slot(
        &mut self,
        scope: QueueScope,
        slot_id: mbv_core::playback_queue::QueueSlotId,
    ) -> bool {
        if scope == QueueScope::Remote && !self.app.has_direct_remote_queue() {
            return false;
        }
        let Some(index) = self
            .app
            .queue_for_scope(scope)
            .slots()
            .iter()
            .position(|slot| slot.slot_id == slot_id)
        else {
            return false;
        };
        self.app.set_queue_scope(scope);
        self.app.set_panel_focus(PanelFocus::Queue);
        self.app.mark_queue_cursor_user_active();
        self.app.queue_for_scope_mut(scope).queue_cursor = index;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::{Msg, QueueRequest};
    use crate::app::tests::{make_app_stub, make_item};
    use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

    #[test]
    fn queue_shell_mounts_and_routes_slot_cursor() {
        let mut app = make_app_stub();
        app.player_tab.set_queue_items(
            vec![
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item("one", "Movie"))),
                mbv_core::playback_queue::QueueItem::Emby(Box::new(make_item("two", "Movie"))),
            ],
            0,
        );
        app.panel_focus = PanelFocus::Queue;
        let mut model = Model::new(app);
        model.sync_queue();
        let id = ComponentId::Queue;
        let message = model
            .application
            .get_component_mut(&id)
            .expect("Queue component mounted")
            .on(&Event::Keyboard(KeyEvent {
                code: Key::Down,
                modifiers: KeyModifiers::NONE,
            }));
        let Some(Msg::Queue(request @ QueueRequest::Cursor { .. })) = message else {
            panic!("queue navigation must emit a slot cursor request");
        };
        model.handle_queue_request(request);
        assert_eq!(model.app.player_tab.queue_cursor, 1);
    }
}
