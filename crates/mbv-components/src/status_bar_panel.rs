//! `StatusBarPanel` — the mounted Interactive Component for the status row
//! (`RootFrame.status_bar` placement, task 2.2; design D10).
//!
//! Owns the status row's pointer regions: the volume pill (scroll adjusts
//! the volume) and the mute pill (click toggles mute). The
//! pill/right-segment spans are shell-produced content (`chrome_status.rs`),
//! projected one-way by `Model::sync_status_bar_panel`; the component owns
//! the overflow drop-order, pill geometry and event resolution. The
//! prefix-armed display and the toast presentation are the Status bar's
//! Surface state (extract-client-core row 4.6): the shell projects the armed
//! flag and the live toast into it, and this adapter builds the painted
//! chrome from the state at paint time.
//!
//! The Local/Peer queue-link pills are queue concern and live in the
//! `QueueColumn` footer (`QueueComponent`), never here.

use ratatui::Frame;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use mbv_render::{StatusBarChrome, StatusBarModel, StatusBarRegions, render_status_bar};
use mbv_surfaces::HitMap;
use mbv_surfaces::status_bar::{StatusBarFrameReport, StatusBarHitTarget, StatusBarSurface};
use mbv_theme as palette;
use mbv_ui_model::media_list::SelectionOrigin;
use mbv_ui_model::playback::VOLUME_STEP;
use mbv_ui_model::toast::{StatusToast, ToastSeverity};
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{Msg, PlaybackRequest, ShellRequest};

/// The status row panel: paints the status row where `RootFrame` places it
/// and retains its volume/mute pill regions.
#[derive(Debug)]
pub struct StatusBarPanel {
    /// The Status bar's Surface state (extract-client-core row 4.6): the
    /// prefix-armed display state and the toast presentation. The adapter
    /// paints from it and holds no copy of a state value.
    surface: StatusBarSurface,
    model: StatusBarModel,
    regions: StatusBarRegions,
    visual_origin: SelectionOrigin,
}

impl StatusBarPanel {
    #[must_use]
    pub fn new() -> Self {
        Self {
            surface: StatusBarSurface::new(),
            model: StatusBarModel::default(),
            regions: StatusBarRegions::default(),
            visual_origin: SelectionOrigin::Queue,
        }
    }

    /// Project the shell-produced status-row content (task 2.2).
    pub fn set_model(&mut self, model: StatusBarModel) {
        self.model = model;
    }

    /// Project the prefix-armed display state (extract-client-core row 4.6).
    pub fn set_prefix_armed(&mut self, armed: bool) {
        self.surface.set_prefix_armed(armed);
    }

    /// Project the live toast presentation (extract-client-core row 4.6);
    /// `None` presents no toast.
    pub fn set_toast(&mut self, toast: Option<StatusToast>) {
        self.surface.set_toast(toast);
    }

    /// The pill regions retained from the last paint (test accessor).
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn regions(&self) -> StatusBarRegions {
        self.regions
    }

    pub fn set_visual_origin(&mut self, origin: SelectionOrigin) {
        self.visual_origin = origin;
    }

    /// The current frame's interaction report (design D6): the hit map over
    /// the bar's painted pill regions, resolved in the same retained
    /// geometry the pointer path resolves against. The shell and the Client
    /// core read this report; neither recomputes the geometry.
    #[must_use]
    pub fn frame_report(&self) -> StatusBarFrameReport {
        let mut hit_map = HitMap::new();
        for (region, target) in [
            (self.regions.volume, StatusBarHitTarget::VolumePill),
            (self.regions.mute, StatusBarHitTarget::MutePill),
            (self.regions.visual_clear, StatusBarHitTarget::VisualClear),
        ] {
            if let Some(rect) = region {
                hit_map.push(crate::media_list::hit_region(rect), target);
            }
        }
        StatusBarFrameReport { hit_map }
    }

    /// The status chrome built from the Surface state at paint time (row
    /// 4.6): the armed pill, present only while prefix mode is armed, and
    /// the live toast's severity-styled spans. The pill is non-interactive:
    /// any mouse event already disarms prefix mode silently, so it carries
    /// no hit region. The toast span carries only the severity foreground;
    /// the row background comes from the painter's `bar_style`.
    fn chrome(&self) -> StatusBarChrome {
        StatusBarChrome {
            prefix_armed: self.surface.prefix_armed().then(Self::prefix_armed_pill),
            toast: self.surface.toast().map(toast_spans),
        }
    }

    /// The prefix-armed pill (design D7, task 7.3): styled like the other
    /// left-segment pills.
    fn prefix_armed_pill() -> Vec<Span<'static>> {
        vec![
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill
                .to_ratatui()),
            ),
            Span::styled(
                " PREFIX ",
                Style::default()
                    .fg(palette::Role::Accent.color().to_ratatui())
                    .bg(
                        palette::surface_colors(palette::Surface::StatusBarPill, false)
                            .fill
                            .to_ratatui(),
                    )
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " ",
                Style::default().bg(palette::surface_colors(
                    palette::Surface::StatusBarPill,
                    false,
                )
                .fill
                .to_ratatui()),
            ),
        ]
    }

    fn handle_mouse(&mut self, event: tuirealm::event::MouseEvent) -> Option<Msg> {
        let at = Position::new(event.column, event.row);
        match event.kind {
            // Legacy wheel mapping: scroll down lowers the volume by the
            // shared `VOLUME_STEP`, the same `PlaybackIntent::AdjustVolume` step
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
                // Same `PlaybackIntent::ToggleMute` the `m` key dispatches.
                Some(Msg::Playback(PlaybackRequest::ToggleMute))
            }
            // The visual-mode indicator clears the multi-selection; the
            // Local/Peer queue-link pills live in the QueueColumn footer
            // (`QueueComponent`), so the status row has no link dispatch.
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

/// The live toast's severity-styled spans: the severity foreground on the
/// status-bar surface, the row background from the painter's `bar_style`
/// (the projection the shell used to build, moved to the adapter with the
/// Surface state in row 4.6).
fn toast_spans(toast: &StatusToast) -> Vec<Span<'static>> {
    let fg = match toast.severity {
        ToastSeverity::Neutral => palette::Role::TextPrimary,
        ToastSeverity::Success => palette::Role::StatusAvailable,
        ToastSeverity::Warning => palette::Role::TextFocusAccent,
        ToastSeverity::Error => palette::Role::StatusError,
    }
    .color()
    .to_ratatui();
    vec![Span::styled(toast.text.clone(), Style::default().fg(fg))]
}

impl Default for StatusBarPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for StatusBarPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        let chrome = self.chrome();
        self.regions = render_status_bar(frame, area, &self.model, &chrome);
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
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;

    use mbv_theme as palette;

    fn pill(text: impl Into<String>, fg: Color) -> Vec<Span<'static>> {
        let fill = palette::surface_colors(palette::Surface::StatusBarPill, false)
            .fill
            .to_ratatui();
        vec![
            Span::styled(" ", Style::default().bg(fill)),
            Span::styled(text.into(), Style::default().fg(fg).bg(fill)),
            Span::styled(" ", Style::default().bg(fill)),
        ]
    }

    fn model(volume: Vec<Span<'static>>, mute: Option<Vec<Span<'static>>>) -> StatusBarModel {
        StatusBarModel {
            mute,
            volume,
            right: pill("R", Color::White),
            visual_mode: None,
        }
    }

    /// A live toast takes over the entire row: the toast text paints and
    /// the pills (and their regions) drop for the toast's duration, so
    /// pointer events are never resolved against hidden pills (spec
    /// `toast-notification-semantics`; restored in-app toast surface).
    #[test]
    fn a_live_toast_takes_over_the_status_row() {
        let model = model(
            pill(" 60", palette::Role::Accent.color().to_ratatui()),
            None,
        );
        let mut panel = StatusBarPanel::new();
        panel.set_model(model);
        panel.set_toast(Some(StatusToast {
            text: "Scanning 'Movies'...".into(),
            severity: ToastSeverity::Neutral,
        }));
        let mut terminal = Terminal::new(TestBackend::new(60, 1)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, 60, 1)))
            .unwrap();
        let buffer = terminal.backend().buffer().clone();

        let row: String = (0..60)
            .filter_map(|x| buffer[(x, 0)].symbol().chars().next())
            .collect();
        assert!(
            row.contains("Scanning 'Movies'..."),
            "the live toast paints on the status row: {row:?}"
        );
        assert!(
            !row.contains("60"),
            "the pills drop while the toast is live: {row:?}"
        );
        assert!(
            panel.regions().volume.is_none() && panel.regions().mute.is_none(),
            "no pill regions are retained while the toast is live"
        );
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
        let (mut panel, _) = drawn_panel(
            60,
            pill(" 60", palette::Role::Accent.color().to_ratatui()),
            None,
        );
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
            pill(" 60", palette::Role::Accent.color().to_ratatui()),
            Some(pill(
                "muted",
                palette::Role::StatusError.color().to_ratatui(),
            )),
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

    /// The frame report resolves the painted pill regions to their semantic
    /// targets (design D6; the control hit maps row 6.5 paints from), while
    /// a live toast's row — no painted pills — reports none.
    #[test]
    fn frame_report_resolves_the_painted_pill_regions() {
        let (panel, _) = drawn_panel(
            60,
            pill(" 60", palette::Role::Accent.color().to_ratatui()),
            Some(pill(
                "muted",
                palette::Role::StatusError.color().to_ratatui(),
            )),
        );
        let at = |x: u16, y: u16| mbv_ui_msg::Position {
            x: f32::from(x),
            y: f32::from(y),
        };
        let resolve = |panel: &StatusBarPanel, x: u16, y: u16| {
            panel.frame_report().hit_map.resolve(at(x, y)).copied()
        };
        let volume = panel.regions().volume.expect("volume region");
        let mute = panel.regions().mute.expect("mute region");
        assert_eq!(
            resolve(&panel, volume.x + 1, volume.y),
            Some(StatusBarHitTarget::VolumePill),
        );
        assert_eq!(
            resolve(&panel, mute.x + 1, mute.y),
            Some(StatusBarHitTarget::MutePill),
        );
        assert_eq!(resolve(&panel, 59, 0), None, "no pill painted there");

        // A live toast takes the whole row: the pills drop and their
        // regions resolve nothing.
        let mut panel = StatusBarPanel::new();
        panel.set_model(model(
            pill(" 60", palette::Role::Accent.color().to_ratatui()),
            None,
        ));
        panel.set_toast(Some(StatusToast {
            text: "Scanning 'Movies'...".into(),
            severity: ToastSeverity::Neutral,
        }));
        let mut terminal = Terminal::new(TestBackend::new(60, 1)).unwrap();
        terminal
            .draw(|f| panel.view(f, Rect::new(0, 0, 60, 1)))
            .unwrap();
        assert!(panel.frame_report().hit_map.is_empty());
    }
}
