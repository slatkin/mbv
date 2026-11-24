//! Frame placement free functions (task 2.3, design D3).
//!
//! Root-frame placement builds ratatui values and publishes them on the
//! shell-owned `AppLayout`, so it is shell free functions over `App` reads,
//! moved out of the shell's inherent `App` methods. The core keeps only the
//! neutral left panel area that the TUI pushes (D13 Rule B).

use super::Model;
use mbv_client_core::App;
use mbv_render::arrangements::chrome::{ChromeGeometryInput, chrome_geometry};
use mbv_render::arrangements::queue::{QueuePanelGeometry, queue_footer_row, queue_list_content};
use mbv_render::layout::{AppLayout, FrameChromeGeometry};
use ratatui::Frame;
use ratatui::layout::Rect;

pub(in crate::tui) fn compute_frame_layout(
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
        model.layout.card.height,
        model.terminal_width,
        area,
    ))
}

pub(in crate::tui) fn compute_chrome_geometry(
    app: &App,
    card_height: u16,
    terminal_width: u16,
    area: Rect,
) -> FrameChromeGeometry {
    chrome_geometry(ChromeGeometryInput {
        area,
        panel_mode: app.effective_panel_mode(),
        panel_focus: app.effective_panel_focus(),
        queue_column_width: app.queue_column_width(),
        terminal_width,
        card_height,
        playback_active: app.effective_playback_state().active,
    })
}

pub(in crate::tui) fn queue_panel_placement(
    app: &App,
    card_height: u16,
    terminal_size: (u16, u16),
) -> QueuePanelGeometry {
    let chrome = compute_chrome_geometry(
        app,
        card_height,
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
pub(in crate::tui) fn compose_root_frame(model: &mut Model, frame: &mut Frame) {
    let Some(chrome) = compute_frame_layout(model, frame.area()) else {
        return;
    };
    let mut layout = AppLayout::default();
    // Preserve the sync-owned queue card checkpoint while publishing
    // this frame's root placements.
    layout.card = model.layout.card.clone();
    if frame.area().height >= 4 {
        layout.left_area = chrome.left_area;
        layout.root_frame = chrome.root;
    }
    // The core reads only the neutral left panel area of the frame just
    // composed (design D13 Rule B): it is zero when the frame is under four
    // rows, exactly as the layout's `left_area` is.
    let left_area = layout.left_area;
    model.app.set_left_panel_area(mbv_ui_model::geom::Rect::new(
        left_area.x,
        left_area.y,
        left_area.width,
        left_area.height,
    ));
    model.layout = layout;
}
