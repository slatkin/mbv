use super::backdrop::dim_backdrop;
use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Clear};

/// Columns the storm side borders add to a modal's content width (2 each
/// side).
const FRAME_COLUMNS: u16 = 4;
/// Rows the frame adds around a modal's content height: the top band and the
/// bottom row.
const FRAME_ROWS: u16 = 2;

/// The painted modal's outer rect and its content area.
#[derive(Debug)]
pub struct ModalFrame {
    /// The whole modal, including the border bands — the click-outside
    /// boundary for pointer handling.
    pub outer: Rect,
    /// The content area inside the border bands.
    pub inner: Rect,
}

/// Paints the modal frame and returns its content area.
pub fn render_modal_frame(f: &mut Frame, dim_flag: &mut bool, w: u16, h: u16, bg: Color) -> Rect {
    render_modal_frame_bounds(f, dim_flag, w, h, bg).inner
}

/// Paints the modal frame and returns both its outer rect (the click-outside
/// boundary) and its content area.
pub fn render_modal_frame_bounds(
    f: &mut Frame,
    dim_flag: &mut bool,
    width: u16,
    height: u16,
    bg: Color,
) -> ModalFrame {
    *dim_flag = true;
    dim_backdrop(f);

    let full = f.area();
    // `width`/`height` describe the content area: the storm frame (2 columns
    // each side, one top band row and one bottom row) is added around it, so
    // every caller's content layout is unchanged by the frame.
    let width = width
        .saturating_add(FRAME_COLUMNS)
        .min(full.width.saturating_sub(2));
    let height = height.saturating_add(FRAME_ROWS).min(full.height);
    let x = full.x + full.width.saturating_sub(width) / 2;
    let y = full.y + full.height.saturating_sub(height) / 2;
    let rect = Rect {
        x,
        y,
        width,
        height,
    };

    f.render_widget(Clear, rect);
    // The modal is a storm frame around a content body: the whole modal is
    // filled with the border color (`Surface::PopupBorder`, `#2b3238`), then the
    // content body (`bg`) is cut out of it — the same pixels as painting the
    // two 2-column sides, the top band, and the bottom row individually, in
    // two widgets instead of four.
    let border =
        Style::default().bg(palette::surface_colors(palette::Surface::PopupBorder, false).fill);
    f.render_widget(Block::default().style(border), rect);
    let top_h = 1_u16.min(rect.height);
    let bottom_h = 1_u16.min(rect.height.saturating_sub(top_h));
    let side_y = rect.y.saturating_add(top_h);
    let side_h = rect.height.saturating_sub(top_h + bottom_h);
    let side_w = 2_u16.min(rect.width);
    let inner = Rect {
        x: rect.x.saturating_add(side_w),
        y: side_y,
        width: rect.width.saturating_sub(side_w.saturating_mul(2)),
        height: side_h,
    };
    f.render_widget(Block::default().style(Style::default().bg(bg)), inner);
    ModalFrame { outer: rect, inner }
}
