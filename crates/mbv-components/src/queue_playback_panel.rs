//! The Queue playback panel (task 3.5, design D10): the queue column's
//! playback surface, mounted in every queue-visible layout, idle included.
//! It owns the always-painted header row (the playback-target indicator
//! and the right-aligned status word, with the now-playing title between
//! them),
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
//! A transport glyph resolves on a single press; the seekbar seeks on a
//! double-click and scrubs on a left-drag anchored inside its bar, recognized
//! by the panel's own `MouseGestureState`.

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
use crate::mouse::gesture::{MouseGesture, MouseGestureState};
use crate::mouse::seekbar::admit_seekbar_scrub;
use mbv_render::PlaybackStripAreas;
use mbv_render::arrangements::chrome::QUEUE_TRANSPORT_ROWS;
use mbv_render::components::chrome_player::TransportAvailability;
use mbv_render::components::widgets::queue_panel_inset;
use mbv_render::{
    HeaderTitle, PlaybackRenderContext, render_header_title, render_player_panel, target_icon,
};
use mbv_theme as palette;
use mbv_ui_model::playback_target::NowPlayingStatus;
use mbv_ui_msg::UserEvent;
use mbv_ui_msg::{Msg, PlaybackRequest, ShellRequest};

/// The queue-column transport's surface: the fixed chrome band the queue
/// playback panel paints in every queue-visible layout (the former
/// queue-only strip's identity, D10).
const TRANSPORT_SURFACE: palette::Surface = palette::Surface::QueueOnlyPlaybackPanel;

#[derive(Debug)]
pub struct QueuePlaybackPanel {
    /// The header's projected facts: the status word and the playback target
    /// (`App::playback_host_label_and_remote`, no tracking suffix) with its
    /// remote flag — every header row paints the target indicator (the icon
    /// and, while remote, the host) and the right-aligned status word; the
    /// remote flag also picks the target icon colour.
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
    /// The visual slot's image rectangle the shell resolves in the sync pass
    /// from the same paint checkpoint the slot adapter uses (artwork, loading
    /// placeholder, or visualizer reservation). `Rect::default()` while the
    /// slot is idle, hidden, or absent, so a right-click resolves nothing.
    visual_slot_area: Rect,
    /// The panel's own retained transport hit geometry (task 3.7): cleared
    /// whenever the transport does not paint, so a collapsed panel resolves
    /// nothing.
    play_pause_area: Rect,
    stop_area: Rect,
    next_area: Rect,
    prev_area: Rect,
    seekbar_area: Rect,
    /// Panel-local pointer recognition (ADR 0024, design.md D3): the
    /// double-click window that separates a transport glyph's single press
    /// from a seekbar seek, and the drag anchor a scrub needs.
    mouse_gestures: MouseGestureState,
    /// The last admitted seekbar scrub; `None` admits the next one. Set only
    /// by an admitted scrub, so the throttle measures real scrubs.
    scrub_at: Option<Instant>,
    /// Panel-local marquee state (the title row's scrolling title).
    marquee_text: String,
    marquee_started_at: Instant,
    /// The playback-target indicator's painted rect, retained from the last
    /// header paint: the icon and host name are the F3 sessions shortcut's
    /// click target.
    target_area: Rect,
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
                title_site: mbv_ui_model::playback::NowPlayingTitleSite::Header,
                status_indicators: None,
                use_nerd_fonts: false,
                idle_feed_title: None,
                availability: TransportAvailability::default(),
            },
            transport_area: None,
            visual_slot_area: Rect::default(),
            play_pause_area: Rect::default(),
            stop_area: Rect::default(),
            next_area: Rect::default(),
            prev_area: Rect::default(),
            seekbar_area: Rect::default(),
            mouse_gestures: MouseGestureState::new(),
            scrub_at: None,
            marquee_text: String::new(),
            marquee_started_at: Instant::now(),
            target_area: Rect::default(),
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

    /// Project the visual slot's hit rectangle, computed by the shell in the
    /// sync pass. `None` (idle, hidden, or no placement) clears the hit
    /// geometry so a right-click resolves nothing.
    pub fn set_visual_slot_area(&mut self, area: Option<Rect>) {
        self.visual_slot_area = area
            .filter(|rect| rect.width > 0 && rect.height > 0)
            .unwrap_or_default();
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

    /// Test-only: the sync-projected visual slot hit rect.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn visual_slot_area_for_test(&self) -> Rect {
        self.visual_slot_area
    }

    /// Test-only: the projected now-playing title parts the sync pass
    /// delivered (task 5.2).
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn transport_title_parts_for_test(&self) -> Option<mbv_queue::PlaybackTitleParts> {
        self.transport.title_parts.clone()
    }

    /// Test-only: the retained playback-target click rect.
    #[cfg(any(test, feature = "test"))]
    #[must_use]
    pub fn target_area_for_test(&self) -> Rect {
        self.target_area
    }

    /// Resolve one pointer event. The transport glyphs and the target
    /// indicator act on a single press; the seekbar needs a recognized
    /// double-click, and a left-drag anchored inside the bar scrubs it.
    fn mouse(&mut self, event: MouseEvent) -> Option<Msg> {
        // Every event reaches the recognizer, the ignored ones included: the
        // first press of a pair must be remembered for the second to be a
        // double-click, and that press arms the drag anchor.
        let gesture = self.mouse_gestures.recognize(event);
        let point = (event.column, event.row).into();
        match event.kind {
            // The playback-target indicator is the F3 sessions shortcut's
            // click target (the header row paints above the transport, so its
            // check leads).
            MouseEventKind::Down(MouseButton::Left)
                if self.target_area.width > 0 && self.target_area.contains(point) =>
            {
                Some(Msg::Shell(Box::new(ShellRequest::OpenSessions)))
            }
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
            // A single press on the bar resolves nothing: only the second
            // press of a pair is a seek, so a stray click cannot jump the
            // position. The transport arms above keep the raw press because
            // that second press recognizes as a double-click, not a click.
            MouseEventKind::Down(MouseButton::Left)
                if self.seekbar_area.width > 0
                    && self.seekbar_area.contains(point)
                    && matches!(gesture, Some(MouseGesture::DoubleClick(_))) =>
            {
                self.seek_intent(event.column)
            }
            MouseEventKind::Drag(MouseButton::Left) => self.scrub(gesture),
            // A right-click on the visual slot's image rectangle opens its
            // chrome menu. The click point anchors the menu; the shell owns
            // the menu build and the queue-column placement bounds.
            MouseEventKind::Down(MouseButton::Right) if self.visual_slot_area.contains(point) => {
                Some(Msg::Shell(Box::new(ShellRequest::VisualSlotContextMenu {
                    x: event.column,
                    y: event.row,
                })))
            }
            _ => None,
        }
    }

    /// The seek intent a column resolves: its fraction of the painted bar
    /// span. `None` when the panel painted no bar.
    fn seek_intent(&self, column: u16) -> Option<Msg> {
        (self.seekbar_area.width > 0).then(|| {
            let fraction = f64::from(column.saturating_sub(self.seekbar_area.x))
                / f64::from(self.seekbar_area.width);
            Msg::Playback(PlaybackRequest::SeekTo(fraction))
        })
    }

    /// Scrub the bar for one recognized left-drag motion. The drag must have
    /// anchored inside the bar span and the pointer must still be over it —
    /// a drag that merely crossed the bar (a queue-row drag, say) is not a
    /// scrub of this bar. Admitted at most once per `SEEKBAR_SCRUB_INTERVAL`.
    fn scrub(&mut self, gesture: Option<MouseGesture>) -> Option<Msg> {
        let column = admit_seekbar_scrub(self.seekbar_area, &mut self.scrub_at, gesture)?;
        self.seek_intent(column)
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
        // The header row always paints the playback-target indicator — the
        // target icon, the host name while remote — and the right-aligned
        // status word; it carries the now-playing title in the zone between
        // them while the header is the title's home (the `Header` site); the
        // artwork site carries the title on the overlay. The indicator's
        // painted rect is retained as the F3 sessions shortcut's click
        // target.
        let mut target_area = Rect::default();
        render_header_title(
            frame,
            header,
            &mut HeaderTitle {
                title: self
                    .transport
                    .now_playing_title
                    .as_ref()
                    .map(|(text, _)| text.as_str()),
                parts: self.transport.title_parts.as_ref(),
                marquee_text: &mut self.marquee_text,
                marquee_started_at: &mut self.marquee_started_at,
                panel: TRANSPORT_SURFACE,
                icon: target_icon(self.transport.use_nerd_fonts, self.host_is_remote),
                title_site: self.transport.title_site,
                status: self.status,
                host: &self.host,
                host_is_remote: self.host_is_remote,
            },
            &mut target_area,
        );
        self.target_area = target_area;
        // While idle — or whenever the shell hands no transport rect — the
        // slot and transport rows are already collapsed (task 3.6); the
        // panel paints nothing else and its hit geometry stays cleared.
        // The retained rect is sync-projected and can be one frame stale
        // against the frame's buffer: a pinned width tween resizes the pty
        // after the sync pass read the size, so a collapse can hand down a
        // wider band than the buffer holds. The gauge painter indexes the
        // buffer without clipping (ratatui-widgets gauge.rs `buf[(x, y)]`),
        // so an unclamped stale band panics (`index outside of buffer`,
        // 2026-10-05 pinned view toggle). Clamp to the placement this frame
        // actually paints: paint and hit geometry then agree with the
        // buffer whatever the tween did between sync and draw.
        let Some(transport_area) = self
            .transport_area
            .map(|retained| retained.intersection(area))
        else {
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
        // The painted row budget is always the four base transport rows:
        // the controls with the status text (right below the visual slot),
        // the title row kept blank (the title lives on the header row), the
        // seekbar with its flanking times, and one blank row.
        let player_h = transport_area.height.min(QUEUE_TRANSPORT_ROWS);
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
mod tests;
