use super::super::ui_util::*;
use super::list_rows::focused_or_subtle;
use super::list_rows::SELECTED_BLOCK_SIDE_PADDING;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

/// Clamp the panel scroll offset (in terminal rows, content-space) so the grid row
/// spanning `[cur_top, cur_bot)` is fully visible within a viewport of `view_h` rows,
/// and never scrolls past the end of `total_h` rows of content.
pub(super) fn home_panel_scroll(
    current: u16,
    cur_top: u16,
    cur_bot: u16,
    total_h: u16,
    view_h: u16,
) -> u16 {
    let max_scroll = total_h.saturating_sub(view_h);
    let mut s = current.min(max_scroll);
    if cur_top < s {
        s = cur_top;
    }
    if cur_bot > s + view_h {
        s = cur_bot.saturating_sub(view_h);
    }
    s
}

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Parses a leading `YYYY-MM-DD` date out of `date_str` (an Emby date field,
/// which may carry a `T...` time/offset suffix that's ignored here) and
/// returns its `(year, month name, day)`, or `None` if it doesn't parse.
fn parse_ymd(date_str: &str) -> Option<(&str, &'static str, u32)> {
    let date_part = date_str.split('T').next().unwrap_or(date_str);
    let parts: Vec<&str> = date_part.splitn(3, '-').collect();
    let [y, m, d] = parts.as_slice() else {
        return None;
    };
    let day: u32 = d.parse().ok()?;
    let month_idx: usize = m.parse::<usize>().ok()?.checked_sub(1)?;
    Some((y, MONTHS.get(month_idx)?, day))
}

/// Formats an Emby `PremiereDate` value (e.g. `2015-06-19T00:00:00.0000000Z`)
/// as a release date like "19 Jun 2015".
pub(super) fn format_release_date(premiere_date: &str) -> String {
    parse_ymd(premiere_date)
        .map(|(y, month, d)| format!("{d} {} {y}", &month[..3]))
        .unwrap_or_else(|| premiere_date.to_string())
}

pub(super) fn render_home_video_item(
    f: &mut Frame,
    item: &mbv_core::api::EmbyItem,
    row_y: u16,
    item_h: u16,
    content_area: Rect,
    text_w: usize,
    selected: bool,
    focused: bool,
) {
    let expanded = selected && item_h > 1;
    let title_y = row_y + if expanded { 2 } else { 0 };

    if expanded {
        let bg = palette::resolve_surface_focus(focused);
        f.render_widget(
            Block::default().style(Style::default().bg(bg)),
            Rect {
                x: content_area.x,
                y: row_y + 1,
                width: text_w as u16,
                height: item_h.saturating_sub(2),
            },
        );
    }

    let marker = super::selection_marker(selected && focused && !expanded, super::MarkerEdge::Left);
    f.render_widget(
        Paragraph::new(marker),
        Rect {
            x: content_area.x,
            y: title_y,
            width: 1,
            height: 1,
        },
    );

    let text_inset = if selected {
        SELECTED_BLOCK_SIDE_PADDING
    } else {
        0
    };
    let tx = content_area.x + text_inset;
    let tw = (text_w as u16).saturating_sub(2 * text_inset);
    let title_color = if expanded {
        palette::YELLOW
    } else if selected && focused {
        palette::IRIS
    } else {
        focused_or_subtle(focused)
    };
    let title_style = if selected && focused {
        Style::default()
            .fg(title_color)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(title_color)
    };
    let title_trunc = trunc_str(&item.display_name(), tw as usize);
    f.render_widget(
        Paragraph::new(Span::styled(title_trunc, title_style)),
        Rect {
            x: tx,
            y: title_y,
            width: tw,
            height: 1,
        },
    );

    if expanded && row_y < content_area.y + content_area.height {
        super::render_selected_block_borders(
            f,
            Rect {
                x: content_area.x,
                width: text_w as u16,
                y: row_y,
                height: item_h,
            },
            0,
            item_h as usize,
            1,
            item_h.saturating_sub(2) as usize,
            super::SelectedBlockBorderStyle::Framed,
        );
    }
}

impl App {
    pub(super) fn render_selected_home_video_detail(
        &mut self,
        f: &mut Frame,
        content_area: Rect,
        row_y: u16,
        item_h: u16,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let detail_height = item_h.saturating_sub(5);
        if detail_height == 0 {
            return;
        }

        self.render_compact_detail(
            f,
            Rect {
                x: content_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: row_y + 3,
                width: content_area
                    .width
                    .saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: detail_height,
            },
            lib_idx,
            focused,
            false,
            layout,
        );
    }
}
