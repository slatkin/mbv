//! Interactive Component for the blocking Selection modal.
//!
//! The shell supplies the validated source snapshot. This component owns the
//! modal cursor, filter cursor, rendering, and the hit targets produced by the
//! same render seam. Source-specific filtering and activation remain shell
//! effects during the mirror-first stage.

use ratatui::layout::Rect;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_selection_modal_content, SelectionModalRenderModel};
use crate::app::types_selection_modal::{
    SelectionModal, SelectionModalFilter, SelectionModalListState, SelectionModalRow,
    SelectionModalSource,
};

pub struct SelectionModalComponent {
    modal: Option<SelectionModal>,
    painted_panel_area: Option<Rect>,
    selector_targets: Vec<(Rect, usize)>,
    row_targets: Vec<(Rect, usize)>,
    dim_backdrop_active: bool,
}

impl SelectionModalComponent {
    pub fn new() -> Self {
        Self {
            modal: None,
            painted_panel_area: None,
            selector_targets: Vec::new(),
            row_targets: Vec::new(),
            dim_backdrop_active: false,
        }
    }

    /// Replace shell-owned content while preserving the component's local
    /// cursor and filter selection when the same modal remains open.
    pub(in crate::app) fn set_content(&mut self, snapshot: &SelectionModal) {
        let previous_source = self.modal.as_ref().map(|modal| modal.source.clone());
        let previous_id = self.selected_id();
        let same_source = previous_source.as_ref() == Some(&snapshot.source);
        let previous_filter = self.modal.as_ref().and_then(|modal| modal.filter.clone());
        let mut modal = snapshot.clone();

        if same_source {
            modal.cursor = previous_id
                .as_deref()
                .and_then(|id| {
                    modal
                        .state
                        .rows()
                        .iter()
                        .position(|row| row.item_id() == Some(id))
                })
                .or_else(|| {
                    modal
                        .state
                        .rows()
                        .get(snapshot.cursor)
                        .and_then(SelectionModalRow::item_id)
                        .and_then(|id| {
                            modal
                                .state
                                .rows()
                                .iter()
                                .position(|row| row.item_id() == Some(id))
                        })
                })
                .unwrap_or_else(|| first_item_index(&modal.state).unwrap_or(0));
            if let (Some(current), Some(previous)) = (modal.filter.as_mut(), previous_filter) {
                if current.labels == previous.labels {
                    current.selected = previous
                        .selected
                        .min(current.labels.len().saturating_sub(1));
                }
            }
        }

        self.modal = Some(modal);
    }

    pub(in crate::app) fn selected_id(&self) -> Option<&str> {
        self.modal.as_ref().and_then(|modal| {
            modal
                .state
                .rows()
                .get(modal.cursor)
                .and_then(SelectionModalRow::item_id)
        })
    }

    pub(in crate::app) fn list_state(&self) -> Option<&SelectionModalListState> {
        self.modal.as_ref().map(|modal| &modal.state)
    }

    pub(in crate::app) fn filter_selected(&self) -> Option<usize> {
        self.modal
            .as_ref()
            .and_then(|modal| modal.filter.as_ref().map(|filter| filter.selected))
    }

    pub(in crate::app) fn row_targets(&self) -> &[(Rect, usize)] {
        &self.row_targets
    }

    pub(in crate::app) fn selector_targets(&self) -> &[(Rect, usize)] {
        &self.selector_targets
    }

    pub(in crate::app) fn source(&self) -> Option<&SelectionModalSource> {
        self.modal.as_ref().map(|modal| &modal.source)
    }

    pub(in crate::app) fn refresh(
        &mut self,
        source: &SelectionModalSource,
        state: SelectionModalListState,
        filter: Option<SelectionModalFilter>,
    ) {
        let Some(current) = self.modal.as_ref() else {
            return;
        };
        if &current.source != source {
            return;
        }
        let snapshot = SelectionModal {
            source: current.source.clone(),
            title: current.title.clone(),
            state: state.normalize(),
            cursor: current.cursor,
            filter: filter.or_else(|| current.filter.clone()),
        };
        self.set_content(&snapshot);
    }

    fn handle_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.modal.as_ref()?.filter.is_some() {
            match key.code {
                Key::Char('[') => return self.select_filter(-1),
                Key::Char(']') => return self.select_filter(1),
                _ => {}
            }
        }
        match key.code {
            Key::Up => {
                self.move_cursor(-1);
                None
            }
            Key::Down => {
                self.move_cursor(1);
                None
            }
            Key::Enter => Some(Msg::Shell(ShellRequest::SelectionModalActivate(
                self.selected_id().map(str::to_owned),
            ))),
            Key::Esc | Key::Backspace => Some(Msg::Shell(ShellRequest::DismissSelectionModal)),
            _ => None,
        }
    }

    fn select_filter(&mut self, delta: i64) -> Option<Msg> {
        {
            let filter = self.modal.as_mut()?.filter.as_mut()?;
            if filter.labels.is_empty() {
                return None;
            }
            filter.selected =
                ((filter.selected as i64 + delta).rem_euclid(filter.labels.len() as i64)) as usize;
        }
        let cursor = first_item_index(&self.modal.as_ref()?.state).unwrap_or(0);
        self.modal.as_mut()?.cursor = cursor;
        Some(Msg::Shell(ShellRequest::SelectionModalFilterSelected))
    }

    fn move_cursor(&mut self, delta: i64) {
        let Some(modal) = self.modal.as_mut() else {
            return;
        };
        let item_positions: Vec<usize> = modal
            .state
            .rows()
            .iter()
            .enumerate()
            .filter_map(|(index, row)| row.item_id().is_some().then_some(index))
            .collect();
        let Some(current) = item_positions
            .iter()
            .position(|&index| index == modal.cursor)
        else {
            modal.cursor = item_positions.first().copied().unwrap_or(0);
            return;
        };
        let next = (current as i64 + delta).clamp(0, item_positions.len() as i64 - 1) as usize;
        modal.cursor = item_positions[next];
    }

    fn handle_mouse(&mut self, mouse: &MouseEvent) -> Option<Msg> {
        if !matches!(mouse.kind, MouseEventKind::Down(MouseButton::Left)) {
            return None;
        }
        let pos = (mouse.column, mouse.row).into();
        if !self
            .painted_panel_area
            .is_some_and(|area| area.contains(pos))
        {
            return Some(Msg::Shell(ShellRequest::DismissSelectionModal));
        }
        if let Some((_, target)) = self
            .selector_targets
            .iter()
            .find(|(area, _)| area.contains(pos))
        {
            if let Some(modal) = self.modal.as_mut() {
                if let Some(filter) = modal.filter.as_mut() {
                    filter.selected = *target;
                }
                modal.cursor = first_item_index(&modal.state).unwrap_or(0);
            }
            return Some(Msg::Shell(ShellRequest::SelectionModalFilterSelected));
        }
        if let Some((_, row_index)) = self.row_targets.iter().find(|(area, _)| area.contains(pos)) {
            if let Some(modal) = self.modal.as_mut() {
                modal.cursor = *row_index;
            }
            return Some(Msg::Shell(ShellRequest::SelectionModalActivate(
                self.selected_id().map(str::to_owned),
            )));
        }
        None
    }
}

fn first_item_index(state: &SelectionModalListState) -> Option<usize> {
    state.rows().iter().position(|row| row.item_id().is_some())
}

impl Default for SelectionModalComponent {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for SelectionModalComponent {
    fn view(&mut self, f: &mut Frame, _area: Rect) {
        let Some(modal) = self.modal.as_ref() else {
            self.painted_panel_area = None;
            self.selector_targets.clear();
            self.row_targets.clear();
            return;
        };
        let geometry = render_selection_modal_content(
            f,
            &mut self.dim_backdrop_active,
            SelectionModalRenderModel {
                title: &modal.title,
                state: &modal.state,
                cursor: modal.cursor,
                filter: modal.filter.as_ref(),
            },
        );
        self.painted_panel_area = Some(geometry.area);
        self.selector_targets = geometry.selector_tabs;
        self.row_targets = geometry.rows;
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

impl AppComponent<Msg, UserEvent> for SelectionModalComponent {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Keyboard(key) => self.handle_key(key),
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}
