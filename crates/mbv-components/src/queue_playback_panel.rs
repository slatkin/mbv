//! The Queue playback panel (task 3.5, design D10): the queue column's
//! playback surface, mounted in every queue-visible layout, idle included.
//! It owns the header row (`[mbv]` left, the status word or the now-playing
//! title right; hidden while the title lives on the artwork),
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

use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use std::time::Instant;
use tuirealm::command::{Cmd, CmdResult};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute, QueryResult};
use tuirealm::state::State;

use super::library_playback_panel::PlaybackProjection;
use mbv_render::PlaybackStripAreas;
use mbv_render::arrangements::chrome::QUEUE_TRANSPORT_ROWS;
use mbv_render::arrangements::chrome::queue_playback_header_visible;
use mbv_render::components::chrome_player::TransportAvailability;
use mbv_render::components::widgets::queue_panel_inset;
use mbv_render::{
    HeaderTitle, PlaybackRenderContext, playback_state_icon, render_header_title,
    render_playback_header, render_player_panel,
};
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
    /// The header's projected status word (idle's right-anchored `IDLE`).
    status: NowPlayingStatus,
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

    /// Project the header row's status word.
    pub fn set_header(&mut self, status: NowPlayingStatus) {
        self.status = status;
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
        // While the title lives on the artwork (the kitty/sixel overlay
        // variant actually painted) and playback is active, the header row
        // is hidden: the geometry no longer reserves its rows and the panel
        // paints nothing there. Otherwise the header carries the now-playing
        // title (moved up from the band's former title row — two-part
        // titles keep their context-left/title-right split, a lone title
        // paints yellow); idle paints the brand row, `[mbv] ... IDLE`.
        if !queue_playback_header_visible(self.status, self.transport.title_site) {
            // Hidden: the slot region starts at the placement's top; painting
            // the header row here would overlap the slot's padding row.
        } else if self.status != NowPlayingStatus::Idle
            && let Some((title, _)) = &self.transport.now_playing_title
        {
            render_header_title(
                frame,
                header,
                &mut HeaderTitle {
                    title: title.as_str(),
                    parts: self.transport.title_parts.as_ref(),
                    marquee_text: &mut self.marquee_text,
                    marquee_started_at: &mut self.marquee_started_at,
                    panel: TRANSPORT_SURFACE,
                    icon: playback_state_icon(
                        self.transport.use_nerd_fonts,
                        self.transport.state.paused,
                    ),
                    title_site: self.transport.title_site,
                },
            );
        } else {
            render_playback_header(frame, header, self.status);
        }
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
