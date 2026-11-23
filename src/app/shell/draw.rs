//! Frame placement free functions (task 2.3, design D3).
//!
//! Root-frame placement builds ratatui values and publishes them on the
//! shell-owned `AppLayout`, so it is shell free functions over `App` reads,
//! moved out of `impl App`. The core keeps only the neutral left panel area
//! that the TUI pushes (D13 Rule B).

use super::Model;
use crate::app::App;
use mbv_render::arrangements::chrome::{ChromeGeometryInput, chrome_geometry};
use mbv_render::arrangements::queue::{QueuePanelGeometry, queue_footer_row, queue_list_content};
use mbv_render::layout::{AppLayout, FrameChromeGeometry};
use ratatui::Frame;
use ratatui::layout::Rect;

pub(in crate::app) fn compute_frame_layout(
    model: &mut Model,
    area: Rect,
) -> Option<FrameChromeGeometry> {
    if area.width == 0 || area.height == 0 {
        return None;
    }
    model.terminal_width = area.width;
    model.terminal_height = area.height;
    let single_panel = super::single_panel_for_width(area.width);
    model.app.set_single_panel(single_panel);
    Some(compute_chrome_geometry(
        &model.app,
        model.terminal_width,
        area,
    ))
}

pub(in crate::app) fn compute_chrome_geometry(
    app: &App,
    terminal_width: u16,
    area: Rect,
) -> FrameChromeGeometry {
    chrome_geometry(ChromeGeometryInput {
        area,
        panel_mode: app.effective_panel_mode(),
        panel_focus: app.effective_panel_focus(),
        queue_column_width: app.queue_column_width,
        terminal_width,
        card_height: app.layout.card.height,
        playback_active: app.effective_playback_state().active,
    })
}

pub(in crate::app) fn queue_panel_placement(
    app: &App,
    terminal_size: (u16, u16),
) -> QueuePanelGeometry {
    let chrome = compute_chrome_geometry(
        app,
        terminal_size.0,
        Rect::new(0, 0, terminal_size.0, terminal_size.1),
    );
    let panel_area = chrome.root.queue.unwrap_or_default();
    QueuePanelGeometry {
        panel_area,
        content_area: queue_list_content(panel_area),
        footer_row: queue_footer_row(panel_area),
    }
}

/// Publish the frame's `RootFrame` placements. Painting is exclusively done
/// by mounted panels and the overlay stack in `Model::draw_frame`.
pub(in crate::app) fn compose_root_frame(model: &mut Model, frame: &mut Frame) {
    let Some(chrome) = compute_frame_layout(model, frame.area()) else {
        return;
    };
    let mut layout = AppLayout::default();
    // Preserve the sync-owned queue card checkpoint while publishing
    // this frame's root placements.
    layout.card = model.app.layout.card.clone();
    if frame.area().height >= 4 {
        layout.left_area = chrome.left_area;
        layout.root_frame = chrome.root;
    }
    model.app.layout = layout;
}
