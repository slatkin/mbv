use super::backdrop::dim_backdrop;
use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Borders, Clear};

/// Columns the storm side borders add to a modal's content width (2 each
/// side).
const FRAME_COLUMNS: u16 = 4;
/// Rows the frame adds around a modal's content height: the top band under
/// the header and the bottom row. The header row was already part of the
/// height callers pass.
const FRAME_ROWS: u16 = 2;

/// The painted modal's outer rect and its content area.
#[derive(Debug)]
pub struct ModalFrame {
    /// The whole modal, including the header and border bands — the
    /// click-outside boundary for pointer handling.
    pub outer: Rect,
    /// The content area inside the header and border bands.
    pub inner: Rect,
}

/// Paints the modal frame and returns its content area.
pub fn render_modal_frame(
    f: &mut Frame,
    dim_flag: &mut bool,
    title: &str,
    w: u16,
    h: u16,
    bg: Color,
) -> Rect {
    render_modal_frame_bounds(f, dim_flag, title, w, h, bg).inner
}

/// Paints the modal frame and returns both its outer rect (the click-outside
/// boundary) and its content area.
pub fn render_modal_frame_bounds(
    f: &mut Frame,
    dim_flag: &mut bool,
    title: &str,
    width: u16,
    height: u16,
    bg: Color,
) -> ModalFrame {
    *dim_flag = true;
    dim_backdrop(f);

    let full = f.area();
    // `width`/`height` describe the content area: the storm frame (2 columns
    // each side, one row under the header and one along the bottom) and the
    // ink header row are added around it, so every caller's content layout is
    // unchanged by the frame.
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
    // The modal is a storm frame around a content body: a 2-column border on
    // the left and right, one row along the bottom, and one full-width row
    // just under the top (`SURFACE_RESTING`, `#2b3238`). The ink header band
    // is the topmost row; the title text is rendered last, on top of it
    // (issue #855).
    let border = Style::default().bg(palette::SURFACE_RESTING);
    f.render_widget(Block::default().style(Style::default().bg(bg)), rect);
    let header_h = 1_u16.min(rect.height);
    let top_h = 1_u16.min(rect.height.saturating_sub(header_h));
    let bottom_h = 1_u16.min(rect.height.saturating_sub(header_h + top_h));
    let side_y = rect.y.saturating_add(header_h + top_h);
    let side_h = rect.height.saturating_sub(header_h + top_h + bottom_h);
    let side_w = 2_u16.min(rect.width);
    f.render_widget(
        Block::default().style(border),
        Rect {
            x: rect.x,
            y: rect.y.saturating_add(header_h),
            width: rect.width,
            height: top_h,
        },
    );
    f.render_widget(
        Block::default().style(border),
        Rect {
            x: rect.x,
            y: side_y,
            width: side_w,
            height: side_h,
        },
    );
    f.render_widget(
        Block::default().style(border),
        Rect {
            x: rect.right().saturating_sub(side_w),
            y: side_y,
            width: side_w,
            height: side_h,
        },
    );
    f.render_widget(
        Block::default().style(border),
        Rect {
            x: rect.x,
            y: rect.bottom().saturating_sub(bottom_h),
            width: rect.width,
            height: bottom_h,
        },
    );
    f.render_widget(
        Block::default().style(Style::default().bg(palette::SURFACE_CHROME)),
        Rect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: header_h,
        },
    );
    let inner = Rect {
        x: rect.x.saturating_add(side_w),
        y: side_y,
        width: rect.width.saturating_sub(side_w.saturating_mul(2)),
        height: side_h,
    };
    let block = Block::default()
        .title(Span::styled(
            title.to_string(),
            Style::default()
                .fg(palette::TEXT_PRIMARY)
                .add_modifier(Modifier::BOLD),
        ))
        .title_alignment(Alignment::Center)
        .borders(Borders::NONE);
    f.render_widget(block, rect);
    ModalFrame { outer: rect, inner }
}
