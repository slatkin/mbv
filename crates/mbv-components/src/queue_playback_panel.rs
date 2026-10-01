//! The Queue playback panel (task 3.5, design D10): the queue column's
//! playback surface, mounted in every queue-visible layout, idle included.
//! It owns the always-painted header row (`STATUS [host]`, left-aligned),
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

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::library_playback_panel::PlaybackProjection;
use mbv_render::PlaybackStripAreas;
use mbv_render::arrangements::chrome::PLAYER_BOX_HEIGHT;
use mbv_render::components::chrome_player::TransportAvailability;
use mbv_render::components::widgets::queue_panel_inset;
use mbv_render::{PlaybackRenderContext, render_playback_header, render_player_panel};
use mbv_theme as palette;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{Msg, PlaybackRequest};

/// The queue-column transport's surface: the fixed chrome band the queue
/// playback panel paints in every queue-visible layout (the former
/// queue-only strip's identity, D10).
const TRANSPORT_SURFACE: palette::Surface = palette::Surface::QueueOnlyPlaybackPanel;

#[derive(Debug)]
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
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: NowPlayingStatus::Idle,
            host: String::new(),
            host_is_remote: false,
            transport: PlaybackProjection {
                state: mbv_ui_model::playback::PlaybackState::default(),
                show_controls: false,
                panel: TRANSPORT_SURFACE,
                panel_focused: false,
                now_playing_title: None,
                title_parts: None,
                status_indicators: None,
                use_nerd_fonts: false,
                idle_feed_title: None,
                availability: TransportAvailability::default(),
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
    pub fn set_header(&mut self, status: NowPlayingStatus, host: String, host_is_remote: bool) {
        self.status = status;
        self.host = host;
        self.host_is_remote = host_is_remote;
    }

    /// Project the transport's facts (the shared transport projection, with
    /// the panel's own chrome-band surface identity).
    pub fn set_transport(&mut self, transport: PlaybackProjection) {
        self.transport = transport;
    }

    /// Project the transport rect computed by the shell in the sync pass from
    /// the prior-paint card checkpoint. `None` collapses the transport and
    /// clears the retained hit geometry (task 3.6: idle paints only the header
    /// row and resolves nothing).
    pub fn set_transport_area(&mut self, area: Option<Rect>) {
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
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn transport_hits(&self) -> (Rect, Rect) {
        (self.play_pause_area, self.seekbar_area)
    }

    /// Test-only: the retained prev/next transport hit rects.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn transport_nav_hits(&self) -> (Rect, Rect) {
        (self.prev_area, self.next_area)
    }

    /// Test-only: the sync-projected transport area.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn transport_area_for_test(&self) -> Option<Rect> {
        self.transport_area
    }

    /// Test-only: the projected now-playing title parts the sync pass
    /// delivered (task 5.2).
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn transport_title_parts_for_test(&self) -> Option<mbv_queue::PlaybackTitleParts> {
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
        // The transport band fills its whole rect before the three transport
        // rows paint over it: the side-by-side slot's leftover rows (a slot
        // taller than the three transport rows) stay on the chrome band.
        frame.render_widget(
            Block::default()
                .style(Style::default().bg(palette::surface_colors(TRANSPORT_SURFACE, false).fill)),
            transport_area,
        );
        // The painted row budget is always the three base transport rows:
        // the title row (a two-part title shares it, context left and title
        // right), the seekbar with its flanking times, and the controls.
        let player_h = transport_area.height.min(PLAYER_BOX_HEIGHT);
        let mut playback = PlaybackStripAreas::default();
        render_player_panel(
            frame,
            PlaybackRenderContext {
                area: transport_area,
                playback: &mut playback,
                player_h,
                controls: mbv_render::PlaybackControls {
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
    use mbv_queue::{PlaybackTitlePart, PlaybackTitlePartRole, PlaybackTitleParts};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use ratatui::style::Color;
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

    /// Paint the panel and return the band's four rows (y 2..y 5), each as
    /// (text, fgs). The band is always three rows: the title on y 2 (a
    /// context part shares it, left-aligned, with the title right-aligned),
    /// the seekbar flanked by its times on y 3, the transport controls on
    /// y 4, y 5 blank. The painter paints the typed parts only over an
    /// attached target's plain title; the parts replace it when present.
    fn painted_band_rows(
        parts: PlaybackTitleParts,
    ) -> (PaintedRow, PaintedRow, PaintedRow, PaintedRow) {
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
        (row(2), row(3), row(4), row(5))
    }

    /// The seekbar row's contract: ` <elapsed> <bar> <total> ` — the
    /// elapsed time left with one space before the bar, the total right
    /// with one space after it, the bar spanning the columns between.
    fn assert_seek_row_flanks_times(text: &str, label: &str) {
        assert!(
            text.starts_with(" 0:00 "),
            "the elapsed time paints left of the seekbar with a space: {label}: {text:?}"
        );
        assert!(
            text.ends_with(" 0:00 "),
            "the total time paints right of the seekbar with a space: {label}: {text:?}"
        );
        assert!(
            text.contains('\u{2591}') || text.contains('\u{2593}'),
            "the bar paints between the times: {label}: {text:?}"
        );
        assert!(
            !text.contains('/'),
            "the seekbar row carries no `pos/dur` cluster: {label}: {text:?}"
        );
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
    /// column's band: the title row first, the seekbar flanked by its times
    /// next, the transport controls and status pills last. A two-part title
    /// shares the one title row — the context part (the show) left-aligned
    /// in the yellow context role, the title part right-aligned in the
    /// green title role, neither carrying a time. Single-part rows paint
    /// the title alone on that row. (The Library strip's combined row keeps
    /// its own contract, owned by `chrome_player.rs`'s painter test.)
    #[rstest]
    #[case::emby_movie("Movie Name", None)]
    #[case::emby_episode("Pilot", Some("Series"))]
    fn split_title_band_paints_each_media_types_parts_in_their_roles(
        #[case] title: &str,
        #[case] context: Option<&str>,
    ) {
        let ((first, first_fgs), (second, _), (third, _), (fourth, _fourth_fgs)) =
            painted_band_rows(parts_for(title, context));
        if let Some(context) = context {
            // One shared row: the show left-aligned in the context role,
            // the title right-aligned in the title role, no time on it.
            assert_cells_carry(
                &first,
                &first_fgs,
                context,
                palette::PLAYBACK_CONTEXT_FG,
                "the context part",
            );
            assert_cells_carry(
                &first,
                &first_fgs,
                title,
                palette::PLAYBACK_TITLE_FG,
                "the title part",
            );
            assert!(
                first.starts_with(format!(" {context}").as_str()),
                "the show hugs the left indent: {first:?}"
            );
            assert!(
                first.ends_with(format!("{title} ").as_str()),
                "the title hugs the right indent: {first:?}"
            );
            assert!(
                !first.contains('/'),
                "no time rides the shared title row: {first:?}"
            );
            // The seekbar rides below the title row with its times, the
            // transport controls on the band's bottom row.
            assert_seek_row_flanks_times(&second, "the two-part band");
            assert!(!second.contains(title) && !second.contains(context));
            assert!(
                third.contains('X'),
                "the stop glyph paints on the bottom row: {third:?}"
            );
            assert!(!third.contains(title) && !third.contains(context));
            assert!(
                !third.contains("0:00"),
                "no time on the controls row: {third:?}"
            );
            assert!(
                !fourth.contains(title) && !fourth.contains(context) && !fourth.contains('X'),
                "nothing paints below the controls row: {fourth:?}"
            );
        } else {
            // The single-part band: the title alone on the first row, the
            // seekbar with its times below it, the controls below that,
            // nothing on the band's last row.
            assert_cells_carry(
                &first,
                &first_fgs,
                title,
                palette::PLAYBACK_TITLE_FG,
                "the title part",
            );
            assert!(
                !first.contains('/'),
                "no time rides the title row: {first:?}"
            );
            assert!(
                !second.contains(title),
                "the title stays on its row: {second:?}"
            );
            assert_seek_row_flanks_times(&second, "the single-part band");
            assert!(
                third.contains('X'),
                "the stop glyph paints on the controls row: {third:?}"
            );
            assert!(
                !third.contains(title) && !third.contains("0:00"),
                "a single-part title stays off the controls row: {third:?}"
            );
            assert!(
                !fourth.contains(title) && !fourth.contains('X'),
                "nothing paints below the controls row: {fourth:?}"
            );
        }
    }

    #[test]
    fn seek_row_shows_partial_progress_and_seeks_from_the_bar_only() {
        // Regression guard: at 75s of 300s the seek row paints a 25% fill
        // between its flanking times, and only the bar span seeks.
        let mut panel = QueuePlaybackPanel::new();
        panel.set_header(NowPlayingStatus::Playing, "music-box".into(), false);
        panel.transport.now_playing_title = Some(("Example".into(), palette::PLAYBACK_VALUE_FG));
        panel.transport.show_controls = true;
        panel.transport.state.position_ticks = 75 * mbv_emby_model::TICKS_PER_SECOND;
        panel.transport.state.runtime_ticks = 300 * mbv_emby_model::TICKS_PER_SECOND;
        panel.set_transport_area(Some(Rect::new(0, 2, 40, 4)));
        let mut terminal = Terminal::new(TestBackend::new(40, 8)).unwrap();
        terminal
            .draw(|frame| panel.view(frame, Rect::new(0, 0, 40, 8)))
            .unwrap();
        let buf = terminal.backend().buffer();
        let text: String = (0..40).map(|x| buf[(x, 3)].symbol().to_string()).collect();
        // ` 1:15 <bar> 5:00 `: elapsed left, total right, bar of 28.
        assert!(
            text.starts_with(" 1:15 "),
            "elapsed left of the bar: {text:?}"
        );
        assert!(text.ends_with(" 5:00 "), "total right of the bar: {text:?}");
        assert_eq!(text.chars().filter(|c| *c == '\u{2593}').count(), 7);
        assert_eq!(text.chars().filter(|c| *c == '\u{2591}').count(), 21);
        // A quarter fill: 7 accent cells, then the track colour.
        let fgs: Vec<Color> = (0..40).map(|x| buf[(x, 3)].fg).collect();
        for x in 6u16..13 {
            assert_eq!(
                fgs[usize::from(x)],
                palette::ACCENT,
                "filled cell {x}: {text:?}"
            );
        }
        for x in 13u16..34 {
            assert_eq!(
                fgs[usize::from(x)],
                palette::PROGRESS_TRACK,
                "unplayed cell {x}: {text:?}"
            );
        }
        // The retained hit rect is the bar span only: x 6, width 28.
        let (_, seekbar) = panel.transport_hits();
        assert_eq!(
            (seekbar.x, seekbar.y, seekbar.width),
            (6, 3, 28),
            "the bar span seeks, the time labels never do"
        );
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
