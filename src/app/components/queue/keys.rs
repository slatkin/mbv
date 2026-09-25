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
        match key.code {
            Key::Char('[')
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.scope = QueueScope::Local;
                // Scope is preassigned here, before the request reaches the
                // shell, so the set_content scope-change reset would not fire;
                // the component resets its own scroll itself (D3).
                self.carrier.set_scroll(0);
                return Some(Msg::Queue(QueueRequest::Scope(self.scope)));
            }
            Key::Char(']')
                if !key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL)
                    && !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) =>
            {
                self.scope = QueueScope::Remote;
                // Scope is preassigned here, before the request reaches the
                // shell, so the set_content scope-change reset would not fire;
                // the component resets its own scroll itself (D3).
                self.carrier.set_scroll(0);
                return Some(Msg::Queue(QueueRequest::Scope(self.scope)));
            }
            Key::Left | Key::Right if key.modifiers == tuirealm::event::KeyModifiers::SHIFT => {
                return Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::ResizeColumn(if key.code == Key::Left {
                        QueueColumnResize::Narrower
                    } else {
                        QueueColumnResize::Wider
                    }),
                ))));
            }
            Key::Up if key.modifiers.is_empty() => {
                return self.move_cursor(-1);
            }
            Key::Down if key.modifiers.is_empty() => {
                return self.move_cursor(1);
            }
            Key::PageUp if key.modifiers.is_empty() => {
                return self
                    .move_cursor(-(self.content_area.height.saturating_sub(1).max(1) as i64));
            }
            Key::PageDown if key.modifiers.is_empty() => {
                return self.move_cursor(self.content_area.height.saturating_sub(1).max(1) as i64);
            }
            Key::Home if key.modifiers.is_empty() => {
                self.delegate_row_local_input(MediaListSurfaceInput::First, None);
                return self.cursor_message();
            }
            Key::End if key.modifiers.is_empty() => {
                self.delegate_row_local_input(MediaListSurfaceInput::Last, None);
                return self.cursor_message();
            }
            Key::Enter => {
                return match self
                    .delegate_row_local_input(MediaListSurfaceInput::Activate, None)
                    .external_intent
                {
                    Some(RowIntent::Activate(slot_id)) => Some(Msg::Queue(QueueRequest::Play {
                        scope: self.scope,
                        slot_id,
                    })),
                    _ => None,
                };
            }
            Key::Delete => {
                if !self.carrier.multi_selection().is_empty() {
                    let slot_ids = self.carrier.multi_selection().to_vec();
                    self.carrier.clear_owner_selection();
                    return Some(Msg::Queue(QueueRequest::RemoveSelection {
                        scope: self.scope,
                        slot_ids,
                    }));
                }
                return self
                    .selected_slot()
                    .map(|(scope, slot_id)| Msg::Queue(QueueRequest::Remove { scope, slot_id }));
            }
            Key::Up if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) => {
                return self.selected_slot().map(|(scope, slot_id)| {
                    Msg::Queue(QueueRequest::Move {
                        scope,
                        slot_id,
                        direction: QueueMove::Up,
                    })
                });
            }
            Key::Down if key.modifiers.contains(tuirealm::event::KeyModifiers::SHIFT) => {
                return self.selected_slot().map(|(scope, slot_id)| {
                    Msg::Queue(QueueRequest::Move {
                        scope,
                        slot_id,
                        direction: QueueMove::Down,
                    })
                });
            }
            Key::Char('z')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                return Some(Msg::Queue(QueueRequest::Undo { scope: self.scope }));
            }
            Key::Char('.') if key.modifiers.is_empty() => {
                // `.` is a selection-dependent chord the focused component
                // owns (CONTEXT.md "Global chord"): emit the queue context-menu
                // request for the currently selected row.
                return match self
                    .delegate_row_local_input(MediaListSurfaceInput::Context, None)
                    .external_intent
                {
                    Some(RowIntent::Context(slot_id)) => {
                        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                            ContextMenuTargets::Queue(vec![slot_id]),
                            None,
                        ))))
                    }
                    Some(RowIntent::ContextSelection(slot_ids)) => Some(Msg::Shell(Box::new(
                        ShellRequest::RowContextMenu(ContextMenuTargets::Queue(slot_ids), None),
                    ))),
                    _ => Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
                        ContextMenuTargets::Queue(vec![]),
                        None,
                    )))),
                };
            }
            Key::Char('i') => {
                return self.selected_slot().map(|(scope, slot_id)| {
                    Msg::Shell(Box::new(ShellRequest::QueueIntent(QueueIntent::Navigate {
                        scope,
                        slot_id,
                    })))
                });
            }
            Key::Char('p') => {
                return Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::PlayNow,
                ))));
            }
            Key::Char('s')
                if key
                    .modifiers
                    .contains(tuirealm::event::KeyModifiers::CONTROL) =>
            {
                return Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::SavePlaylist,
                ))));
            }
            Key::Char('c') if !key.modifiers.contains(tuirealm::event::KeyModifiers::ALT) => {
                return Some(Msg::Shell(Box::new(ShellRequest::QueueIntent(
                    QueueIntent::Clear,
                ))));
            }
            _ => {}
        }
        None
    }
}
