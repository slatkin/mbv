use crate::app::layout::{AppLayout, FrameChromeGeometry};
use crate::app::render::arrangements::chrome::{chrome_geometry, ChromeGeometryInput};
use crate::app::render::arrangements::queue::{
    queue_footer_row, queue_list_box, queue_panel_subareas, QueuePanelGeometry,
};
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

impl App {
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
            card_height: self.layout.card.height,
            playback_active: self.effective_playback_state().active,
            queue_title_expanded: self.transport_title_expanded(),
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
        QueuePanelGeometry {
            panel_area,
            content_area: queue_panel_subareas(queue_list_box(panel_area)),
            footer_row: queue_footer_row(panel_area),
        }
    }

    /// Publish the frame's `RootFrame` placements. Painting is exclusively done
    /// by mounted panels and the overlay stack in `Model::draw_frame`.
    pub(in crate::app) fn compose_root_frame(&mut self, frame: &mut Frame) {
        let Some(chrome) = self.compute_frame_layout(frame.area()) else {
            return;
        };
        let mut layout = AppLayout::default();
        // Preserve the sync-owned queue card checkpoint while publishing
        // this frame's root placements.
        layout.card = self.layout.card.clone();
        if frame.area().height >= 4 {
            layout.left_area = chrome.left_area;
            layout.root_frame = chrome.root;
        }
        self.layout = layout;
    }
}
