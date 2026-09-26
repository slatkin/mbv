//! `TabPanel` — the mounted Interactive Component for the tab bar
//! (`RootFrame.tab` placement, task 2.1; design D10).
//!
//! Owns the tab bar's local interaction state: the tab hit regions the
//! painter resolves, and click resolution against them. Tab titles, the
//! selected position and the scroll anchor are shell-owned content (the
//! keyboard tab-cycling path reads/writes the same scroll anchor), projected
//! one-way by `Model::sync_tab_panel`. A click on a painted tab emits
//! `Msg::Shell(ShellRequest::TabSelect)`; the shell owns the tab switch and
//! its side effects. This replaces the deleted the deleted tabs hit map
//! side channel and the shell's `MouseClick` tab-click path.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::msg::{Msg, ShellRequest};
use super::user_event::UserEvent;
use crate::app::render::{render_tab_bar, TabBarModel};

/// The tab bar panel: paints the tab bar where `RootFrame` places it, retains
/// its tab hit regions, and emits a tab-select `Msg` for clicks.
pub struct TabPanel {
    titles: Vec<String>,
    markers: Vec<bool>,
    selected: usize,
    scroll: usize,
    /// Per-tab hit targets from the last paint, as
    /// `(screen_rect, tab_position)`; resolved against clicks.
    hits: Vec<(Rect, usize)>,
    /// Full tab position currently under the pointer, if any.
    hovered: Option<usize>,
}

impl TabPanel {
    pub fn new() -> Self {
        Self {
            titles: Vec::new(),
            markers: Vec::new(),
            selected: 0,
            scroll: 0,
            hits: Vec::new(),
            hovered: None,
        }
    }

    /// Project the shell-owned tab content (task 2.1): titles in position
    /// order, the selected tab's position, and the scroll anchor.
    pub(in crate::app) fn set_content(
        &mut self,
        titles: Vec<String>,
        markers: Vec<bool>,
        selected: usize,
        scroll: usize,
    ) {
        self.titles = titles;
        self.markers = markers;
        self.selected = selected;
        self.scroll = scroll;
    }

    /// The hit regions retained from the last paint (test accessor).
    #[cfg(test)]
    pub(in crate::app) fn hit_regions(&self) -> &[(Rect, usize)] {
        &self.hits
    }

    #[cfg(test)]
    pub(in crate::app) fn test_markers(&self) -> &[bool] {
        &self.markers
    }

    #[cfg(test)]
    pub(in crate::app) fn test_selected(&self) -> usize {
        self.selected
    }

    #[cfg(test)]
    pub(in crate::app) fn test_hovered(&self) -> Option<usize> {
        self.hovered
    }

    fn resolve_click(&self, at: Position) -> Option<usize> {
        self.hits
            .iter()
            .find(|(rect, _)| rect.contains(at))
            .map(|(_, tab_pos)| *tab_pos)
    }
}

impl Default for TabPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TabPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.hits.clear();
        render_tab_bar(
            frame,
            area,
            &TabBarModel {
                titles: &self.titles,
                markers: &self.markers,
                selected: self.selected,
                scroll: self.scroll,
                hovered: self.hovered,
            },
            &mut self.hits,
        );
    }
    fn query(&self, _attr: Attribute) -> Option<QueryResult<'_>> {
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

impl AppComponent<Msg, UserEvent> for TabPanel {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            // Resolve only geometry this panel painted; clicks elsewhere
            // (including the overflow arrows, which have never been
            // clickable) are no-ops.
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Moved => {
                let pos = Position::new(mouse.column, mouse.row);
                self.hovered = self
                    .hits
                    .iter()
                    .find(|(rect, _)| rect.contains(pos))
                    .map(|(_, tab_pos)| *tab_pos);
                None
            }
            Event::Mouse(mouse) if mouse.kind == MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position::new(mouse.column, mouse.row);
                self.resolve_click(pos)
                    .map(|tab_pos| Msg::Shell(Box::new(ShellRequest::TabSelect(tab_pos))))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    use crate::app::palette;

    fn titles() -> Vec<String> {
        ["Continue", "Movies", "TV Shows", "Music", "Feeds"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    fn drawn_panel(
        width: u16,
        titles: Vec<String>,
        markers: Vec<bool>,
        selected: usize,
        scroll: usize,
    ) -> (TabPanel, ratatui::buffer::Buffer) {
        let mut panel = TabPanel::new();
        panel.set_content(titles, markers, selected, scroll);
        let mut terminal = Terminal::new(TestBackend::new(width, 3)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, width, 3)))
            .unwrap();
        (panel, terminal.backend().buffer().clone())
    }

    /// Hit regions are retained at the painted tab labels; a click on one
    /// emits the tab-select `Msg` with the tab's position.
    #[test]
    fn click_on_a_painted_tab_emits_tab_select() {
        let (panel, _) = drawn_panel(80, titles(), vec![false; 5], 0, 0);
        let hits: Vec<(Rect, usize)> = panel.hit_regions().to_vec();
        assert_eq!(hits.len(), 5, "one hit region per painted tab: {hits:?}");
        let (rect, pos) = hits[2];
        assert_eq!(pos, 2);
        let mut panel = panel;
        let msg = panel.on(&Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: rect.x,
            row: rect.y,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }));
        assert_eq!(
            msg,
            Some(Msg::Shell(Box::new(ShellRequest::TabSelect(2)))),
            "clicking the third tab selects it"
        );
    }

    #[test]
    fn moved_over_unselected_tab_sets_hover_without_selection_or_msg() {
        let (mut panel, _) = drawn_panel(80, titles(), vec![false; 5], 0, 0);
        let (rect, position) = panel.hit_regions()[1];
        let msg = panel.on(&Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Moved,
            column: rect.x,
            row: rect.y,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }));
        assert_eq!(msg, None);
        assert_eq!(panel.hovered, Some(position));
        assert_eq!(panel.selected, 0);
    }
}
