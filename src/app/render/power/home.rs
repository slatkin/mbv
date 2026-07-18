use super::super::super::ui_util::*;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

/// Clamp the panel scroll offset (in terminal rows, content-space) so the grid row
/// spanning `[cur_top, cur_bot)` is fully visible within a viewport of `view_h` rows,
/// and never scrolls past the end of `total_h` rows of content.
fn power_home_panel_scroll(
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

fn feed_added_date(date_added: &str) -> String {
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

    date_added
        .splitn(3, '-')
        .collect::<Vec<_>>()
        .as_slice()
        .windows(3)
        .next()
        .and_then(|p| {
            let y = p[0];
            let d: u32 = p[2].parse().ok()?;
            let m: usize = p[1].parse::<usize>().ok()?.checked_sub(1)?;
            Some(format!("Added {} {}, {}", d, MONTHS.get(m)?, y))
        })
        .unwrap_or_else(|| date_added.to_string())
}

fn home_video_item_height(item: &mbv_core::api::MediaItem, text_w: usize) -> u16 {
    if item.overview.is_empty() || text_w == 0 {
        3 // title + meta + separator
    } else {
        let ov_text = trunc_overview(&item.overview);
        let lines = wrap(&ov_text, text_w).len() as u16;
        3 + lines // title + meta + overview lines + separator
    }
}

fn render_home_video_item(
    f: &mut Frame,
    item: &mbv_core::api::MediaItem,
    row_y: u16,
    item_h: u16,
    content_area: Rect,
    text_w: usize,
    selected: bool,
    focused: bool,
    is_feed_lib: bool,
) {
    let marker = if selected && focused {
        Span::styled("\u{258c}", Style::default().fg(palette::PINE))
    } else {
        Span::raw(" ")
    };
    f.render_widget(
        Paragraph::new(marker),
        Rect {
            x: content_area.x,
            y: row_y,
            width: 1,
            height: 1,
        },
    );

    let tx = content_area.x + 1;
    let tw = (text_w.saturating_sub(1)) as u16;
    let title_color = if selected && focused {
        palette::IRIS
    } else {
        palette::TEXT
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
            y: row_y,
            width: tw,
            height: 1,
        },
    );

    if row_y + 1 < content_area.y + content_area.height {
        let mut meta_spans: Vec<Span> = Vec::new();
        if item.played {
            meta_spans.push(Span::styled(
                "\u{2713} ",
                Style::default().fg(palette::PINE),
            ));
        }
        let mut parts: Vec<String> = Vec::new();
        if is_feed_lib && !item.date_added.is_empty() {
            parts.push(feed_added_date(&item.date_added));
        }
        let dur_s = item.runtime_ticks / TICKS_PER_SECOND;
        if dur_s > 0 {
            parts.push(fmt_duration_approx(dur_s));
        }
        if !parts.is_empty() {
            meta_spans.push(Span::styled(
                parts.join("  "),
                Style::default().fg(palette::SUBTLE),
            ));
        }
        if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
            let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
            meta_spans.push(Span::styled(
                format!("  {}%", pct),
                Style::default().fg(palette::YELLOW),
            ));
        }
        f.render_widget(
            Paragraph::new(Line::from(meta_spans)),
            Rect {
                x: tx,
                y: row_y + 1,
                width: tw,
                height: 1,
            },
        );
    }

    if !item.overview.is_empty() && item_h >= 4 && row_y + 2 < content_area.y + content_area.height
    {
        let ov_text = trunc_overview(&item.overview);
        let wrapped = wrap(&ov_text, (tw as usize).max(1));
        let ov_color = if selected && focused {
            palette::WHITE
        } else {
            palette::MUTED
        };
        let ov_style = Style::default().fg(ov_color);
        for (li, line) in wrapped.iter().enumerate() {
            let ly = row_y + 2 + li as u16;
            if ly >= content_area.y + content_area.height {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(line.to_string(), ov_style)),
                Rect {
                    x: tx,
                    y: ly,
                    width: tw,
                    height: 1,
                },
            );
        }
    }

    let sep_y = row_y + item_h - 1;
    if sep_y < content_area.y + content_area.height {
        let sep_str = "\u{2500}".repeat(text_w);
        f.render_widget(
            Paragraph::new(Span::styled(sep_str, Style::default().fg(palette::MUTED))),
            Rect {
                x: content_area.x,
                y: sep_y,
                width: text_w as u16,
                height: 1,
            },
        );
    }
}

impl App {
    pub(super) fn render_power_home_video_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
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
            let count_label = format!(" {} items", total_count);
            f.render_widget(
                Paragraph::new(Span::styled(
                    count_label,
                    Style::default().fg(palette::SUBTLE),
                )),
                Rect {
                    height: 1,
                    ..content_area
                },
            );
            content_area = Rect {
                y: content_area.y + 1,
                height: content_area.height.saturating_sub(1),
                ..content_area
            };
        }

        if n == 0 {
            return;
        }

        let is_feed_lib = {
            let c = self.client.lock().unwrap();
            c.config
                .feed_view_libraries
                .contains(&self.libs[lib_idx].library.name.to_lowercase())
        };

        // Conservatively assume scrollbar present to get text_w for height
        // calculations, then recheck once we know the real total height.
        let text_w_with_sb = (content_area.width as usize).saturating_sub(1);
        let item_heights: Vec<u16> = items
            .iter()
            .map(|it| home_video_item_height(it, text_w_with_sb))
            .collect();
        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > content_area.height;
        let text_w =
            (content_area.width as usize).saturating_sub(if needs_scrollbar { 1 } else { 0 });

        let mut scroll = {
            let mut s = 0usize;
            while s < cursor {
                let visible_h: u16 = item_heights[s..=cursor].iter().sum();
                if visible_h <= content_area.height {
                    break;
                }
                s += 1;
            }
            s
        };
        if scroll > cursor {
            scroll = cursor;
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
            let selected = i == cursor;
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
                is_feed_lib,
            );
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
            super::render_power_scrollbar_with_viewport(
                f,
                content_area,
                n,
                visible_items.max(1),
                scroll,
            );
        }
    }

    pub(super) fn render_power_feed_home_video_group_view(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }
        self.ensure_lib_loaded_for(lib_idx);

        let Some(root_level) = self.libs[lib_idx].nav_stack.first() else {
            return;
        };
        let groups = self.libs[lib_idx]
            .feed_home_video
            .as_ref()
            .map(|state| {
                state
                    .groups
                    .iter()
                    .map(|group| group.folder.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected_group = self.feed_home_video_selected_group_index(lib_idx);
        let items = self.feed_home_video_selected_items(lib_idx);
        let (cursor, stored_scroll, loading) = self.libs[lib_idx]
            .feed_home_video
            .as_ref()
            .map(|state| (state.video_cursor, state.video_scroll, state.loading))
            .unwrap_or((0, 0, root_level.loading));

        let max_y = area.y + area.height;
        let mut row = area.y;
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();

        if row < max_y {
            row += 1;
        }
        if row < max_y {
            const MAX_LABEL: usize = 12;
            let tab_labels: Vec<String> = std::iter::once("All".to_string())
                .chain(
                    groups
                        .iter()
                        .map(|g| trunc_str(&g.name, MAX_LABEL).to_string()),
                )
                .collect();
            let n_tabs = tab_labels.len();
            let pill_widths: Vec<usize> = tab_labels.iter().map(|l| l.width() + 2).collect();
            let bar_w = area.width as usize;

            let count_fitting = |start: usize, avail: usize| -> usize {
                let mut used = 0usize;
                let mut count = 0usize;
                for width in pill_widths.iter().take(n_tabs).skip(start) {
                    let need = if count == 0 { *width } else { 1 + *width };
                    if used + need > avail {
                        break;
                    }
                    used += need;
                    count += 1;
                }
                count
            };

            let mut scroll_start = 0usize;
            loop {
                let avail = bar_w
                    .saturating_sub(if scroll_start > 0 { 2 } else { 0 })
                    .saturating_sub(2);
                let cnt = count_fitting(scroll_start, avail);
                if cnt == 0 || scroll_start + cnt > selected_group {
                    break;
                }
                scroll_start += 1;
            }

            let has_left = scroll_start > 0;
            let avail_pills = bar_w
                .saturating_sub(if has_left { 2 } else { 0 })
                .saturating_sub(2);
            let cnt = count_fitting(scroll_start, avail_pills);
            let scroll_end = (scroll_start + cnt).min(n_tabs);
            let has_right = scroll_end < n_tabs;

            let mut spans: Vec<Span> = Vec::new();
            let mut x_cursor = area.x;
            if has_left {
                let chunk = "\u{2039} ";
                spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
                x_cursor += chunk.width() as u16;
            }
            for (idx, label) in tab_labels[scroll_start..scroll_end].iter().enumerate() {
                if idx > 0 {
                    spans.push(Span::raw(" "));
                    x_cursor += 1;
                }
                let abs_idx = scroll_start + idx;
                let selected = abs_idx == selected_group;
                let style = if selected {
                    Style::default()
                        .fg(palette::YELLOW)
                        .bg(palette::FOAM)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::BASE).bg(palette::FOAM)
                };
                let pill = format!(" {} ", label);
                let pill_rect = Rect {
                    x: x_cursor,
                    y: row,
                    width: pill.width() as u16,
                    height: 1,
                };
                selector_tabs.push((pill_rect, abs_idx));
                spans.push(Span::styled(pill.clone(), style));
                x_cursor += pill.width() as u16;
            }
            if has_right {
                spans.push(Span::styled(
                    " \u{203a}",
                    Style::default().fg(palette::FOAM),
                ));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
        }
        if row < max_y {
            row += 1;
        }
        if row < max_y {
            row += 1;
        }
        layout.selector_tabs = selector_tabs;

        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        layout.left_area = list_area;
        if list_area.height == 0 {
            return;
        }

        if items.is_empty() {
            if row < max_y {
                let msg = if loading {
                    " Loading\u{2026}"
                } else {
                    " (empty)"
                };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        msg,
                        Style::default().fg(palette::MUTED),
                    ))),
                    Rect {
                        x: list_area.x,
                        y: list_area.y,
                        width: list_area.width,
                        height: 1,
                    },
                );
            }
            return;
        }

        let current_pos = cursor.min(items.len().saturating_sub(1));
        let text_w_with_sb = (list_area.width as usize).saturating_sub(1);
        let item_heights: Vec<u16> = items
            .iter()
            .map(|it| home_video_item_height(it, text_w_with_sb))
            .collect();
        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > list_area.height;
        let text_w = (list_area.width as usize).saturating_sub(if needs_scrollbar { 1 } else { 0 });

        let mut scroll = stored_scroll.min(items.len().saturating_sub(1));
        if current_pos < scroll {
            scroll = current_pos;
        }
        while scroll < current_pos {
            let visible_h: u16 = item_heights[scroll..=current_pos].iter().sum();
            if visible_h <= list_area.height {
                break;
            }
            scroll += 1;
        }
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            state.video_scroll = scroll;
        }

        let mut row_map: Vec<Option<usize>> = Vec::with_capacity(list_area.height as usize);
        let mut row_y = list_area.y;
        let mut visible_items = 0usize;
        for (item_idx, item) in items.iter().enumerate().skip(scroll) {
            if row_y >= list_area.y + list_area.height {
                break;
            }
            visible_items += 1;
            let item_h = item_heights[item_idx];
            let selected = item_idx == current_pos;
            if selected {
                layout.cursor_screen_y = Some(row_y);
            }
            render_home_video_item(
                f, item, row_y, item_h, list_area, text_w, selected, focused, true,
            );
            let visible_rows = (list_area.y + list_area.height)
                .saturating_sub(row_y)
                .min(item_h);
            for _ in 0..visible_rows {
                row_map.push(Some(item_idx));
            }
            row_y += item_h;
        }
        row_map.resize(list_area.height as usize, None);
        layout.left_row_map = row_map;

        if needs_scrollbar && focused {
            super::render_power_scrollbar_with_viewport(
                f,
                list_area,
                items.len(),
                visible_items.max(1),
                scroll,
            );
        }
    }

    pub(super) fn render_power_home_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 || area.width == 0 {
            return;
        }
        layout.left_area = area;

        struct Section {
            section_idx: usize,
            flat_start: usize,
            items: Vec<mbv_core::api::MediaItem>,
        }
        enum DisplayRow {
            Pills,
            Empty,
            Item(usize, Box<mbv_core::api::MediaItem>),
            Blank,
        }

        let continue_items = self.home.continue_items.clone();
        let latest = self.home.latest.clone();

        let mut flat = continue_items.len();
        let mut new_sections: Vec<Section> = Vec::new();
        for (idx, (_title, _lib, items, _cur)) in latest.iter().enumerate() {
            if items.is_empty() {
                flat += items.len();
                continue;
            }
            new_sections.push(Section {
                section_idx: idx + 1,
                flat_start: flat,
                items: items.clone(),
            });
            flat += items.len();
        }

        if !new_sections
            .iter()
            .any(|section| section.section_idx == self.home.section)
        {
            self.home.section = new_sections
                .first()
                .map(|section| section.section_idx)
                .unwrap_or(0);
        }

        let selected_new = new_sections
            .iter()
            .find(|section| section.section_idx == self.home.section);

        let mut rows: Vec<DisplayRow> = Vec::new();
        if continue_items.is_empty() {
            rows.push(DisplayRow::Empty);
        } else {
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(idx, Box::new(item)));
            }
        }
        if let Some(section) = selected_new {
            rows.push(DisplayRow::Blank);
            rows.push(DisplayRow::Pills);
            for (idx, item) in section.items.iter().cloned().enumerate() {
                rows.push(DisplayRow::Item(section.flat_start + idx, Box::new(item)));
            }
        }

        let visible_flat_indices: Vec<usize> = rows
            .iter()
            .filter_map(|row| match row {
                DisplayRow::Item(flat_idx, _) => Some(*flat_idx),
                _ => None,
            })
            .collect();
        if let Some(first) = visible_flat_indices.first() {
            if !visible_flat_indices.contains(&self.home.power_home_cursor) {
                self.home.power_home_cursor = *first;
            }
        } else {
            self.home.power_home_cursor = 0;
        }
        let cursor = self.home.power_home_cursor;

        let content_h = rows.len().max(1) as u16;
        let needs_scrollbar = content_h > area.height;
        let list_w = area
            .width
            .saturating_sub(if needs_scrollbar { 1 } else { 0 });
        let cursor_row = rows
            .iter()
            .position(|row| matches!(row, DisplayRow::Item(flat_idx, _) if *flat_idx == cursor))
            .unwrap_or(0) as u16;
        let scroll_y = power_home_panel_scroll(
            self.home.power_home_scroll as u16,
            cursor_row,
            cursor_row + 1,
            content_h,
            area.height,
        );
        self.home.power_home_scroll = scroll_y as usize;

        let mut hitmap: Vec<(Rect, usize)> = Vec::new();
        layout.selector_tabs = Vec::new();
        let visible = area.height.min(content_h.saturating_sub(scroll_y));
        for k in 0..visible {
            let row_idx = scroll_y as usize + k as usize;
            let sy = area.y + k;
            let row_rect = Rect {
                x: area.x,
                y: sy,
                width: list_w,
                height: 1,
            };
            match &rows[row_idx] {
                DisplayRow::Pills => {
                    self.render_power_home_section_pills_row(f, row_rect, layout);
                }
                DisplayRow::Empty => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled("(empty)", Style::default().fg(palette::MUTED)),
                        ])),
                        row_rect,
                    );
                }
                DisplayRow::Blank => {}
                DisplayRow::Item(flat_idx, item) => {
                    let selected_row = *flat_idx == cursor;
                    if selected_row {
                        layout.cursor_screen_y = Some(sy);
                    }

                    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
                        let mins = (item.runtime_ticks / TICKS_PER_SECOND / 60).max(1);
                        format!("{}m", mins)
                    } else {
                        String::new()
                    };
                    let avail = (list_w as usize).saturating_sub(1);
                    let name_w = avail.saturating_sub(dur_str.width());
                    let title = trunc_str(&item.display_name(), name_w);
                    let pad = name_w.saturating_sub(title.width());

                    let fg = if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let mut spans: Vec<Span> = if selected_row && focused {
                        vec![
                            Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                            Span::styled(
                                title,
                                Style::default()
                                    .fg(palette::IRIS)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]
                    } else {
                        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                    };
                    if !dur_str.is_empty() {
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                    }
                    f.render_widget(Paragraph::new(Line::from(spans)), row_rect);
                    hitmap.push((row_rect, *flat_idx));
                }
            }
        }

        layout.home.hitmap = hitmap;

        if needs_scrollbar && focused {
            let max_off = content_h.saturating_sub(area.height) as usize;
            super::render_power_scrollbar(f, area, max_off, scroll_y as usize);
        }
    }

    pub(super) fn render_power_home_section_pills_row(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPower,
    ) {
        if area.width == 0 || area.height == 0 {
            layout.selector_tabs = Vec::new();
            return;
        }

        let mut labels: Vec<(usize, String)> = Vec::new();
        for (idx, (title, _lib, items, _cur)) in self.home.latest.iter().enumerate() {
            if !items.is_empty() {
                labels.push((idx + 1, title.clone()));
            }
        }
        if labels.is_empty() {
            layout.selector_tabs = Vec::new();
            return;
        }
        if !labels
            .iter()
            .any(|(section_idx, _)| *section_idx == self.home.section)
        {
            self.home.section = labels[0].0;
        }

        const MAX_LABEL: usize = 18;
        let pill_widths: Vec<usize> = labels
            .iter()
            .map(|(_, label)| trunc_str(label, MAX_LABEL).width() + 2)
            .collect();
        let selected_pos = labels
            .iter()
            .position(|(section_idx, _)| *section_idx == self.home.section)
            .unwrap_or(0);
        let count_fitting = |start: usize, avail: usize| -> usize {
            let mut used = 0usize;
            let mut count = 0usize;
            for width in pill_widths.iter().skip(start) {
                let need = if count == 0 { *width } else { 1 + *width };
                if used + need > avail {
                    break;
                }
                used += need;
                count += 1;
            }
            count
        };

        let mut scroll_start = 0usize;
        loop {
            let avail = (area.width as usize)
                .saturating_sub(if scroll_start > 0 { 2 } else { 0 })
                .saturating_sub(2);
            let count = count_fitting(scroll_start, avail);
            if count == 0 || scroll_start + count > selected_pos {
                break;
            }
            scroll_start += 1;
        }

        let has_left = scroll_start > 0;
        let avail_pills = (area.width as usize)
            .saturating_sub(if has_left { 2 } else { 0 })
            .saturating_sub(2);
        let count = count_fitting(scroll_start, avail_pills);
        let scroll_end = (scroll_start + count).min(labels.len());
        let has_right = scroll_end < labels.len();

        let mut spans: Vec<Span> = Vec::new();
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();
        let mut x_cursor = area.x;
        if has_left {
            let chunk = "\u{2039} ";
            spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
            x_cursor += chunk.width() as u16;
        }
        for (idx, (section_idx, label)) in labels[scroll_start..scroll_end].iter().enumerate() {
            if idx > 0 {
                spans.push(Span::raw(" "));
                x_cursor += 1;
            }
            let selected = *section_idx == self.home.section;
            let style = if selected {
                Style::default()
                    .fg(palette::YELLOW)
                    .bg(palette::FOAM)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::BASE).bg(palette::FOAM)
            };
            let label = trunc_str(label, MAX_LABEL);
            let pill = format!(" {label} ");
            let pill_rect = Rect {
                x: x_cursor,
                y: area.y,
                width: pill.width() as u16,
                height: 1,
            };
            selector_tabs.push((pill_rect, *section_idx));
            spans.push(Span::styled(pill.clone(), style));
            x_cursor += pill.width() as u16;
        }
        if has_right {
            spans.push(Span::styled(
                " \u{203a}",
                Style::default().fg(palette::FOAM),
            ));
        }

        f.render_widget(Paragraph::new(Line::from(spans)), area);
        layout.selector_tabs = selector_tabs;
    }
}

#[cfg(test)]
mod tests {
    use super::power_home_panel_scroll;
    use crate::app::layout::AppLayout;
    use crate::app::tests::{make_app_stub, make_items};
    use mbv_core::api::TICKS_PER_SECOND;
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn renders_keep_watching_then_selected_new_section() {
        let mut app = make_app_stub();

        let mut cont = make_items(3);
        for (i, it) in cont.iter_mut().enumerate() {
            it.name = ["Taskmaster", "QI XL", "8 Diagram Pole Fighter"][i].to_string();
            it.runtime_ticks = (2820 + i as i64 * 600) * TICKS_PER_SECOND;
        }
        app.home.continue_items = cont;

        let music = {
            let mut v = make_items(3);
            for (i, it) in v.iter_mut().enumerate() {
                it.name = ["King Of America", "Either/Or", "Too-Rye-Ay"][i].to_string();
            }
            v
        };
        let youtube = {
            let mut v = make_items(2);
            for (i, it) in v.iter_mut().enumerate() {
                it.name = ["NXL Not-E3 Showcase", "Comedians Taking Over"][i].to_string();
            }
            v
        };
        app.home.latest = vec![
            ("New Music".into(), "l1".into(), music, 0),
            ("YouTube".into(), "l2".into(), youtube, 0),
        ];

        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 20);
            app.render_power_home_list(f, area, true, &mut layout.power);
        })
        .unwrap();

        let out = buffer_to_string(&term);
        println!("\n{out}");

        assert!(out.contains("Taskmaster"));
        assert!(out.contains("QI XL"));
        assert!(out.contains("8 Diagram Pole Fighter"));
        assert!(out.contains("New Music"));
        assert!(out.contains("YouTube"));
        assert!(out.contains("King Of America"));
        assert!(out.contains("Either/Or"));
        assert!(!out.contains("NXL Not-E3 Showcase"));
        // Durations render as minutes only, never hours (67m for 4020s, not 1h07m).
        assert!(out.contains("47m"));
        assert!(out.contains("67m"));
        assert!(!out.contains("1h"));
        assert_eq!(layout.power.home.hitmap.len(), 6);
    }

    #[test]
    fn keeps_current_offset_when_row_already_visible() {
        // Row [2,6) fits inside viewport [0,10); offset unchanged.
        assert_eq!(power_home_panel_scroll(0, 2, 6, 20, 10), 0);
    }

    #[test]
    fn scrolls_down_to_reveal_row_below_viewport() {
        // Row [14,20) is below viewport [0,10); scroll so its bottom is visible.
        assert_eq!(power_home_panel_scroll(0, 14, 20, 30, 10), 10);
    }

    #[test]
    fn scrolls_up_to_reveal_row_above_viewport() {
        // Row [2,6) is above current offset 8; snap up to its top.
        assert_eq!(power_home_panel_scroll(8, 2, 6, 30, 10), 2);
    }

    #[test]
    fn never_scrolls_past_end() {
        // Cursor is the last row [11,15); offset clamped to total_h - view_h = 5.
        assert_eq!(power_home_panel_scroll(99, 11, 15, 15, 10), 5);
    }
}
