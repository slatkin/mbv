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
//! `QueueColumn` footer (`QueueComponent`), never here.

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

    fn handle_mouse(&mut self, event: tuirealm::event::MouseEvent) -> Option<Msg> {
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

impl AppComponent<Msg, UserEvent> for StatusBarPanel {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            // Resolve only geometry this panel painted.
            Event::Mouse(mouse) => self.handle_mouse(*mouse),
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
}
