use ratatui::layout::Position;
use tuirealm::event::{MouseEvent, MouseEventKind};

use crate::media_list::{MediaListSurfaceInput, RowIntent};
use mbv_surfaces::gesture::{ClickSelection, PointerGesture};
use mbv_ui_model::context_menu::ContextMenuTargets;
use mbv_ui_msg::{Msg, QueueRequest, ShellRequest, TerminalObserverEvent};

use super::QueueComponent;

impl QueueComponent {
    /// Gesture recognition (click / double-click / right-click / wheel) comes
    /// from the private `MouseGestureState` (ADR 0024, design.md D3). Row
    /// identity comes from the embedded control's retained current-frame
    /// point resolution (design.md D6). The component emits a semantic `Msg`
    /// with a resolved `QueueSlotId`/link — never raw coordinates — except
    /// the context-menu anchor (design.md D4).
    pub fn handle_mouse(&mut self, mouse: MouseEvent) -> Option<Msg> {
        // Queue does not consume hover-move (design.md D7).
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            PointerGesture::Scroll { at, delta } => self.handle_scroll(at, delta),
            PointerGesture::Click { at, selection } => self.handle_click(at, selection),
            PointerGesture::DoubleClick(at) => self.handle_double_click(at),
            PointerGesture::RightClick(at) => self.handle_right_click(at, mouse.column, mouse.row),
            PointerGesture::Drag { to, .. } => self.handle_drag(to),
            PointerGesture::DragEnd => {
                self.drag_grab = None;
                None
            }
        }
    }

    fn handle_scroll(&mut self, at: Position, delta: i64) -> Option<Msg> {
        if !self.wide.claims_current_point(at) {
            return None;
        }
        self.delegate_row_local_input(MediaListSurfaceInput::Wheel { at, delta }, None);
        // Return a framework-visible claim after mutating local state;
        // dropping the message would let the framework's mutation be
        // discarded by the mouse fold.
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }

    fn handle_click(&mut self, at: Position, selection: ClickSelection) -> Option<Msg> {
        if self.playlist_hit.width > 0 && self.playlist_hit.contains(at) {
            self.drag_grab = None;
            return Some(Msg::Shell(Box::new(ShellRequest::OpenPlaylists)));
        }
        if !self.wide.claims_current_point(at) {
            return None;
        }
        let target = self.wide.resolve_current_point(at).copied();
        if let Some(target) = target {
            let input = match selection {
                ClickSelection::Toggle => MediaListSurfaceInput::ToggleClick(at),
                ClickSelection::Range => MediaListSurfaceInput::RangeClick(at),
                ClickSelection::Plain => MediaListSurfaceInput::Click(at),
            };
            self.delegate_row_local_input(input, Some(target));
        }
        self.drag_grab = if selection == ClickSelection::Plain {
            target
        } else {
            None
        };
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowClick {
            slot_id: self.surface.selected_target().copied(),
        })))
    }

    fn handle_double_click(&mut self, at: Position) -> Option<Msg> {
        if self.playlist_hit.width > 0 && self.playlist_hit.contains(at) {
            return None;
        }
        if !self.wide.claims_current_point(at) {
            return None;
        }
        let target = self.wide.resolve_current_point(at).copied();
        if let Some(target) = target {
            self.delegate_row_local_input(MediaListSurfaceInput::Click(at), Some(target));
        }
        Some(Msg::Shell(Box::new(ShellRequest::QueueRowActivate {
            slot_id: self.surface.selected_target().copied(),
        })))
    }

    fn handle_right_click(&mut self, at: Position, column: u16, row: u16) -> Option<Msg> {
        // Legacy parity: a right-click on blank queue space opens no
        // menu. Only resolve a menu when the click lands on a row —
        // never fall back to the prior selection (design.md D4).
        let slot_id = self.wide.resolve_current_point(at).copied()?;
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
        let resolved = self.wide.resolve_current_point(to).copied()?;
        if resolved == grabbed {
            return None;
        }
        self.delegate_row_local_input(MediaListSurfaceInput::Click(to), Some(grabbed));
        Some(Msg::Queue(QueueRequest::MoveTo {
            link: self.surface.link(),
            slot_id: grabbed,
            onto: resolved,
        }))
    }
}
