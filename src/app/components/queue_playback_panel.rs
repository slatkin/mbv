//! The Queue playback panel (task 3.5, design D10): the queue column's
//! playback surface, mounted in every queue-visible layout, idle included.
//! It owns the always-painted header row (status left, `on <host>` right),
//! the visual slot's region (painted by the shell's App-side slot adapter on
//! the panel's behalf — the ABS `paint_home_image` seam) and the queue-column
//! transport presentation, which routes through the shared width-driven
//! transport arrangement (`render/arrangements/playback_transport.rs`), the
//! same arrangement the Library playback panel's strip calls (task 4.1).
//!
//! While idle the panel paints only the header row: its visual slot and
//! transport reserve zero rows, and the shell publishes that collapse
//! (task 3.6). Transport hit geometry is the panel's own retained
//! state (task 3.7) — the legacy playback geometry side channel is not read here.

use std::time::Instant;

use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::library_playback_panel::PlaybackProjection;
use super::msg::{Msg, PlaybackRequest};
use super::user_event::UserEvent;
use crate::app::palette;
use crate::app::render::arrangements::chrome::PLAYER_BOX_HEIGHT;
use crate::app::render::components::widgets::queue_panel_inset;
use crate::app::render::PlaybackStripAreas;
use crate::app::render::{render_playback_header, render_player_panel, PlaybackRenderContext};
use crate::app::NowPlayingStatus;

/// The queue-column transport's surface: the fixed chrome band the queue
/// playback panel paints in every queue-visible layout (the former
/// queue-only strip's identity, D10).
const TRANSPORT_SURFACE: palette::Surface = palette::Surface::QueueOnlyPlaybackPanel;

pub struct QueuePlaybackPanel {
    /// The header's projected facts: status word left, playback target
    /// right (`App::playback_host_label_and_remote`, no tracking suffix)
    /// with its remote flag for the hostname colour.
    status: NowPlayingStatus,
    host: String,
    host_is_remote: bool,
    /// The transport's projected facts (the shared transport projection).
    transport: PlaybackProjection,
    /// The transport rect the shell computes in the sync pass from the
    /// prior-paint card checkpoint (task 3.5). Draw is read-only with respect
    /// to panel layout state. `None` while idle (task 3.6) or on a degenerate
    /// rect.
    transport_area: Option<Rect>,
    /// The panel's own retained transport hit geometry (task 3.7): cleared
    /// whenever the transport does not paint, so a collapsed panel resolves
    /// nothing.
    play_pause_area: Rect,
    stop_area: Rect,
    next_area: Rect,
    prev_area: Rect,
    seekbar_area: Rect,
    /// Panel-local marquee state (the title row's scrolling title).
    marquee_text: String,
    marquee_started_at: Instant,
    props: tuirealm::props::Props,
}

impl QueuePlaybackPanel {
    pub fn new() -> Self {
        Self {
            status: NowPlayingStatus::Idle,
            host: String::new(),
            host_is_remote: false,
            transport: PlaybackProjection {
                state: crate::app::state::types::playback::PlaybackState::default(),
                show_controls: false,
                panel: TRANSPORT_SURFACE,
                panel_focused: false,
                now_playing_title: None,
                title_parts: None,
                status_indicators: None,
                use_nerd_fonts: false,
                idle_feed_title: None,
                availability: super::library_playback_panel::TransportAvailability::default(),
            },
            transport_area: None,
            play_pause_area: Rect::default(),
            stop_area: Rect::default(),
            next_area: Rect::default(),
            prev_area: Rect::default(),
            seekbar_area: Rect::default(),
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
            props: tuirealm::props::Props::default(),
        }
    }

    /// Project the header row's facts (status word, playback target and
    /// its remote flag for the hostname colour).
    pub(in crate::app) fn set_header(
        &mut self,
        status: NowPlayingStatus,
        host: String,
        host_is_remote: bool,
    ) {
        self.status = status;
        self.host = host;
        self.host_is_remote = host_is_remote;
    }

    /// Project the transport's facts (the shared transport projection, with
    /// the panel's own chrome-band surface identity).
    pub(in crate::app) fn set_transport(&mut self, transport: PlaybackProjection) {
        self.transport = transport;
    }

    /// Project the transport rect computed by the shell in the sync pass from
    /// the prior-paint card checkpoint. `None` collapses the transport and
    /// clears the retained hit geometry (task 3.6: idle paints only the header
    /// row and resolves nothing).
    pub(in crate::app) fn set_transport_area(&mut self, area: Option<Rect>) {
        let painted = area.filter(|r| r.width > 0 && r.height > 0);
        if painted.is_none() {
            self.play_pause_area = Rect::default();
            self.stop_area = Rect::default();
            self.next_area = Rect::default();
            self.prev_area = Rect::default();
            self.seekbar_area = Rect::default();
        }
        self.transport_area = painted;
    }

    /// Test-only: the retained transport hit geometry (task 3.7).
    #[cfg(test)]
    pub(in crate::app) fn transport_hits(&self) -> (Rect, Rect) {
        (self.play_pause_area, self.seekbar_area)
    }

    /// Test-only: the retained prev/next transport hit rects.
    #[cfg(test)]
    pub(in crate::app) fn transport_nav_hits(&self) -> (Rect, Rect) {
        (self.prev_area, self.next_area)
    }

    /// Test-only: the sync-projected transport area.
    #[cfg(test)]
    pub(in crate::app) fn transport_area_for_test(&self) -> Option<Rect> {
        self.transport_area
    }

    /// Test-only: the projected now-playing title parts the sync pass
    /// delivered (task 5.2).
    #[cfg(test)]
    pub(in crate::app) fn transport_title_parts_for_test(
        &self,
    ) -> Option<mbv_core::playback_queue::PlaybackTitleParts> {
        self.transport.title_parts.clone()
    }

    fn mouse(&self, event: MouseEvent) -> Option<Msg> {
        let point = (event.column, event.row).into();
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if self.play_pause_area.contains(point) => {
                Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.stop_area.contains(point) && self.transport.availability.stop =>
            {
                Some(Msg::Playback(PlaybackRequest::Stop))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.prev_area.contains(point) && self.transport.availability.previous =>
            {
                Some(Msg::Playback(PlaybackRequest::Previous))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.next_area.contains(point) && self.transport.availability.next =>
            {
                Some(Msg::Playback(PlaybackRequest::Next))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.seekbar_area.contains(point) && self.seekbar_area.width > 0 =>
            {
                let fraction = f64::from(event.column.saturating_sub(self.seekbar_area.x))
                    / f64::from(self.seekbar_area.width);
                Some(Msg::Playback(PlaybackRequest::SeekTo(fraction)))
            }
            _ => None,
        }
    }
}

impl Default for QueuePlaybackPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for QueuePlaybackPanel {
    fn view(&mut self, frame: &mut Frame, area: Rect) {
        // The header row is the placement's painted row, painted in every
        // queue-visible layout, idle included (D10). It sits one row down
        // under the column's recessed top padding and is inset two columns
        // each side (the queue column's canonical content inset), so it is
        // not flush with the column's top, left, or right edge. Bottom
        // padding stays zero: the slot/transport band follows it directly.
        let header = Rect {
            height: 1,
            ..queue_panel_inset(area)
        };
        render_playback_header(frame, header, self.status, &self.host, self.host_is_remote);
        // While idle — or whenever the shell hands no transport rect — the
        // slot and transport rows are already collapsed (task 3.6); the
        // panel paints nothing else and its hit geometry stays cleared.
        let Some(transport_area) = self.transport_area else {
            return;
        };
        // The transport band fills its whole rect before the four transport
        // rows paint over it: the side-by-side slot's leftover rows (a slot
        // taller than the four transport rows) stay on the chrome band.
        frame.render_widget(
            Block::default()
                .style(Style::default().bg(palette::surface_colors(TRANSPORT_SURFACE, false).fill)),
            transport_area,
        );
        // The painted row budget: the three base transport rows, plus the
        // expanded title band's fourth row while the projected title carries
        // a context part (the shell sized the band for the same condition).
        let title_expanded = self
            .transport
            .title_parts
            .as_ref()
            .is_some_and(|parts| parts.context.is_some());
        let player_h = transport_area
            .height
            .min(PLAYER_BOX_HEIGHT + u16::from(title_expanded));
        let mut playback = PlaybackStripAreas::default();
        render_player_panel(
            frame,
            PlaybackRenderContext {
                area: transport_area,
                playback: &mut playback,
                player_h,
                controls: crate::app::render::PlaybackControls {
                    show: self.transport.show_controls,
                    use_nerd_fonts: self.transport.use_nerd_fonts,
                    availability: self.transport.availability,
                    panel_focused: false,
                    progress: (
                        self.transport.state.position_ticks,
                        self.transport.state.runtime_ticks,
                        self.transport.state.paused,
                    ),
                    idle_feed_title: None,
                },
                now_playing_title: self.transport.now_playing_title.clone(),
                panel: TRANSPORT_SURFACE,
                status_indicators: self.transport.status_indicators.clone(),
                title_parts: self.transport.title_parts.clone(),
                // The queue column never shows the idle feed title: while
                // idle the panel is collapsed to its header row (task 3.6),
                // and the feed title's only open-link gate re-points at the
                // panel's presence (task 3.8).
                marquee_text: &mut self.marquee_text,
                marquee_started_at: &mut self.marquee_started_at,
            },
        );
        self.play_pause_area = playback.play_pause;
        self.stop_area = playback.stop;
        self.next_area = playback.next;
        self.prev_area = playback.prev;
        self.seekbar_area = playback.seekbar;
    }

    fn query(&self, attr: Attribute) -> Option<QueryResult<'_>> {
        self.props.get_for_query(attr)
    }
    fn attr(&mut self, attr: Attribute, value: AttrValue) {
        self.props.set(attr, value);
    }
    fn state(&self) -> State {
        State::None
    }
    fn perform(&mut self, _cmd: Cmd) -> CmdResult {
        CmdResult::NoChange
    }
}

impl AppComponent<Msg, UserEvent> for QueuePlaybackPanel {
    fn on(&mut self, ev: &Event<UserEvent>) -> Option<Msg> {
        match ev {
            Event::Mouse(mouse) => self.mouse(*mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::playback_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;
    use rstest::rstest;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};

    /// One painted band row: its text and each cell's foreground.
    type PaintedRow = (String, Vec<Color>);

    /// The media-type families of the requirements table, as the projection
    /// carries them (title part, optional context part). The mapping itself is
    /// core's table (task 1.1); these fixtures pin the painted behaviour per
    /// media type — on the queue column's split lower title row, the same
    /// content the Library strip paints.
    fn parts_for(title: &str, context: Option<&str>) -> PlaybackTitleParts {
        PlaybackTitleParts {
            title: PlaybackTitlePart {
                role: PlaybackTitlePartRole::Title,
                text: title.to_string(),
            },
            context: context.map(|text| PlaybackTitlePart {
                role: PlaybackTitlePartRole::Context,
                text: text.to_string(),
            }),
        }
    }

    /// Paint the panel and return the title band's three rows below the
    /// seekbar (y 3, y 4, y 5), each as (text, fgs). With a context part the
    /// band expands: the show + `pos / dur` time on y 3, the title alone on
    /// y 4, the transport controls on y 5. Without one the band stays
    /// two rows: the title + time on y 3, the controls on y 4, y 5 blank.
    /// The painter paints the typed parts only over an attached target's
    /// plain title; the parts replace it when present.
    fn painted_split_title_rows(parts: PlaybackTitleParts) -> (PaintedRow, PaintedRow, PaintedRow) {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.transport.show_controls = true;
        panel.transport.now_playing_title = Some(("Fallback".into(), palette::PLAYBACK_VALUE_FG));
        panel.transport.title_parts = Some(parts);
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();
        let row = |y: u16| {
            (
                (0..40).map(|x| buf[(x, y)].symbol().to_string()).collect(),
                (0..40).map(|x| buf[(x, y)].fg).collect(),
            )
        };
        (row(3), row(4), row(5))
    }

    fn assert_cells_carry(text: &str, fgs: &[Color], needle: &str, expected: Color, label: &str) {
        let start = text
            .find(needle)
            .unwrap_or_else(|| panic!("{label} not painted in the row: {text:?}"));
        for (i, _) in needle.char_indices() {
            assert_eq!(
                fgs[start + i],
                expected,
                "cell {i} of {label} must carry its role's fg: {text:?}"
            );
        }
    }

    /// The painted media-type table (tasks 4.1, 4.2, 4.4) on the queue
    /// column's title band: two-part rows expand onto two content rows with
    /// the transport controls on the band's bottom row — the context part
    /// (the show) paints on the first row in the yellow context role beside
    /// the `pos / dur` time, and the title part alone on the row below in
    /// the aqua title role; single-part rows keep the unexpanded band,
    /// painting the title and the time on the first row and the controls on
    /// the row below. (The one-space delineation between parts remains a
    /// contract only where the parts still share a row — the Library strip's
    /// combined row, owned by `chrome_player.rs`'s painter test.)
    #[rstest]
    #[case::emby_movie("Movie Name", None)]
    #[case::emby_episode("Pilot", Some("Series"))]
    fn split_title_band_paints_each_media_types_parts_in_their_roles(
        #[case] title: &str,
        #[case] context: Option<&str>,
    ) {
        let ((top, top_fgs), (mid, mid_fgs), (bottom, _bottom_fgs)) =
            painted_split_title_rows(parts_for(title, context));
        if let Some(context) = context {
            // The first row carries the show in the context role and
            // the `pos / dur` time; the title is not on it.
            assert_cells_carry(
                &top,
                &top_fgs,
                context,
                palette::PLAYBACK_CONTEXT_FG,
                "the context part",
            );
            assert!(
                !top.contains(title),
                "the title paints below the show row, not on it: {top:?}"
            );
            assert!(
                top.contains('/'),
                "the elapsed/duration time rides the show row: {top:?}"
            );
            // The row below carries the title alone in the title role:
            // no show, no time, no context-role paint.
            assert_cells_carry(
                &mid,
                &mid_fgs,
                title,
                palette::PLAYBACK_TITLE_FG,
                "the title part",
            );
            assert!(!mid.contains(context), "the show stays on its row: {mid:?}");
            assert!(!mid.contains('/'), "no time on the title row: {mid:?}");
            let title_start = mid.find(title).unwrap();
            assert_ne!(
                mid_fgs[title_start - 1],
                palette::PLAYBACK_CONTEXT_FG,
                "no context part beside the title: {mid:?}"
            );
            // The transport controls land on the band's bottom row.
            assert!(
                bottom.contains('X'),
                "the stop glyph paints on the bottom row: {bottom:?}"
            );
            assert!(!bottom.contains(title) && !bottom.contains(context));
        } else {
            // The unexpanded band: the title and the time share the
            // first row, the controls ride the row below, and nothing
            // expands onto the band's last row.
            assert_cells_carry(
                &top,
                &top_fgs,
                title,
                palette::PLAYBACK_TITLE_FG,
                "the title part",
            );
            assert!(top.contains('/'), "the time rides the title row: {top:?}");
            assert!(
                mid.contains('X'),
                "the stop glyph paints on the controls row: {mid:?}"
            );
            assert!(
                !mid.contains(title) && !mid.contains('/'),
                "a single-part row does not expand onto the controls row: {mid:?}"
            );
            assert!(
                !bottom.contains(title) && !bottom.contains('X'),
                "nothing paints below the controls row: {bottom:?}"
            );
        }
    }

    #[test]
    fn transport_clicks_resolve_against_retained_geometry() {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
        panel.transport.show_controls = true;
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();

        let (play_pause, seekbar) = panel.transport_hits();
        let click = |column: u16, row: u16| {
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column,
                row,
                modifiers: KeyModifiers::NONE,
            })
        };
        assert!(matches!(
            panel.on(&click(play_pause.x + 1, play_pause.y)),
            Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
        ));
        let column = seekbar.x + seekbar.width / 2;
        assert!(matches!(
            panel.on(&click(column, seekbar.y)),
            Some(Msg::Playback(PlaybackRequest::SeekTo(f))) if (f - 0.5).abs() < 1e-6
        ));
        // Prev and next keep distinct painted rects and resolve to their own
        // intents (the prev control was painted without a hit rect until now).
        panel.transport.availability.previous = true;
        panel.transport.availability.next = true;
        let (prev, next) = panel.transport_nav_hits();
        assert!(prev.width > 0 && next.width > 0);
        assert!(prev.right() <= next.x, "prev={prev:?} next={next:?}");
        assert!(matches!(
            panel.on(&click(prev.x, prev.y)),
            Some(Msg::Playback(PlaybackRequest::Previous))
        ));
        assert!(matches!(
            panel.on(&click(next.x, next.y)),
            Some(Msg::Playback(PlaybackRequest::Next))
        ));
        // A collapsed panel (no transport painted) resolves nothing.
        panel.set_transport_area(None);
        assert!(panel.on(&click(5, 4)).is_none());
        let _ = KeyEvent::new(Key::Null, KeyModifiers::NONE);
    }
}
