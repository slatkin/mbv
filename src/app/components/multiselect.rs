//! Interactive Component for a nested Settings Multiselect popup.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::mouse::gesture::{MouseGesture, MouseGestureState};
use super::mouse::hit::HitRegions;
use super::msg::Msg;
use super::user_event::UserEvent;
use crate::app::render::{render_multiselect_content, MultiSelectRenderModel};
use crate::app::types_context_menu::{MultiSelectItem, MultiSelectKind, MultiSelectPopup};

pub struct MultiselectComponent {
    kind: Option<MultiSelectKind>,
    items: Vec<(String, String, bool)>,
    cursor: usize,
    dim_backdrop_active: bool,
    /// The painted modal rect (last frame) — the outside-click boundary.
    frame: Rect,
    /// Irregular painted chrome (task 5.1, design.md D6): item rows,
    /// repopulated in `view()` from the geometry the painter just produced.
    hit_rows: HitRegions<usize>,
    /// Private per-parent gesture recognition (ADR 0024, design.md D3).
    mouse_gestures: MouseGestureState,
}

impl MultiselectComponent {
    pub fn new() -> Self {
        Self {
            kind: None,
            items: Vec::new(),
            cursor: 0,
            dim_backdrop_active: false,
            frame: Rect::default(),
            hit_rows: HitRegions::new(),
            mouse_gestures: MouseGestureState::new(),
        }
    }

    /// Mirror App content without replacing local choices while the popup is
    /// open. App receives the choices only when the component commits them.
    pub(in crate::app) fn set_content(&mut self, popup: &MultiSelectPopup) {
        let same_items = self.items.len() == popup.items.len()
            && self
                .items
                .iter()
                .zip(&popup.items)
                .all(|(current, next)| current.0 == next.0 && current.1 == next.1);
        if self.kind != Some(popup.kind) || !same_items {
            self.kind = Some(popup.kind);
            self.items = popup.items.clone();
            self.cursor = popup.cursor.min(self.items.len().saturating_sub(1));
        } else {
            self.cursor = self.cursor.min(self.items.len().saturating_sub(1));
        }
    }

    pub(in crate::app) fn commit_snapshot(
        &self,
    ) -> Option<(MultiSelectKind, Vec<MultiSelectItem>)> {
        self.kind.map(|kind| (kind, self.items.clone()))
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        match key.code {
            Key::Up => {
                self.cursor = self.cursor.saturating_sub(1);
                None
            }
            Key::Down => {
                if !self.items.is_empty() {
                    self.cursor = (self.cursor + 1).min(self.items.len() - 1);
                }
                None
            }
            Key::Char(' ') => {
                if let Some(item) = self.items.get_mut(self.cursor) {
                    item.2 = !item.2;
                }
                None
            }
            Key::Esc | Key::Enter => self.commit_snapshot().map(|(kind, items)| {
                Msg::Shell(super::msg::ShellRequest::MultiselectCommit { kind, items })
            }),
            _ => None,
        }
    }

    /// Mouse handling (task 5.1): only actions with a keyboard equivalent.
    /// A row click toggles that item's selection and moves the cursor to it
    /// (Space equivalent), a double-click commits (Enter equivalent), and an
    /// outside click commits exactly like Esc — this popup's only dismiss
    /// path *is* a commit (`Esc | Enter` both emit `MultiselectCommit`).
    /// The painted rows are `[ ]/[x]` checkboxes with no separate confirm
    /// button, so there is no click target for a bare confirm. Right-click
    /// and wheel have no keyboard equivalent here and are ignored.
    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if matches!(mouse.kind, MouseEventKind::Moved) {
            return None;
        }
        match self.mouse_gestures.recognize(mouse)? {
            MouseGesture::Click(at) => {
                if let Some(&index) = self.hit_rows.resolve(at) {
                    if let Some(item) = self.items.get_mut(index) {
                        item.2 = !item.2;
                        self.cursor = index;
                    }
                    return None;
                }
                if !self.frame.contains(at) {
                    return self.commit_request();
                }
                None
            }
            MouseGesture::DoubleClick(at) => {
                if self.hit_rows.resolve(at).is_some() {
                    return self.commit_request();
                }
                // Outside double-click: the first click already committed.
                None
            }
            _ => None,
        }
    }

    /// The keyboard Esc/Enter path: commit the current choices.
    fn commit_request(&self) -> Option<Msg> {
        self.commit_snapshot().map(|(kind, items)| {
            Msg::Shell(super::msg::ShellRequest::MultiselectCommit { kind, items })
        })
    }

    #[cfg(test)]
    pub(crate) fn test_rows(&self) -> &HitRegions<usize> {
        &self.hit_rows
    }

    #[cfg(test)]
    pub(crate) fn test_cursor(&self) -> usize {
        self.cursor
    }

    #[cfg(test)]
    pub(crate) fn test_items(&self) -> &[(String, String, bool)] {
        &self.items
    }

    #[cfg(test)]
    pub(crate) fn test_frame(&self) -> Rect {
        self.frame
    }

    /// Test seam: forget the last click so the next event is neither
    /// throttled nor promoted to a double-click.
    #[cfg(test)]
    pub(crate) fn reset_mouse_gestures_for_test(&mut self) {
        self.mouse_gestures.reset_for_test();
    }
}

impl Default for MultiselectComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MultiselectComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        let Some(kind) = self.kind else {
            return;
        };
        let geometry = render_multiselect_content(
            f,
            &mut self.dim_backdrop_active,
            MultiSelectRenderModel {
                kind,
                items: &self.items,
                cursor: self.cursor,
            },
        );
        // Adopt the rects the painter just produced into the irregular-
        // chrome registry (task 5.1, design.md D6).
        self.frame = geometry.frame;
        self.hit_rows.clear();
        for (rect, index) in geometry.rows {
            self.hit_rows.push(rect, index);
        }
    }

    fn query<'a>(&'a self, _attr: Attribute) -> Option<QueryResult<'a>> {
        None
    }

    fn attr(&mut self, _attr: Attribute, _value: AttrValue) {}

    fn state(&self) -> State {
        State::None
    }

    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for MultiselectComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tuirealm::event::{KeyEvent, KeyModifiers};

    fn popup() -> MultiSelectPopup {
        MultiSelectPopup {
            kind: MultiSelectKind::HiddenLibraries,
            items: vec![
                ("movies".into(), "Movies".into(), false),
                ("shows".into(), "Shows".into(), true),
            ],
            cursor: 0,
        }
    }

    fn key(code: Key) -> Event<UserEvent> {
        Event::Keyboard(KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        })
    }

    #[test]
    fn settings_popup_multiselect_keeps_local_cursor_and_choice() {
        let mut component = MultiselectComponent::new();
        component.set_content(&popup());
        assert_eq!(component.on(&key(Key::Down)), None);
        assert_eq!(component.on(&key(Key::Char(' '))), None);

        assert_eq!(component.cursor, 1);
        assert!(!component.items[1].2);
    }

    #[test]
    fn settings_popup_multiselect_commit_is_typed() {
        let mut component = MultiselectComponent::new();
        component.set_content(&popup());

        assert!(matches!(
            component.on(&key(Key::Enter)),
            Some(Msg::Shell(
                super::super::msg::ShellRequest::MultiselectCommit { .. }
            ))
        ));
    }

    #[test]
    fn settings_popup_multiselect_renders_without_app_state() {
        let mut component = MultiselectComponent::new();
        component.set_content(&popup());
        let mut terminal = Terminal::new(TestBackend::new(60, 16)).unwrap();
        terminal
            .draw(|frame| component.view(frame, frame.area()))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("Hidden Libraries"));
        assert!(output.contains("Movies"));
    }
}
