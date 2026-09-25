//! `StatusBarPanel` — the mounted Interactive Component for the status row
//! (`RootFrame.status_bar` placement, task 2.2; design D10).
//!
//! Owns the status row's pointer regions: the volume pill (scroll adjusts
//! the volume) and the mute pill (click toggles mute). The remote/session
//! pill region is retained verbatim, but it has no click dispatch:
//! `show_session_pill` is hard-coded `false` (preserved from the base
//! frame), so the pill never paints and the region stays `None`. The
//! pill/right-segment spans are shell-produced content (`chrome_status.rs`),
//! projected one-way by `Model::sync_status_bar_panel`; the component owns
//! the overflow drop-order, pill geometry and event resolution.
//!
//! The Local/Remote queue-scope pills are queue concern and live in the
//! QueueColumn footer (`QueueComponent`), never here.

use ratatui::layout::{Position, Rect};
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::media_list::SelectionOrigin;
use super::msg::{Msg, PlaybackRequest, ShellRequest};
use super::user_event::UserEvent;
use crate::app::dispatch::action::VOLUME_STEP;
use crate::app::render::{render_status_bar, StatusBarModel, StatusBarRegions};

/// The status row panel: paints the status row where `RootFrame` places it
/// and retains its volume/mute/remote pill regions.
pub struct StatusBarPanel {
    model: StatusBarModel,
    regions: StatusBarRegions,
    visual_origin: SelectionOrigin,
}

impl StatusBarPanel {
    pub fn new() -> Self {
        Self {
            model: StatusBarModel::default(),
            regions: StatusBarRegions::default(),
            visual_origin: SelectionOrigin::Queue,
        }
    }

    /// Project the shell-produced status-row content (task 2.2).
    pub(in crate::app) fn set_model(&mut self, model: StatusBarModel) {
        self.model = model;
    }

    /// The pill regions retained from the last paint (test accessor).
    #[cfg(test)]
    pub(in crate::app) fn regions(&self) -> StatusBarRegions {
        self.regions
    }

    pub(in crate::app) fn set_visual_origin(&mut self, origin: SelectionOrigin) {
        self.visual_origin = origin;
    }

    fn handle_mouse(&mut self, event: &tuirealm::event::MouseEvent) -> Option<Msg> {
        let at = Position::new(event.column, event.row);
        match event.kind {
            // Legacy wheel mapping: scroll down lowers the volume by the
            // shared `VOLUME_STEP`, the same `Command::AdjustVolume` step
            // the `-`/`+` keys dispatch.
            MouseEventKind::ScrollDown if self.regions.volume.is_some_and(|r| r.contains(at)) => {
                Some(Msg::Playback(PlaybackRequest::VolumeDelta(-VOLUME_STEP)))
            }
            MouseEventKind::ScrollUp if self.regions.volume.is_some_and(|r| r.contains(at)) => {
                Some(Msg::Playback(PlaybackRequest::VolumeDelta(VOLUME_STEP)))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.regions.mute.is_some_and(|r| r.contains(at)) =>
            {
                // Same `Command::ToggleMute` the `m` key dispatches.
                Some(Msg::Playback(PlaybackRequest::ToggleMute))
            }
            // The visual-mode indicator clears the multi-selection; the
            // Local/Remote queue-scope pills live in the QueueColumn footer
            // (`QueueComponent`), so the status row has no scope dispatch.
            MouseEventKind::Down(MouseButton::Left)
                if self.regions.visual_clear.is_some_and(|r| r.contains(at)) =>
            {
                Some(Msg::Shell(Box::new(ShellRequest::ClearMultiSelection(
                    self.visual_origin.clone(),
                ))))
            }
            _ => None,
        }
    }
}

impl Default for StatusBarPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StatusBarPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        self.regions = render_status_bar(frame, area, &self.model);
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

impl AppComponent<Msg, UserEvent> for StatusBarPanel {
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            // Resolve only geometry this panel painted.
            Event::Mouse(mouse) => self.handle_mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::style::{Color, Style};
    use ratatui::text::Span;
    use ratatui::Terminal;

    use crate::app::palette;

    fn pill(text: impl Into<String>, fg: Color) -> Vec<Span<'static>> {
        let fill = palette::surface_colors(palette::Surface::StatusBarPill, false).fill;
        vec![
            Span::styled(" ", Style::default().bg(fill)),
            Span::styled(text.into(), Style::default().fg(fg).bg(fill)),
            Span::styled(" ", Style::default().bg(fill)),
        ]
    }

    fn model(volume: Vec<Span<'static>>, mute: Option<Vec<Span<'static>>>) -> StatusBarModel {
        StatusBarModel {
            show_session_pill: false,
            remote: Vec::new(),
            mute,
            volume,
            right: pill("R", Color::White),
            visual_mode: None,
            prefix_armed: None,
        }
    }

    fn drawn_panel(
        width: u16,
        volume: Vec<Span<'static>>,
        mute: Option<Vec<Span<'static>>>,
    ) -> (StatusBarPanel, ratatui::buffer::Buffer) {
        let mut panel = StatusBarPanel::new();
        panel.set_model(model(volume, mute));
        let mut terminal = Terminal::new(TestBackend::new(width, 1)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, width, 1)))
            .unwrap();
        (panel, terminal.backend().buffer().clone())
    }

    /// The moved painter's characterization: volume and mute pills paint on
    /// the left, the right segment on the right, each region matching the
    /// painted pill.
    #[test]
    fn status_row_paints_pills_and_retains_their_regions() {
        let (panel, buffer) = drawn_panel(
            60,
            pill(" 60", palette::ACCENT),
            Some(pill("muted", palette::STATUS_ERROR)),
        );
        let row: String = (0..60).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(row.contains(" 60"), "volume pill row: {row:?}");
        assert!(row.contains("muted"), "mute pill row: {row:?}");
        assert!(row.contains("R"), "right segment row: {row:?}");

        let regions = panel.regions();
        let vol = regions.volume.expect("volume region");
        let mute = regions.mute.expect("mute region");
        assert_eq!(vol.y, 0);
        assert_eq!(mute.y, 0);
        // Regions sit over the painted pills, in painted order.
        assert!(vol.x < mute.x, "volume precedes mute: {vol:?} {mute:?}");
        assert_eq!(
            vol.width, 5,
            "space + speaker glyph + ` 60` (unicode width)"
        );
        let volume_text: String = (vol.x..vol.right())
            .map(|x| buffer[(x, 0)].symbol())
            .collect();
        assert!(
            volume_text.contains("60"),
            "volume pill row: {volume_text:?}"
        );
        let mute_text: String = (mute.x..mute.right())
            .map(|x| buffer[(x, 0)].symbol())
            .collect();
        assert!(mute_text.contains("muted"), "mute pill row: {mute_text:?}");
    }

    /// Overflow (characterization of the moved painter): the volume pill
    /// drops only when it cannot fit; the mute pill paints after it even
    /// when the row overflows (the left paragraph truncates it), matching
    /// the legacy `ind_mu` behaviour verbatim.
    #[test]
    fn status_row_overflow_keeps_the_legacy_drop_order() {
        // Narrow enough that volume+mute cannot both fit: both regions are
        // still retained (the mute rect extends past the row's right edge
        // and its paint truncates, as the legacy published rects did).
        let (panel, buffer) = drawn_panel(
            8,
            pill(" 60", palette::ACCENT),
            Some(pill("muted", palette::STATUS_ERROR)),
        );
        let regions = panel.regions();
        let vol = regions.volume.expect("volume fits");
        assert_eq!(vol.width, 5);
        let mute = regions.mute.expect("mute region retained verbatim");
        assert_eq!(mute.x, vol.right(), "mute follows the painted volume pill");
        let row: String = (0..8).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(
            !row.contains("muted"),
            "row truncates the mute pill: {row:?}"
        );

        // Narrower still: volume drops, and mute with it.
        let (panel, _) = drawn_panel(4, pill(" 60", palette::ACCENT), None);
        let regions = panel.regions();
        assert!(regions.volume.is_none(), "volume drops next");
        assert!(regions.mute.is_none());
    }

    /// Scrolling on the volume pill emits the volume intent; scrolling
    /// anywhere else on the row does not.
    #[test]
    fn scroll_on_the_volume_pill_emits_the_volume_intent() {
        let (mut panel, _) = drawn_panel(60, pill(" 60", palette::ACCENT), None);
        let vol = panel.regions().volume.expect("volume region");
        let mouse = |kind, column, row| {
            Event::Mouse(tuirealm::event::MouseEvent {
                kind,
                column,
                row,
                modifiers: tuirealm::event::KeyModifiers::NONE,
            })
        };
        assert_eq!(
            panel.on(&mouse(MouseEventKind::ScrollDown, vol.x + 1, vol.y)),
            Some(Msg::Playback(PlaybackRequest::VolumeDelta(-5))),
            "scroll down lowers the volume (legacy wheel mapping)"
        );
        assert_eq!(
            panel.on(&mouse(MouseEventKind::ScrollUp, vol.x + 1, vol.y)),
            Some(Msg::Playback(PlaybackRequest::VolumeDelta(5))),
        );
        assert_eq!(
            panel.on(&mouse(MouseEventKind::ScrollDown, 59, 0)),
            None,
            "scroll outside the pill is a no-op"
        );
    }

    /// Clicking the mute pill emits the mute intent (the same `Command` the
    /// `m` key dispatches); clicking the row outside any pill does not.
    #[test]
    fn click_on_the_mute_pill_emits_the_mute_intent() {
        let (mut panel, _) = drawn_panel(
            60,
            pill(" 60", palette::ACCENT),
            Some(pill("muted", palette::STATUS_ERROR)),
        );
        let mute = panel.regions().mute.expect("mute region");
        let mouse = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: mute.x + 1,
            row: mute.y,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        });
        assert_eq!(
            panel.on(&mouse),
            Some(Msg::Playback(PlaybackRequest::ToggleMute))
        );
        let outside = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 59,
            row: 0,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        });
        assert_eq!(
            panel.on(&outside),
            None,
            "click outside any pill is a no-op"
        );
    }

    /// The retained remote/session region has no click dispatch: even a
    /// hand-built model that shows the pill (the production projection
    /// always passes `show_session_pill: false`) resolves a click to `None`.
    #[test]
    fn remote_pill_region_stays_absent_without_the_session_pill() {
        let mut panel = StatusBarPanel::new();
        let mut m = model(pill(" 60", palette::ACCENT), None);
        m.show_session_pill = true;
        m.remote = pill("HOST", Color::White);
        panel.set_model(m);
        let mut terminal = Terminal::new(TestBackend::new(80, 1)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, 80, 1)))
            .unwrap();
        let remote = panel.regions().remote.expect("remote region shown");
        let mouse = Event::Mouse(tuirealm::event::MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: remote.x + 1,
            row: remote.y,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        });
        assert_eq!(
            panel.on(&mouse),
            None,
            "the retained remote region has no click dispatch"
        );
    }

    /// The status row paints only where it is placed: with a narrow
    /// placement inside a wider frame, nothing outside the placement is
    /// touched.
    #[test]
    fn status_row_paints_only_inside_its_placement() {
        let mut panel = StatusBarPanel::new();
        panel.set_model(model(pill(" 60", palette::ACCENT), None));
        let mut terminal = Terminal::new(TestBackend::new(40, 3)).unwrap();
        let placement = Rect::new(5, 2, 20, 1);
        terminal.draw(|f| panel.view(f, placement)).unwrap();
        let buffer = terminal.backend().buffer();
        for y in 0..3 {
            for x in 0..40 {
                if placement.contains(Position::new(x, y)) {
                    continue;
                }
                assert_eq!(
                    buffer[(x, y)].bg,
                    ratatui::style::Color::Reset,
                    "cell ({x}, {y}) outside the status placement must stay untouched"
                );
            }
        }
    }
}
