//! Home's input translation over the flat carrier: the embedded owner's key
//! handling and the panel's slot-gesture translation. Enter plays the
//! selected item; the action chords (`Ctrl+Enter`, `Ctrl+W`, Delete, `.`)
//! carry the marked item targets in display order, or the selected item
//! target (`openspec/specs/home-latest-sections`: "Home actions resolve the
//! row's own section and Service identity").

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::HomeContent;
use crate::media_list::{MediaListOperation, MediaListSurfaceInput, RowIntent};
use mbv_ui_model::targets::HomeItemTarget;
use mbv_ui_msg::{HomeRequest, LeafKeyResult, Msg, ShellRequest};

impl HomeContent {
    /// The selected row's item target.
    pub(super) fn selected_item_target(&self) -> Option<HomeItemTarget> {
        self.carrier.selected_target().cloned()
    }

    /// The marked item targets in display order, or the selected item target.
    fn action_item_targets(&self) -> Vec<HomeItemTarget> {
        let marked = self.display_order(self.carrier.multi_selection());
        if marked.is_empty() {
            self.selected_item_target().into_iter().collect()
        } else {
            marked
        }
    }

    /// `targets` re-ordered to the order the list shows them. Marks are kept
    /// in marking order, so they are sorted here.
    fn display_order(&self, targets: &[HomeItemTarget]) -> Vec<HomeItemTarget> {
        self.carrier
            .rows()
            .iter()
            .filter_map(|row| row.selectable_target())
            .filter(|target| targets.contains(target))
            .cloned()
            .collect()
    }

    fn action_request(&self, make: fn(Vec<HomeItemTarget>) -> HomeRequest) -> Option<Msg> {
        let targets = self.action_item_targets();
        (!targets.is_empty()).then(|| Msg::Shell(Box::new(ShellRequest::Home(make(targets)))))
    }

    fn context_menu(targets: Vec<HomeItemTarget>, anchor: Option<(u16, u16)>) -> Option<Msg> {
        (!targets.is_empty()).then(|| {
            Msg::Shell(Box::new(ShellRequest::Home(HomeRequest::ContextMenu(
                targets, anchor,
            ))))
        })
    }

    /// Home's local key interpretation, forwarded by the focused panel (the
    /// router owns every global chord and keeps precedence).
    pub(super) fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.carrier.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(Box::new(ShellRequest::SelectionProjection(
                self.carrier.selection_summary(),
            ))));
        }
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        match key.code {
            Key::Up => self.navigate(MediaListOperation::Move(-1)),
            Key::Down => self.navigate(MediaListOperation::Move(1)),
            Key::PageUp => self.navigate(MediaListOperation::Page(-1)),
            Key::PageDown => self.navigate(MediaListOperation::Page(1)),
            Key::Home => self.navigate(MediaListOperation::First),
            Key::End => self.navigate(MediaListOperation::Last),
            Key::Char('.') => Self::context_menu(self.action_item_targets(), None),
            Key::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.action_request(HomeRequest::Enqueue)
            }
            Key::Enter => self.activate_selected_row(),
            Key::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.action_request(HomeRequest::ToggleWatched)
            }
            Key::Delete => self.action_request(HomeRequest::Delete),
            _ => None,
        }
    }

    fn navigate(&mut self, operation: MediaListOperation<HomeItemTarget>) -> Option<Msg> {
        self.carrier.delegate_operation(operation);
        None
    }

    pub(super) fn activate_selected_row(&mut self) -> Option<Msg> {
        let target = self.selected_item_target()?;
        Some(Msg::Shell(Box::new(ShellRequest::Home(HomeRequest::Play(
            target,
        )))))
    }

    /// Translate resolved list gestures into Home's typed requests.
    pub(super) fn on_list_slot_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        let target = match input {
            MediaListSurfaceInput::Click(at)
            | MediaListSurfaceInput::ToggleClick(at)
            | MediaListSurfaceInput::RangeClick(at)
            | MediaListSurfaceInput::DoubleClick(at)
            | MediaListSurfaceInput::ContextClick(at) => {
                Some(self.carrier.resolve_current_point(at).cloned()?)
            }
            _ => None,
        };
        let outcome = self
            .carrier
            .delegate_operation(input.into_operation(target)?);
        match input {
            MediaListSurfaceInput::Wheel { .. } => Some(Msg::TerminalEvent(
                mbv_ui_msg::TerminalObserverEvent::MouseClaimed,
            )),
            MediaListSurfaceInput::Click(_)
            | MediaListSurfaceInput::ToggleClick(_)
            | MediaListSurfaceInput::RangeClick(_) => Some(Msg::Shell(Box::new(
                ShellRequest::Home(HomeRequest::RowClick(self.selected_item_target()?)),
            ))),
            MediaListSurfaceInput::DoubleClick(_) => Some(Msg::Shell(Box::new(
                ShellRequest::Home(HomeRequest::RowActivate(self.selected_item_target()?)),
            ))),
            MediaListSurfaceInput::ContextClick(at) => {
                let targets = match outcome.external_intent {
                    Some(RowIntent::Context(target)) => vec![target],
                    Some(RowIntent::ContextSelection(targets)) => self.display_order(&targets),
                    _ => self.action_item_targets(),
                };
                Self::context_menu(targets, Some((at.x, at.y)))
            }
            _ => None,
        }
    }

    /// The shared `LeafKeyResult` disposition: movement and activation keys
    /// are claimed even when they produce no request, so the panel never
    /// lets them fall through to a covered surface.
    pub(super) fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        match self.handle_key(key) {
            Some(message) => LeafKeyResult::Consumed(Some(Box::new(message))),
            None if matches!(
                key.code,
                Key::Up
                    | Key::Down
                    | Key::Left
                    | Key::Right
                    | Key::Home
                    | Key::End
                    | Key::PageUp
                    | Key::PageDown
                    | Key::Enter
            ) =>
            {
                LeafKeyResult::Consumed(None)
            }
            None => LeafKeyResult::Unhandled,
        }
    }
}
