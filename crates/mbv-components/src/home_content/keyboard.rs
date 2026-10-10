//! Home's input translation (row 5.2, design D6 of
//! `home-recently-added-tree`): the embedded owner's key handling and the
//! panel's slot-gesture translation over the shared [`TreeBrowser`]. Enter
//! activates an item or toggles a root or branch; Right never collapses; the
//! action chords (`Ctrl+Enter`, `Ctrl+W`, Delete, `.`) carry the marked item
//! targets in display order, or the single selected item target.

use ratatui::layout::Position;
use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use super::super::list::tree_browser::{TreeExternalIntent, TreeOperation};
use super::HomeContent;
use crate::media_list::MediaListSurfaceInput;
use mbv_ui_model::context_menu::ContextMenuTargets;
use mbv_ui_model::targets::{HomeItemTarget, HomeTreeTarget};
use mbv_ui_msg::{LeafKeyResult, Msg, ShellRequest};

impl HomeContent {
    /// The item targets a resolved activate or context intent carries:
    /// an `Item` resolves to its section-scoped target; a `Root`/`Age`
    /// target stays local to the owner (no request crosses the boundary).
    /// A marked `ContextSelection` keeps the tree's display order.
    fn item_targets(intent: Option<TreeExternalIntent<HomeTreeTarget>>) -> Vec<HomeItemTarget> {
        let Some(intent) = intent else {
            return Vec::new();
        };
        match intent {
            TreeExternalIntent::Activate(HomeTreeTarget::Item(target))
            | TreeExternalIntent::Context(HomeTreeTarget::Item(target)) => {
                vec![target]
            }
            TreeExternalIntent::ContextSelection(targets) => Self::marked_item_targets(targets),
            TreeExternalIntent::Activate(_) | TreeExternalIntent::Context(_) => Vec::new(),
        }
    }

    /// The item targets among resolved tree targets, in the order given.
    fn marked_item_targets(targets: Vec<HomeTreeTarget>) -> Vec<HomeItemTarget> {
        targets
            .into_iter()
            .filter_map(|target| match target {
                HomeTreeTarget::Item(item_target) => Some(item_target),
                _ => None,
            })
            .collect()
    }

    /// The action chords' targets (design D6): the marked item targets in
    /// display order — the shared mark set expands an aggregate root or
    /// branch to its item descendants — or the single selected item target.
    fn action_item_targets(&self) -> Vec<HomeItemTarget> {
        let marked = Self::marked_item_targets(self.tree.action_targets());
        if marked.is_empty() {
            self.selected_item_target().into_iter().collect()
        } else {
            marked
        }
    }

    /// The shell request for an action chord over its item targets, or no
    /// request when there are none.
    fn action_request(&self, make: fn(Vec<HomeItemTarget>) -> ShellRequest) -> Option<Msg> {
        let targets = self.action_item_targets();
        (!targets.is_empty()).then(|| Msg::Shell(Box::new(make(targets))))
    }

    /// Home's local key interpretation, forwarded by the focused panel (the
    /// mounted `HomeComponent::handle_key` contract, unchanged — the router
    /// owns every global chord and keeps precedence). The aggregate mark
    /// surface replaces the retired carrier's visual mode: `Ctrl+A`, Esc and
    /// Space stay unhandled here, and marking goes through the tree's own
    /// toggle surface.
    pub(super) fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if key.modifiers.contains(KeyModifiers::ALT)
            && matches!(key.code, Key::Left | Key::Right | Key::Up | Key::Down)
        {
            return None;
        }
        match key.code {
            Key::Up => {
                self.tree.apply(TreeOperation::Move(-1));
                None
            }
            Key::Down => {
                self.tree.apply(TreeOperation::Move(1));
                None
            }
            Key::PageUp => {
                self.tree.apply(TreeOperation::Page(-1));
                None
            }
            Key::PageDown => {
                self.tree.apply(TreeOperation::Page(1));
                None
            }
            Key::Home => {
                self.tree.apply(TreeOperation::First);
                None
            }
            Key::End => {
                self.tree.apply(TreeOperation::Last);
                None
            }
            Key::Left => {
                self.tree.apply(TreeOperation::Parent);
                None
            }
            // Right never activates or collapses (design D6): the tree's
            // expanded-root `Activate` intent is consumed and dropped.
            Key::Right => {
                self.tree.apply(TreeOperation::Right);
                None
            }
            // Ctrl+A multi-select (visual mode) is replaced by the shared
            // tree's aggregate mark surface; Home keeps no select-all.
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => None,
            Key::Char('.') => self.context_menu_request(None),
            Key::Enter if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.action_request(ShellRequest::HomeEnqueue)
            }
            Key::Enter => self.activate_selected_row(),
            Key::Char('w') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.action_request(ShellRequest::HomeToggleWatched)
            }
            Key::Delete => self.action_request(ShellRequest::HomeDelete),
            _ => None,
        }
    }

    pub(super) fn activate_selected_row(&mut self) -> Option<Msg> {
        // Design D6: Enter applies `Activate`; on a `Root`/`Age` target the
        // owner resolves it locally into an expansion toggle.
        match self.tree.selected_target() {
            Some(HomeTreeTarget::Item(item_target)) => {
                let target = item_target.clone();
                self.tree.apply(TreeOperation::Activate);
                Some(Msg::Shell(Box::new(ShellRequest::HomePlay(target))))
            }
            Some(root_or_age @ (HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_))) => {
                let root_or_age = root_or_age.clone();
                self.tree
                    .apply(TreeOperation::ToggleExpansionTarget(root_or_age));
                None
            }
            None => None,
        }
    }

    /// The `.` request: the selected item target, or the marked item targets
    /// in display order (the tree's `Context` operation resolves both). A
    /// structural row with no marks carries no item target, so it emits no
    /// menu.
    fn context_menu_request(&mut self, anchor: Option<(u16, u16)>) -> Option<Msg> {
        let targets = Self::item_targets(self.tree.apply(TreeOperation::Context).external_intent);
        (!targets.is_empty()).then(|| Self::context_menu(targets, anchor))
    }

    fn context_menu(targets: Vec<HomeItemTarget>, anchor: Option<(u16, u16)>) -> Msg {
        Msg::Shell(Box::new(ShellRequest::RowContextMenu(
            ContextMenuTargets::Home(targets),
            anchor,
        )))
    }

    /// Translate resolved list gestures into Home's typed requests.
    pub(super) fn on_list_slot_event(&mut self, input: MediaListSurfaceInput) -> Option<Msg> {
        match input {
            MediaListSurfaceInput::Wheel { at, delta } => {
                // The wheel scrolls the viewport, not the selection
                // (wheel-scrolls-viewport D4); a claimed wheel still focuses
                // Library.
                if !self.tree.claims_current_point(at) {
                    return None;
                }
                self.tree.apply(TreeOperation::Scroll(delta));
                Some(Msg::TerminalEvent(
                    mbv_ui_msg::TerminalObserverEvent::MouseClaimed,
                ))
            }
            MediaListSurfaceInput::Click(at) | MediaListSurfaceInput::RangeClick(at) => {
                let target = self.select_at(at)?;
                match target {
                    HomeTreeTarget::Item(item_target) => {
                        Some(Msg::Shell(Box::new(ShellRequest::HomeRowClick {
                            target: item_target,
                        })))
                    }
                    _ => None,
                }
            }
            MediaListSurfaceInput::ToggleClick(at) => {
                self.tree.resolve_current_point(at)?;
                self.tree.apply(TreeOperation::PointerToggleMark(at));
                None
            }
            MediaListSurfaceInput::DoubleClick(at) => {
                let target = self.select_at(at)?;
                match target {
                    HomeTreeTarget::Item(item_target) => {
                        Some(Msg::Shell(Box::new(ShellRequest::HomeRowActivate {
                            target: item_target,
                        })))
                    }
                    root_or_age @ (HomeTreeTarget::Root(_) | HomeTreeTarget::Age(_)) => {
                        self.tree
                            .apply(TreeOperation::ToggleExpansionTarget(root_or_age));
                        None
                    }
                }
            }
            MediaListSurfaceInput::ContextClick(at) => {
                self.select_at(at)?;
                self.context_menu_request(Some((at.x, at.y)))
            }
            _ => None,
        }
    }

    /// Selects the tree row under a pointer position and returns its target,
    /// or `None` when the position resolves to no row.
    fn select_at(&mut self, at: Position) -> Option<HomeTreeTarget> {
        let target = self.tree.resolve_current_point(at).cloned()?;
        self.tree.apply(TreeOperation::Select(target.clone()));
        Some(target)
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
