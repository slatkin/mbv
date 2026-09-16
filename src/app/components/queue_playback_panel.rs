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
                state: crate::app::types_playback::PlaybackState::default(),
                show_controls: false,
                panel: TRANSPORT_SURFACE,
                panel_focused: false,
                now_playing_title: None,
                title_parts: None,
                status_indicators: None,
                throbber: ratatui::text::Span::raw(""),
                idle_feed_title: None,
                use_nerd_fonts: false,
                stop_available: false,
                next_available: false,
            },
            transport_area: None,
            play_pause_area: Rect::default(),
            stop_area: Rect::default(),
            next_area: Rect::default(),
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
            self.seekbar_area = Rect::default();
        }
        self.transport_area = painted;
    }

    /// Test-only: the retained transport hit geometry (task 3.7).
    #[cfg(test)]
    pub(in crate::app) fn transport_hits(&self) -> (Rect, Rect) {
        (self.play_pause_area, self.seekbar_area)
    }

    /// Test-only: the sync-projected transport area.
    #[cfg(test)]
    pub(in crate::app) fn transport_area_for_test(&self) -> Option<Rect> {
        self.transport_area
    }

    fn mouse(&self, event: &MouseEvent) -> Option<Msg> {
        let point = (event.column, event.row).into();
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) if self.play_pause_area.contains(point) => {
                Some(Msg::Playback(PlaybackRequest::TogglePlayPause))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.stop_area.contains(point) && self.transport.stop_available =>
            {
                Some(Msg::Playback(PlaybackRequest::Stop))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.next_area.contains(point) && self.transport.next_available =>
            {
                Some(Msg::Playback(PlaybackRequest::Next))
            }
            MouseEventKind::Down(MouseButton::Left)
                if self.seekbar_area.contains(point) && self.seekbar_area.width > 0 =>
            {
                let fraction = event.column.saturating_sub(self.seekbar_area.x) as f64
                    / self.seekbar_area.width as f64;
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
        let player_h = transport_area.height.min(PLAYER_BOX_HEIGHT);
        let mut playback = PlaybackStripAreas::default();
        render_player_panel(
            frame,
            PlaybackRenderContext {
                area: transport_area,
                playback: &mut playback,
                player_h,
                show_controls: self.transport.show_controls,
                now_playing_title: self.transport.now_playing_title.clone(),
                panel: TRANSPORT_SURFACE,
                panel_focused: false,
                progress: (
                    self.transport.state.position_ticks,
                    self.transport.state.runtime_ticks,
                    self.transport.state.paused,
                ),
                use_nerd_fonts: self.transport.use_nerd_fonts,
                stop_available: self.transport.stop_available,
                next_available: self.transport.next_available,
                status_indicators: self.transport.status_indicators.clone(),
                throbber: self.transport.throbber.clone(),
                title_parts: self.transport.title_parts.clone(),
                // The queue column never shows the idle feed title: while
                // idle the panel is collapsed to its header row (task 3.6),
                // and the feed title's only open-link gate re-points at the
                // panel's presence (task 3.8).
                idle_feed_title: None,
                marquee_text: &mut self.marquee_text,
                marquee_started_at: &mut self.marquee_started_at,
            },
        );
        self.play_pause_area = playback.play_pause_area;
        self.stop_area = playback.stop_area;
        self.next_area = playback.next_area;
        self.seekbar_area = playback.seekbar_area;
    }

    fn query<'a>(&'a self, attr: Attribute) -> Option<QueryResult<'a>> {
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
    fn on(&mut self, event: &Event<UserEvent>) -> Option<Msg> {
        match event {
            Event::Mouse(mouse) => self.mouse(mouse),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::palette::Surface;
    use mbv_core::playback_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
    use ratatui::Terminal;
    use rstest::rstest;
    use tuirealm::event::{Key, KeyEvent, KeyModifiers};

    fn painted_panel(idle: bool) -> QueuePlaybackPanel {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(
            if idle {
                NowPlayingStatus::Idle
            } else {
                NowPlayingStatus::Playing
            },
            "music-box".into(),
            false,
        );
        panel.transport.show_controls = !idle;
        // The shell hands the transport band the rows below the header's
        // band: row 0 is the header's recessed padding, row 1 the header
        // itself.
        let transport_area = (!idle).then_some(Rect::new(0, 2, 40, 4));
        panel.set_transport_area(transport_area);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        panel
    }

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

    /// Paint the panel and return the split lower title row's text plus each
    /// cell's foreground (the transport band at y 2: seekbar y 2, controls
    /// y 3, title y 4).
    fn painted_split_title_row(parts: PlaybackTitleParts) -> (String, Vec<Color>) {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.transport.show_controls = true;
        // The painter paints the typed parts only over an attached target's
        // plain title; the parts replace it when present.
        panel.transport.now_playing_title = Some(("Fallback".into(), palette::PLAYBACK_VALUE_FG));
        panel.transport.title_parts = Some(parts);
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();
        (
            (0..40).map(|x| buf[(x, 4)].symbol().to_string()).collect(),
            (0..40).map(|x| buf[(x, 4)].fg).collect(),
        )
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
    /// column's split lower title row: two-part rows paint the title part in
    /// the aqua title role and the context part — with its leading space — in
    /// the yellow context role, delineated by exactly one space and no
    /// separator glyph; single-part rows paint wholly in the title role with
    /// no context part after them.
    #[rstest]
    #[case::emby_movie("Movie Name", None)]
    #[case::emby_home_video("Home Video", None)]
    #[case::audiobookshelf_book("Book Title", None)]
    #[case::emby_episode("Pilot", Some("Series"))]
    #[case::emby_audio_track("Track", Some("Artist"))]
    #[case::audiobookshelf_podcast("Episode", Some("Show"))]
    #[case::feed_entry("Entry", Some("Subscription"))]
    fn split_lower_title_row_paints_each_media_types_parts_in_their_roles(
        #[case] title: &str,
        #[case] context: Option<&str>,
    ) {
        let (text, fgs) = painted_split_title_row(parts_for(title, context));
        assert_cells_carry(
            &text,
            &fgs,
            title,
            palette::PLAYBACK_TITLE_FG,
            "the title part",
        );
        match context {
            Some(context) => {
                // D3: exactly one space between the parts and no separator
                // glyph of any form.
                let joined = format!("{title} {context}");
                assert!(
                    text.contains(&joined),
                    "exactly one space between the parts: {text:?}"
                );
                for separator in [" - ", " \u{2013} ", " \u{2014} ", " | ", " \u{2022} "] {
                    assert!(
                        !text.contains(&format!("{title}{separator}{context}")),
                        "no separator glyph between the parts: {text:?}"
                    );
                }
                // The context span owns its leading space, so the space and
                // the context text paint in the context role.
                let start = text.find(&joined).unwrap();
                for i in title.chars().count()..joined.chars().count() {
                    assert_eq!(
                        fgs[start + i],
                        palette::PLAYBACK_CONTEXT_FG,
                        "the space and context part paint in the context role: {text:?}"
                    );
                }
            }
            None => {
                // A single-part row paints no context part: nothing in the
                // context role follows the title run (the audiobook case is
                // task 4.4, design D5).
                let after = text.find(title).unwrap() + title.chars().count();
                assert_ne!(
                    fgs[after],
                    palette::PLAYBACK_CONTEXT_FG,
                    "no context part after the title: {text:?}"
                );
            }
        }
    }

    #[test]
    fn idle_panel_paints_only_the_header_row_and_resolves_nothing() {
        let panel = painted_panel(true);
        let (play_pause, seekbar) = panel.transport_hits();
        assert_eq!(play_pause.width, 0, "idle panel keeps no play/pause hit");
        assert_eq!(seekbar.width, 0, "idle panel keeps no seekbar hit");

        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Idle, "music-box".into(), false);
        panel.set_transport_area(None);
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("IDLE"));
        assert!(output.contains("on music-box"));
        assert!(
            !output.contains("PLAYING") && !output.contains("PAUSED"),
            "idle header states IDLE: {output:?}"
        );
        // Nothing else of the panel paints: no seekbar track, no transport.
        assert!(!output.contains('\u{2594}'));
    }

    #[test]
    fn active_panel_paints_the_transport_its_shell_rect_names() {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
        panel.transport.show_controls = true;
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let output: String = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect();
        assert!(output.contains("PLAYING"), "header states PLAYING");
        assert!(output.contains("Example"), "transport paints the title");
        assert!(output.contains('\u{2594}'), "seekbar track paints");

        let (play_pause, seekbar) = panel.transport_hits();
        assert!(play_pause.width > 0 && seekbar.width > 0, "hits retained");
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
        // A collapsed panel (no transport painted) resolves nothing.
        panel.set_transport_area(None);
        assert!(panel.on(&click(5, 4)).is_none());
        let _ = KeyEvent::new(Key::Null, KeyModifiers::NONE);
    }

    #[test]
    fn transport_band_uses_the_queue_column_chrome_surface() {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();
        // The transport band's first row paints the chrome-band fill.
        assert_eq!(
            buf[(1, 2)].style().bg,
            Some(palette::surface_colors(Surface::QueueOnlyPlaybackPanel, false).fill),
        );
    }

    /// The header is recessed in the queue column: the row above it and the
    /// two columns each side carry no header-band paint, and the painted row
    /// itself starts two columns in from the placement's left edge.
    #[test]
    fn header_paints_inset_one_row_down_and_two_columns_in() {
        let band = palette::surface_colors(Surface::QueueOnlyPlaybackPanel, false).fill;
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.set_transport_area(None);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();

        assert_eq!(
            buf[(2, 1)].style().bg,
            Some(band),
            "header row paints inset"
        );
        assert_ne!(
            buf[(0, 0)].style().bg,
            Some(band),
            "no paint above the inset"
        );
        assert_ne!(
            buf[(2, 0)].style().bg,
            Some(band),
            "no paint above the inset"
        );
        assert_ne!(
            buf[(0, 1)].style().bg,
            Some(band),
            "no paint left of the inset"
        );
        assert_ne!(
            buf[(1, 1)].style().bg,
            Some(band),
            "no paint left of the inset"
        );
        assert_ne!(
            buf[(38, 1)].style().bg,
            Some(band),
            "no paint right of the inset"
        );
        assert_ne!(
            buf[(39, 1)].style().bg,
            Some(band),
            "no paint right of the inset"
        );
    }

    /// The header carries no progress while playing: the transport's
    /// throbber and percent stay out of the header row even when the
    /// projected transport state has position and runtime to state.
    #[test]
    fn header_shows_no_progress_while_playing() {
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.transport.state.position_ticks = 45 * mbv_core::api::TICKS_PER_SECOND;
        panel.transport.state.runtime_ticks = 90 * mbv_core::api::TICKS_PER_SECOND;
        panel.transport.throbber = ratatui::text::Span::raw("~");
        panel.set_transport_area(None);
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();
        let header: String = (0..40).map(|x| buf[(x, 1)].symbol().to_owned()).collect();
        assert!(
            !header.contains('~') && !header.contains('%'),
            "no throbber or percent in the header: {header:?}"
        );
        assert!(header.contains(" PLAYING"), "status still left: {header:?}");
        assert!(
            header.trim_end().ends_with("on music-box"),
            "target still right: {header:?}"
        );
    }
}
