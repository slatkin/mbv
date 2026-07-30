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
pub(super) fn power_home_panel_scroll(
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
    item: &mbv_core::api::MediaItem,
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
        let bg = if focused {
            palette::MEDIA_SELECTED_BG
        } else {
            palette::PLAYBACK_PANEL_BG
        };
        f.render_widget(
            Block::default().style(Style::default().bg(bg)),
            Rect {
                x: content_area.x + SELECTED_BLOCK_SIDE_PADDING,
                y: row_y + 1,
                width: (text_w as u16).saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING),
                height: item_h.saturating_sub(2),
            },
        );
    }

    let marker = super::selection_marker(selected && focused && !expanded);
    f.render_widget(
        Paragraph::new(marker),
        Rect {
            x: content_area.x,
            y: title_y,
            width: 1,
            height: 1,
        },
    );

    let tx = content_area.x + SELECTED_BLOCK_SIDE_PADDING;
    let tw = (text_w as u16).saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING);
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

        self.render_power_compact_detail(
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
            layout,
        );
        layout.cursor_screen_y = Some(row_y + 1);
    }

    pub(super) fn render_power_home_video_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }
        self.ensure_lib_loaded_for(lib_idx);

        let mut content_area = area;
        layout.left_area = content_area;

        let (items, cursor, total_count) = {
            let lib = &self.libs[lib_idx];
            match lib.nav_stack.last() {
                // `total_count` is Emby's TotalRecordCount, not `items.len()` --
                // with lazy pagination `items` may only hold a subset of the
                // library until the user scrolls further.
                Some(lvl) => (lvl.items.clone(), lvl.cursor, lvl.total_count),
                None => return,
            }
        };

        let n = items.len();

        // Item count label (matches render_power_list style). Uses the
        // server-reported total, not `n`, for the reason above.
        if focused && content_area.height > 0 {
            content_area = super::render_power_count_label(f, content_area, total_count);
        }

        if n == 0 {
            return;
        }
        let current_pos = cursor.min(n.saturating_sub(1));

        // Conservatively assume scrollbar present to get text_w for height
        // calculations, then recheck once we know the real total height.
        let text_w_with_sb = (content_area.width as usize).saturating_sub(1);
        let mut item_heights = vec![1; n];
        let selected_panel_width =
            text_w_with_sb.saturating_sub(2 * SELECTED_BLOCK_SIDE_PADDING as usize) as u16;
        let selected_height = self
            .compact_banner_layout_with_overview(&items[current_pos], selected_panel_width, true)
            .content_rows()
            .saturating_add(5) as u16;
        let selected_index = current_pos;
        item_heights[selected_index] = selected_height;
        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > content_area.height;
        let text_w = super::power_content_width(content_area.width, needs_scrollbar);

        let mut scroll = {
            let mut s = 0usize;
            while s < current_pos {
                let visible_h: u16 = item_heights[s..=current_pos].iter().sum();
                if visible_h <= content_area.height {
                    break;
                }
                s += 1;
            }
            s
        };
        if scroll > current_pos {
            scroll = current_pos;
        }

        let mut row_y = content_area.y;
        let mut visible_items = 0usize;
        let mut row_map: Vec<Option<usize>> = Vec::with_capacity(content_area.height as usize);

        for (i, item) in items.iter().enumerate().skip(scroll) {
            if row_y >= content_area.y + content_area.height {
                break;
            }
            visible_items += 1;
            let item_h = item_heights[i];
            let selected = i == current_pos;
            if selected {
                layout.cursor_screen_y = Some(row_y);
            }
            render_home_video_item(
                f,
                item,
                row_y,
                item_h,
                content_area,
                text_w,
                selected,
                focused,
            );
            if selected {
                self.render_selected_home_video_detail(
                    f,
                    Rect {
                        width: text_w as u16,
                        ..content_area
                    },
                    row_y,
                    item_h,
                    lib_idx,
                    focused,
                    layout,
                );
            }
            let visible_rows = (content_area.y + content_area.height)
                .saturating_sub(row_y)
                .min(item_h);
            for _ in 0..visible_rows {
                row_map.push(Some(i));
            }
            row_y += item_h;
        }
        row_map.resize(content_area.height as usize, None);
        layout.left_row_map = row_map;

        // Scrollbar (hidden when unfocused, consistent with queue panel).
        if needs_scrollbar && focused {
            super::render_power_right_scrollbar_with_viewport(
                f,
                content_area,
                n,
                visible_items.max(1),
                scroll,
            );
        }
    }
}
