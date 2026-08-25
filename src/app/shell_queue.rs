use super::components::{ComponentId, QueueComponent, QueueMove, QueueRequest};
use super::shell::Model;
use super::{PanelFocus, QueueScope};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
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
                    self.app
                        .handle_queue_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
            QueueRequest::Remove { scope, slot_id } => {
                if self.select_queue_slot(scope, slot_id) {
                    self.app
                        .handle_queue_key(KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
                }
            }
            QueueRequest::Move {
                scope,
                slot_id,
                direction,
            } => {
                if self.select_queue_slot(scope, slot_id) {
                    let key = match direction {
                        QueueMove::Up => KeyCode::Up,
                        QueueMove::Down => KeyCode::Down,
                    };
                    self.app
                        .handle_queue_key(KeyEvent::new(key, KeyModifiers::SHIFT));
                }
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
