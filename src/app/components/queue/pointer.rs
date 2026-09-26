use ratatui::layout::Position;
use tuirealm::event::{MouseEvent, MouseEventKind};

use crate::app::components::media_list::{MediaListSurfaceInput, RowIntent};
use crate::app::components::mouse::gesture::{ClickModifier, MouseGesture};
use crate::app::components::msg::{Msg, QueueRequest, ShellRequest, TerminalObserverEvent};
use crate::app::state::types::context_menu::ContextMenuTargets;
use crate::app::state::types::playback::QueueScope;

use super::QueueComponent;

impl QueueComponent {
    /// Gesture recognition (click / double-click / right-click / wheel) comes
    /// from the private `MouseGestureState` (ADR 0024, design.md D3). Row
    /// identity comes from the embedded control's retained current-frame
    /// point resolution (design.md D6). The component emits a semantic `Msg`
    /// with a resolved `QueueSlotId`/scope — never raw coordinates — except
    /// the context-menu anchor (design.md D4).
    pub(super) fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        // Queue does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Scroll { at, delta } => self.handle_scroll(at, delta),
            MouseGesture::Click { at, modifier } => self.handle_click(at, modifier),
            MouseGesture::DoubleClick(at) => self.handle_double_click(at),
            MouseGesture::RightClick(at) => self.handle_right_click(at, mouse.column, mouse.row),
            MouseGesture::Drag { to, .. } => self.handle_drag(to),
            MouseGesture::DragEnd => {
                self.drag_grab = None;
                None
            }
        }
    }

    fn handle_scroll(&mut self, at: Position, delta: i64) -> Option<Msg> {
        if !self.carrier.claims_current_point(at) {
            return None;
        }
        self.delegate_row_local_input(MediaListSurfaceInput::Wheel { at, delta }, None);
        // Return a framework-visible claim after mutating local state;
        // dropping the message would let the framework's mutation be
        // discarded by the mouse fold.
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }

    fn handle_click(&mut self, at: Position, modifier: ClickModifier) -> Option<Msg> {
        if let Some(scope) = self.claim_scope_pill(at) {
            return Some(Msg::Shell(Box::new(ShellRequest::QueueScopeClick {
                scope,
            })));
        }
        if !self.carrier.claims_current_point(at) {
            return None;
        }
        let target = self.carrier.resolve_current_point(at).copied();
        if let Some(target) = target {
            let input = match modifier {
                ClickModifier::Ctrl => MediaListSurfaceInput::ToggleClick(at),
                ClickModifier::Shift => MediaListSurfaceInput::RangeClick(at),
                ClickModifier::None => MediaListSurfaceInput::Click(at),
            };
            self.delegate_row_local_input(input, Some(target));
        }
        self.drag_grab = if modifier == ClickModifier::None {
            target
        } else {
            None
        };
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowClick {
            slot_id: self.carrier.selected_target().copied(),
        })))
    }

    fn handle_double_click(&mut self, at: Position) -> Option<Msg> {
        if let Some(scope) = self.claim_scope_pill(at) {
            return Some(Msg::Shell(Box::new(ShellRequest::QueueScopeClick {
                scope,
            })));
        }
        if !self.carrier.claims_current_point(at) {
            return None;
        }
        let target = self.carrier.resolve_current_point(at).copied();
        if let Some(target) = target {
            self.delegate_row_local_input(MediaListSurfaceInput::Click(at), Some(target));
        }
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
            slot_id: self.carrier.selected_target().copied(),
        })))
    }

    fn handle_right_click(&mut self, at: Position, column: u16, row: u16) -> Option<Msg> {
        // Legacy parity: a right-click on blank queue space opens no
        // menu. Only resolve a menu when the click lands on a row —
        // never fall back to the prior selection (design.md D4).
        let slot_id = self.carrier.resolve_current_point(at).copied()?;
        let outcome =
            self.delegate_row_local_input(MediaListSurfaceInput::ContextClick(at), Some(slot_id));
        let targets = match outcome.external_intent {
            Some(RowIntent::Context(target)) => vec![target],
            Some(RowIntent::ContextSelection(targets)) => targets,
            _ => vec![slot_id],
        };
        Some(Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            ContextMenuTargets::Queue(targets),
            Some((column, row)),
        ))))
    }

    fn handle_drag(&mut self, to: Position) -> Option<Msg> {
        let grabbed = self.drag_grab?;
        let resolved = self.carrier.resolve_current_point(to).copied()?;
        if resolved == grabbed {
            return None;
        }
        self.delegate_row_local_input(MediaListSurfaceInput::Click(to), Some(grabbed));
        Some(Msg::Queue(QueueRequest::MoveTo {
            scope: self.scope,
            slot_id: grabbed,
            onto: resolved,
        }))
    }

    /// If `at` lands on a scope pill, switch the component's own scope and
    /// reset its scroll, and return the new scope for the shell dispatch.
    fn claim_scope_pill(&mut self, at: Position) -> Option<QueueScope> {
        let scope = if self.scope_local.is_some_and(|r| r.contains(at)) {
            QueueScope::Local
        } else if self.scope_remote.is_some_and(|r| r.contains(at)) {
            QueueScope::Remote
        } else {
            return None;
        };
        self.scope = scope;
        self.carrier.set_scroll(0);
        Some(scope)
    }
}
