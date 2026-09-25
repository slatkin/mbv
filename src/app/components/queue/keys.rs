use tuirealm::event::{Key, KeyEvent};

use crate::app::components::media_list::{MediaListSurfaceInput, RowIntent};
use crate::app::components::msg::{
    LeafKeyResult, Msg, QueueColumnResize, QueueIntent, QueueMove, QueueRequest, ShellRequest,
};
use crate::app::state::types::context_menu::ContextMenuTargets;
use crate::app::state::types::playback::QueueScope;

use super::QueueComponent;

impl QueueComponent {
    fn move_cursor(&mut self, delta: i64) -> Option<Msg> {
        let outcome = self.delegate_row_local_input(MediaListSurfaceInput::Move(delta), None);
        if outcome.selected_target.is_some() {
            self.cursor_message()
        } else {
            None
        }
    }

    pub(super) fn handle_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if matches!(
                key.code,
                Key::Up
                    | Key::Down
                    | Key::PageUp
                    | Key::PageDown
                    | Key::Home
                    | Key::End
                    | Key::Left
                    | Key::Right
                    | Key::Enter
                    | Key::Delete
                    | Key::Char('[' | ']' | '.' | 'i' | 'p' | 's' | 'c' | 'z')
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            ))));
        }
        if let Some(message) = self.handle_scope_key(key) {
            return Some(message);
        }
        if let Some(message) = self.handle_resize_key(key) {
            return Some(message);
        }
        match key.code {
            Key::Up | Key::Down => self.handle_vertical_key(key),
            Key::PageUp | Key::PageDown | Key::Home | Key::End => self.handle_navigation_key(key),
            Key::Enter => self.activate_selected_row(),
            Key::Delete => self.delete_selection(),
            Key::Char('z')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                Some(Msg::Queue(QueueRequest::Undo { scope: self.scope }))
            }
            Key::Char('.') if key.modifiers.is_empty() => self.open_context_menu(),
            Key::Char('i') => self.navigate_to_selection(),
            Key::Char('p') => Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                QueueIntent::PlayNow,
            )))),
            Key::Char('s')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::SavePlaylist,
                ))))
            }
            Key::Char('c') if !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) => Some(
                Msg::Shell(Box::new(ShellRequest::QueueIntent(QueueIntent::Clear))),
            ),
            _ => None,
        }
    }

    fn handle_scope_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        let scope = match key.code {
            Key::Char('[' | ']')
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                if key.code == Key::Char('[') {
                    QueueScope::Local
                } else {
                    QueueScope::Remote
                }
            }
            _ => return None,
        };
        self.scope = scope;
        // Scope is preassigned before the shell request, so set_content will not reset scroll.
        self.carrier.set_scroll(0);
        Some(Msg::Queue(QueueRequest::Scope(self.scope)))
    }

    fn handle_resize_key(&self, key: &KeyEvent) -> Option<Msg> {
        if !matches!(key.code, Key::Left | Key::Right)
            || key.modifiers != tuirealm::event::KeyModifiers::SHIFT
        {
            return None;
        }
        let resize = if key.code == Key::Left {
            QueueColumnResize::Narrower
        } else {
            QueueColumnResize::Wider
        };
        Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
            QueueIntent::ResizeColumn(resize),
        ))))
    }

    fn handle_vertical_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.is_empty() {
            return match key.code {
                Key::Up => self.move_cursor(-1),
                Key::Down => self.move_cursor(1),
                _ => None,
            };
        }
        if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) {
            let direction = if key.code == Key::Up {
                QueueMove::Up
            } else {
                QueueMove::Down
            };
            return self.selected_slot().map(|(scope, slot_id)| {
                Msg::Queue(QueueRequest::Move {
                    scope,
                    slot_id,
                    direction,
                })
            });
        }
        None
    }

    fn handle_navigation_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if !key.modifiers.is_empty() {
            return None;
        }
        match key.code {
            Key::PageUp => {
                self.move_cursor(-(self.content_area.height.saturating_sub(1).max(1) as i64))
            }
            Key::PageDown => {
                self.move_cursor(self.content_area.height.saturating_sub(1).max(1) as i64)
            }
            Key::Home => {
                self.delegate_row_local_input(MediaListSurfaceInput::First, None);
                self.cursor_message()
            }
            Key::End => {
                self.delegate_row_local_input(MediaListSurfaceInput::Last, None);
                self.cursor_message()
            }
            _ => None,
        }
    }

    fn activate_selected_row(&mut self) -> Option<Msg> {
        match self
            .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
            .external_intent
        {
            Some(RowIntent::Activate(slot_id)) => Some(Msg::Queue(QueueRequest::Play {
                scope: self.scope,
                slot_id,
            })),
            _ => None,
        }
    }

    fn delete_selection(&mut self) -> Option<Msg> {
        if !self.carrier.multi_selection().is_empty() {
            let slot_ids = self.carrier.multi_selection().to_vec();
            self.carrier.clear_owner_selection();
            return Some(Msg::Queue(QueueRequest::RemoveSelection {
                scope: self.scope,
                slot_ids,
            }));
        }
        self.selected_slot()
            .map(|(scope, slot_id)| Msg::Queue(QueueRequest::Remove { scope, slot_id }))
    }

    fn open_context_menu(&mut self) -> Option<Msg> {
        // `.` is a selection-dependent chord owned by the focused component.
        match self
            .delegate_row_local_input(MediaListSurfaceInput::Context, None)
            .external_intent
        {
            Some(RowIntent::Context(slot_id)) => Some(Msg::Shell(Box::new(
                ShellRequest::RowContextMenu(ContextMenuTargets::Queue(vec![slot_id]), None),
            ))),
            Some(RowIntent::ContextSelection(slot_ids)) => Some(Msg::Shell(Box::new(
                ShellRequest::RowContextMenu(ContextMenuTargets::Queue(slot_ids), None),
            ))),
            _ => Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                ContextMenuTargets::Queue(vec![]),
                None,
            )))),
        }
    }

    fn navigate_to_selection(&self) -> Option<Msg> {
        self.selected_slot().map(|(scope, slot_id)| {
            Msg::Shell(Box::new(ShellRequest::QueueIntent(QueueIntent::Navigate {
                scope,
                slot_id,
            })))
        })
    }
}
