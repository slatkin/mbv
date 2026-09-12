use super::shell_queue::NOW_PLAYING_THROBBER_FRAMES;
use crate::app::layout::{AppLayout, FrameChromeGeometry};
use crate::app::render::arrangements::chrome::{chrome_geometry, ChromeGeometryInput};
use crate::app::render::arrangements::queue::{queue_panel_subareas, QueuePanelGeometry};
use crate::app::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::Frame;

impl App {
    pub(in crate::app) fn now_playing_throbber_span(&self) -> Span<'static> {
        let frame = NOW_PLAYING_THROBBER_FRAMES
            [self.now_playing_throbber_index % NOW_PLAYING_THROBBER_FRAMES.len()];
        Span::styled(frame.to_string(), Style::default().fg(palette::ACCENT))
    }

    pub(in crate::app) fn compute_frame_layout(
        &mut self,
        area: Rect,
    ) -> Option<FrameChromeGeometry> {
        if area.width == 0 || area.height == 0 {
            return None;
        }
        self.terminal_width = area.width;
        self.terminal_height = area.height;
        Some(self.compute_chrome_geometry(area))
    }

    pub(in crate::app) fn compute_chrome_geometry(&self, area: Rect) -> FrameChromeGeometry {
        chrome_geometry(ChromeGeometryInput {
            area,
            panel_mode: self.effective_panel_mode(),
            panel_focus: self.effective_panel_focus(),
            queue_column_width: self.queue_column_width,
            terminal_width: self.terminal_width,
            card_height: self.layout.main.card.height,
            playback_active: self.effective_playback_state().active,
        })
    }

    pub(in crate::app) fn queue_panel_placement(&self) -> QueuePanelGeometry {
        let chrome = self.compute_chrome_geometry(Rect::new(
            0,
            0,
            self.terminal_width,
            self.terminal_height,
        ));
        let panel_area = chrome.root.queue.unwrap_or_default();
        let (content_area, title_area, pill_row, title_reserved) = queue_panel_subareas(panel_area);
        QueuePanelGeometry {
            panel_area,
            content_area,
            title_area,
            pill_row,
            title_reserved,
        }
    }

    /// Publish the frame's RootFrame placements. Painting is exclusively done
    /// by mounted panels and the overlay stack in `Model::draw_frame`.
    pub(in crate::app) fn compose_root_frame(&mut self, frame: &mut Frame) {
        let Some(chrome) = self.compute_frame_layout(frame.area()) else {
            return;
        };
        let mut layout = AppLayout::default();
        if frame.area().height >= 4 {
            layout.main.panel_area = chrome.panel_area;
            layout.main.panel_content_area = chrome.panel_content_area;
            layout.main.left_area = chrome.left_area;
            layout.playback.player_area = chrome.player_area;
            layout.playback.status_area = chrome.status_area;
            layout.root_frame = chrome.root;
        }
        self.layout = layout;
    }
}
