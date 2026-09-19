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
        selected: usize,
        scroll: usize,
    ) {
        self.titles = titles;
        self.selected = selected;
        self.scroll = scroll;
    }

    /// The hit regions retained from the last paint (test accessor).
    #[cfg_attr(not(test), allow(dead_code))]
    pub(in crate::app) fn hit_regions(&self) -> &[(Rect, usize)] {
        &self.hits
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
                selected: self.selected,
                scroll: self.scroll,
                hovered: self.hovered,
            },
            &mut self.hits,
        );
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

impl AppComponent<Msg, UserEvent> for TabPanel {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
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
                    .map(|tab_pos| Msg::Shell(ShellRequest::TabSelect(tab_pos)))
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
        ["Home", "Movies", "TV Shows", "Music", "Feeds"]
            .into_iter()
            .map(String::from)
            .collect()
    }

    fn drawn_panel(
        width: u16,
        selected: usize,
        scroll: usize,
    ) -> (TabPanel, ratatui::buffer::Buffer) {
        let mut panel = TabPanel::new();
        panel.set_content(titles(), selected, scroll);
        let mut terminal = Terminal::new(TestBackend::new(width, 3)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, width, 3)))
            .unwrap();
        (panel, terminal.backend().buffer().clone())
    }

    /// The moved painter's characterization: the selected tab is highlighted
    /// with the accent marker, unselected tabs are muted uppercase labels.
    #[test]
    fn tab_bar_paints_selected_and_muted_labels() {
        let (_, buffer) = drawn_panel(80, 1, 0);
        let row: String = (0..80).map(|x| buffer[(x, 1)].symbol()).collect();
        assert!(row.contains("▐ MOVIES"), "selected tab row: {row:?}");
        assert!(row.contains("  HOME  "), "muted home tab row: {row:?}");
    }

    /// Overflow arrows: with all tabs fitting, neither arrow paints; a narrow
    /// strip scrolled past the first tab shows the left arrow, and the last
    /// tab overflowing shows the right arrow.
    #[test]
    fn tab_bar_overflow_arrows_follow_the_visible_window() {
        let (_, wide) = drawn_panel(120, 0, 0);
        let wide_row: String = (0..120).map(|x| wide[(x, 1)].symbol()).collect();
        assert!(!wide_row.contains('«') && !wide_row.contains('»'));

        let (panel, narrow) = drawn_panel(30, 4, 1);
        let row: String = (0..30).map(|x| narrow[(x, 1)].symbol()).collect();
        assert!(
            row.contains('«'),
            "scrolled strip shows the left arrow: {row:?}"
        );
        assert!(
            panel.hit_regions().iter().all(|(_, pos)| *pos != 0),
            "scrolled-out tabs are not hit targets"
        );
    }

    /// Hit regions are retained at the painted tab labels; a click on one
    /// emits the tab-select `Msg` with the tab's position.
    #[test]
    fn click_on_a_painted_tab_emits_tab_select() {
        let (panel, _) = drawn_panel(80, 0, 0);
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
            Some(Msg::Shell(ShellRequest::TabSelect(2))),
            "clicking the third tab selects it"
        );
    }

    #[test]
    fn moved_over_unselected_tab_sets_hover_without_selection_or_msg() {
        let (mut panel, _) = drawn_panel(80, 0, 0);
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

    #[test]
    fn moved_into_gap_clears_hover_without_selection_or_msg() {
        let (mut panel, _) = drawn_panel(80, 0, 0);
        let (rect, _) = panel.hit_regions()[1];
        panel.hovered = Some(1);
        let msg = panel.on(&Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Moved,
            column: rect.x,
            row: 0,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }));
        assert_eq!(msg, None);
        assert_eq!(panel.hovered, None);
        assert_eq!(panel.selected, 0);
    }

    #[test]
    fn moved_over_selected_tab_sets_hover_without_changing_selection() {
        let (mut panel, _) = drawn_panel(80, 2, 0);
        let (rect, position) = panel.hit_regions()[2];
        let msg = panel.on(&Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Moved,
            column: rect.x,
            row: rect.y,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        }));
        assert_eq!(msg, None);
        assert_eq!(panel.hovered, Some(position));
        assert_eq!(panel.selected, 2);
    }

    #[test]
    fn unselected_hover_strengthens_label_while_selected_style_wins() {
        let (mut panel, _) = drawn_panel(80, 0, 0);
        let unselected = panel.hit_regions()[1].0;
        let selected = panel.hit_regions()[0].0;
        panel.hovered = Some(1);
        let mut terminal = Terminal::new(TestBackend::new(80, 3)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, 80, 3)))
            .unwrap();
        let buffer = terminal.backend().buffer();
        assert_eq!(
            buffer[(unselected.x + 2, unselected.y)].fg,
            palette::TEXT_STRONG
        );
        assert_eq!(
            buffer[(selected.x + 1, selected.y)].fg,
            palette::TEXT_STRONG,
            "selected styling remains unchanged when another tab is hovered"
        );
        assert!(buffer[(selected.x + 1, selected.y)]
            .modifier
            .contains(ratatui::style::Modifier::BOLD));
    }

    /// A click outside the painted tab labels — including the overflow
    /// arrows — resolves to nothing.
    #[test]
    fn click_outside_the_tab_labels_is_a_noop() {
        let (mut panel, _) = drawn_panel(30, 4, 1);
        for at in [(0u16, 0u16), (0, 1), (29, 2)] {
            assert_eq!(
                panel.on(&Event::Mouse(tuirealm::event::MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: at.0,
                    row: at.1,
                    modifiers: tuirealm::event::KeyModifiers::NONE,
                })),
                None,
                "click at {at:?} must not select a tab"
            );
        }
    }

    /// The panel fills its own placement with the tab bar's surface: every
    /// cell of `RootFrame.tab` carries the TabBar fill (the full-column
    /// backdrop underneath stays until task 12.1).
    #[test]
    fn tab_panel_fills_its_placement_with_its_surface() {
        let (_, buffer) = drawn_panel(60, 0, 0);
        let fill = palette::surface_colors(palette::Surface::TabBar, false).fill;
        for y in 0..3 {
            for x in 0..60 {
                assert_eq!(
                    buffer[(x, y)].bg,
                    fill,
                    "tab placement cell ({x}, {y}) must carry the TabBar surface"
                );
            }
        }
    }
}
