use super::super::super::ui_util::*;
use super::POWER_RENDER_FILTER;
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
fn format_release_date(premiere_date: &str) -> String {
    parse_ymd(premiere_date)
        .map(|(y, month, d)| format!("{d} {} {y}", &month[..3]))
        .unwrap_or_else(|| premiere_date.to_string())
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
) {
    let expanded = selected && item_h > 1;
    let title_y = row_y + if expanded { 2 } else { 0 };

    if expanded {
        f.render_widget(
            Block::default().style(Style::default().bg(palette::MEDIA_SELECTED_BG)),
            Rect {
                x: content_area.x,
                y: row_y + 1,
                width: text_w as u16,
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

    let tx = content_area.x + 1;
    let tw = (text_w.saturating_sub(1)) as u16;
    let title_color = if expanded {
        palette::YELLOW
    } else if selected && focused {
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

/// Pre-wrapped content for the Keep Watching hero panel's metadata column,
/// plus the total row count it needs. Computed once (mirroring
/// `compact_banner_layout`'s measure-before-render pattern) so the caller
/// can size the panel to fit before rendering, and so the title and
/// overview are wrapped exactly once per frame rather than once to measure
/// and again to render.
struct KeepWatchingHeroLayout {
    title_lines: Vec<String>,
    show_name: String,
    overview_lines: Vec<String>,
    height: u16,
}

impl App {
    fn render_selected_home_video_detail(
        &mut self,
        f: &mut Frame,
        content_area: Rect,
        row_y: u16,
        item_h: u16,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        let detail_height = item_h.saturating_sub(5);
        if detail_height == 0 {
            return;
        }

        self.render_power_compact_detail(
            f,
            Rect {
                x: content_area.x + 1,
                y: row_y + 3,
                width: content_area.width.saturating_sub(2),
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
        let selected_panel_width = text_w_with_sb.saturating_sub(2) as u16;
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
            const MAX_LABEL: usize = 12;
            let labels: Vec<String> = std::iter::once("All".to_string())
                .chain(
                    groups
                        .iter()
                        .map(|g| trunc_str(&g.name, MAX_LABEL).to_string()),
                )
                .collect();
            // Tabs are identified by 0-based index (0 = "All").
            let ids: Vec<usize> = (0..labels.len()).collect();
            selector_tabs = super::render_pill_bar(
                f,
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                super::PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: selected_group,
                    prefix: None,
                    underlay: super::PillUnderlay::Blank { fill: false },
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
                super::render_power_placeholder(
                    f,
                    Rect {
                        x: list_area.x,
                        y: list_area.y,
                        width: list_area.width,
                        height: 1,
                    },
                    msg,
                );
            }
            return;
        }

        let current_pos = cursor.min(items.len().saturating_sub(1));
        let text_w_with_sb = (list_area.width as usize).saturating_sub(1);
        let mut item_heights = vec![1; items.len()];
        let selected_panel_width = text_w_with_sb.saturating_sub(2) as u16;
        let selected_height = self
            .compact_banner_layout_with_overview(&items[current_pos], selected_panel_width, true)
            .content_rows()
            .saturating_add(5) as u16;
        item_heights[current_pos] = selected_height;
        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > list_area.height;
        let text_w = super::power_content_width(list_area.width, needs_scrollbar);

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
            render_home_video_item(f, item, row_y, item_h, list_area, text_w, selected, focused);
            if selected {
                self.render_selected_home_video_detail(
                    f,
                    Rect {
                        width: text_w as u16,
                        ..list_area
                    },
                    row_y,
                    item_h,
                    lib_idx,
                    focused,
                    layout,
                );
            }
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
            super::render_power_right_scrollbar_with_viewport(
                f,
                list_area,
                items.len(),
                visible_items.max(1),
                scroll,
            );
        }
    }

    /// Image types to request for the Keep Watching hero panel, mirroring
    /// the per-type conventions used for the queue card (`render_power_card`).
    fn keep_watching_hero_image_types(item: &mbv_core::api::MediaItem) -> &'static [&'static str] {
        match item.item_type.as_str() {
            "Movie" => &["Backdrop", "Primary", "Logo"],
            _ => &["Primary", "Backdrop"],
        }
    }

    /// Builds the Keep Watching hero panel's metadata layout for `item` at
    /// the meta column's width: title wrap lines, then one row each for the
    /// show-name line, the duration/progress line, and the blank separator,
    /// then the wrapped overview.
    fn keep_watching_hero_layout(
        item: &mbv_core::api::MediaItem,
        text_w: usize,
    ) -> KeepWatchingHeroLayout {
        if text_w == 0 {
            return KeepWatchingHeroLayout {
                title_lines: Vec::new(),
                show_name: String::new(),
                overview_lines: Vec::new(),
                height: 0,
            };
        }
        let title_lines: Vec<String> = wrap(&item.name, text_w)
            .into_iter()
            .map(|s| s.into_owned())
            .collect();
        let show_name = if item.item_type == "Episode" {
            item.series_name.clone()
        } else {
            String::new()
        };
        let overview_lines: Vec<String> = if item.overview.is_empty() {
            Vec::new()
        } else {
            wrap(&clean_overview(&item.overview), text_w)
                .into_iter()
                .map(|s| s.into_owned())
                .collect()
        };
        let height = title_lines.len() as u16 // title
            + 1 // show name row
            + 1 // duration / progress row
            + 1 // blank separator row
            + overview_lines.len() as u16; // overview
        KeepWatchingHeroLayout {
            title_lines,
            show_name,
            overview_lines,
            height,
        }
    }

    /// Renders the Keep Watching hero panel's image column into `area`,
    /// top-aligned (with a one-row pad so it isn't flush against the top of
    /// the panel) and horizontally centered. The column is a fixed reserved
    /// box (unlike the queue card's growing/shrinking slot), so a dim
    /// placeholder simply fills it while no artwork is ready yet.
    fn render_keep_watching_hero_image(&mut self, f: &mut Frame, area: Rect, cache_key: &str) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let top_pad = 1u16.min(area.height.saturating_sub(1));
        let img_area = Rect {
            x: area.x,
            y: area.y + top_pad,
            width: area.width,
            height: area.height - top_pad,
        };
        if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = Size {
                width: img_area.width,
                height: img_area.height,
            };
            if let Some(actual) = state.size_for(
                ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                avail,
            ) {
                let img_rect = Rect {
                    x: img_area.x + img_area.width.saturating_sub(actual.width) / 2,
                    y: img_area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
                    img_rect,
                    state,
                );
                return;
            }
        }
        f.render_widget(
            Block::default().style(Style::default().bg(palette::OVERLAY)),
            img_area,
        );
    }

    /// Renders the Keep Watching hero panel's metadata column for the
    /// focused item: episode title (yellow, wraps), show name (green), a
    /// duration/percent-watched line, a blank separator row, then the full
    /// overview (the caller sizes the panel via
    /// `keep_watching_hero_meta_height` so nothing here gets clipped).
    fn render_keep_watching_hero_meta(
        &self,
        f: &mut Frame,
        area: Rect,
        item: &mbv_core::api::MediaItem,
        layout: &KeepWatchingHeroLayout,
        focused: bool,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let text_w = area.width as usize;
        let mut row = area.y;
        let max_y = area.y + area.height;

        for line in &layout.title_lines {
            if row >= max_y {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(
                    line.clone(),
                    Style::default()
                        .fg(palette::YELLOW)
                        .add_modifier(Modifier::BOLD),
                )),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
        }

        if row < max_y {
            if !layout.show_name.is_empty() {
                f.render_widget(
                    Paragraph::new(Span::styled(
                        trunc_str(&layout.show_name, text_w),
                        Style::default().fg(palette::AQUA),
                    )),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
            }
            row += 1;
        }

        if row < max_y {
            let release_date = if item.premiere_date.is_empty() {
                String::new()
            } else {
                format_release_date(&item.premiere_date)
            };
            let dur_str = if item.runtime_ticks > 0 {
                fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
            } else {
                String::new()
            };
            let progress_span =
                if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                    let pct =
                        (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    Some(Span::styled(
                        format!("{}% watched", pct),
                        Style::default().fg(palette::GREEN),
                    ))
                } else if !item.played {
                    Some(Span::styled(
                        "Unwatched",
                        Style::default().fg(palette::MUTED),
                    ))
                } else {
                    None
                };

            let mut spans: Vec<Span> = Vec::new();
            if !release_date.is_empty() {
                spans.push(Span::styled(
                    release_date,
                    Style::default().fg(palette::SUBTLE),
                ));
            }
            if !dur_str.is_empty() {
                if !spans.is_empty() {
                    spans.push(Span::raw("  "));
                }
                spans.push(Span::styled(
                    trunc_str(&dur_str, text_w),
                    Style::default().fg(palette::SUBTLE),
                ));
            }
            if let Some(progress_span) = progress_span {
                if !spans.is_empty() {
                    spans.push(Span::raw("  "));
                }
                spans.push(progress_span);
            }
            if !spans.is_empty() {
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
            row += 1;
        }

        row += 1; // blank separator row

        let ov_color = if focused {
            palette::WHITE
        } else {
            palette::MUTED
        };
        for line in &layout.overview_lines {
            if row >= max_y {
                break;
            }
            f.render_widget(
                Paragraph::new(Span::styled(line.clone(), Style::default().fg(ov_color))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );
            row += 1;
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

        struct Section {
            section_idx: usize,
            flat_start: usize,
            items: Vec<mbv_core::api::MediaItem>,
        }
        enum DisplayRow {
            Empty,
            Item(usize, Box<mbv_core::api::MediaItem>),
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

        if self.home.section != 0
            && !new_sections
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

        self.render_power_home_section_pills_row(
            f,
            Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            },
            layout,
        );
        let content_area = Rect {
            y: area.y.saturating_add(2),
            height: area.height.saturating_sub(2),
            ..area
        };

        let mut rows: Vec<DisplayRow> = Vec::new();
        if self.home.section == 0 {
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(idx, Box::new(item)));
            }
        } else if let Some(section) = selected_new {
            for (idx, item) in section.items.iter().cloned().enumerate() {
                rows.push(DisplayRow::Item(section.flat_start + idx, Box::new(item)));
            }
        }
        if rows.is_empty() {
            rows.push(DisplayRow::Empty);
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

        // --- Home hero panel ----------------------------------------------
        // Shared hero above the selected Home list. It reflects the current
        // flat cursor item whether the active pill is Continue Watching or one
        // of the Newest sections.
        let hero_item = self.power_home_current_item();
        let hero: Option<(mbv_core::api::MediaItem, u16, KeepWatchingHeroLayout)> =
            if area.width < 24 {
                None
            } else {
                hero_item.and_then(|item| {
                    let img_w = ((area.width as u32 * 2 / 5) as u16)
                        .clamp(12, 32)
                        .min(area.width.saturating_sub(12));
                    let meta_w = area.width.saturating_sub(img_w + 1) as usize;
                    let mut meta_layout = Self::keep_watching_hero_layout(&item, meta_w);
                    let max_allowed = content_area.height.saturating_sub(7);
                    meta_layout.height = meta_layout.height.min(max_allowed);
                    if meta_layout.height < 4 {
                        None
                    } else {
                        Some((item, img_w, meta_layout))
                    }
                })
            };
        let hero_h: u16 = hero
            .as_ref()
            .map(|(_, _, l)| {
                let max_allowed = content_area.height.saturating_sub(7);
                l.height.max(10).min(max_allowed)
            })
            .unwrap_or(0);

        let list_area = Rect {
            y: content_area.y + hero_h + 2,
            height: content_area.height.saturating_sub(hero_h + 2),
            ..content_area
        };
        layout.left_area = list_area;

        if let Some((item, img_w, meta_layout)) = &hero {
            let img_w = *img_w;
            let hero_area = Rect {
                x: content_area.x,
                y: content_area.y,
                width: content_area.width,
                height: hero_h,
            };
            let meta_area = Rect {
                x: hero_area.x,
                y: hero_area.y,
                width: hero_area.width.saturating_sub(img_w + 1),
                height: hero_h,
            };
            let img_area = Rect {
                x: hero_area.x + hero_area.width.saturating_sub(img_w),
                y: hero_area.y,
                width: img_w,
                height: hero_h,
            };

            let cache_key = format!("{}:pwr_kw", item.id);
            if self.images_enabled() {
                let img_types = Self::keep_watching_hero_image_types(item);
                self.fetch_card_image(
                    cache_key.clone(),
                    item.id.clone(),
                    item.series_id.clone(),
                    img_types,
                );
            }
            self.render_keep_watching_hero_image(f, img_area, &cache_key);
            self.render_keep_watching_hero_meta(f, meta_area, item, meta_layout, focused);
        }

        let content_h = rows.len().max(1) as u16;
        let needs_scrollbar = content_h > list_area.height;
        let list_w = super::power_content_width(list_area.width, needs_scrollbar) as u16;
        let cursor_row = rows
            .iter()
            .position(|row| matches!(row, DisplayRow::Item(flat_idx, _) if *flat_idx == cursor))
            .unwrap_or(0) as u16;
        let scroll_y = power_home_panel_scroll(
            self.home.power_home_scroll as u16,
            cursor_row,
            cursor_row + 1,
            content_h,
            list_area.height,
        );
        self.home.power_home_scroll = scroll_y as usize;

        let mut hitmap: Vec<(Rect, usize)> = Vec::new();

        let visible = list_area.height.min(content_h.saturating_sub(scroll_y));
        for k in 0..visible {
            let row_idx = scroll_y as usize + k as usize;
            let sy = list_area.y + k;
            let row_rect = Rect {
                x: list_area.x,
                y: sy,
                width: list_w,
                height: 1,
            };
            match &rows[row_idx] {
                DisplayRow::Empty => {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled("(empty)", Style::default().fg(palette::MUTED)),
                        ])),
                        row_rect,
                    );
                }
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
                    let avail = (row_rect.width as usize).saturating_sub(1);
                    // Reserve a 6-column gap before the duration column so the title
                    // truncates well before running up against it, plus a 1-column
                    // pad after the duration so it isn't flush against the right edge.
                    const DUR_GAP: usize = 6;
                    let dur_reserve = if dur_str.is_empty() {
                        0
                    } else {
                        dur_str.width() + DUR_GAP + 1
                    };
                    let name_w = avail.saturating_sub(dur_reserve);
                    let title = trunc_str(&item.display_name(), name_w);
                    // The gap between title and duration grows to fill whatever
                    // `name_w` didn't need, so it's just what's left of `avail`
                    // after the title and duration (DUR_GAP only sets where
                    // truncation kicks in, above).
                    let pad = avail.saturating_sub(title.width() + dur_str.width() + 1);

                    let fg = if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let mut spans: Vec<Span> = if selected_row && focused {
                        vec![
                            super::selection_marker(true),
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
            let max_off = content_h.saturating_sub(list_area.height) as usize;
            super::render_power_right_scrollbar(f, list_area, max_off, scroll_y as usize);
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

        let mut labels: Vec<(usize, String)> = vec![(0, "Continue Watching".to_string())];
        for (idx, (title, _lib, items, _cur)) in self.home.latest.iter().enumerate() {
            if !items.is_empty() {
                labels.push((idx + 1, title.clone()));
            }
        }
        if !labels
            .iter()
            .any(|(section_idx, _)| *section_idx == self.home.section)
        {
            self.home.section = labels[0].0;
        }

        const MAX_LABEL: usize = 18;
        let selected_pos = labels
            .iter()
            .position(|(section_idx, _)| *section_idx == self.home.section)
            .unwrap_or(0);
        // Pre-truncated pill labels; ids are the section indices (idx+1) used
        // as click targets, distinct from the pill's display position.
        let label_strs: Vec<String> = labels
            .iter()
            .map(|(_, label)| trunc_str(label, MAX_LABEL).to_string())
            .collect();
        let ids: Vec<usize> = labels.iter().map(|(section_idx, _)| *section_idx).collect();
        layout.selector_tabs = super::render_pill_bar(
            f,
            area,
            super::PillBar {
                labels: &label_strs,
                ids: &ids,
                selected_pos,
                prefix: None,
                underlay: super::PillUnderlay::Blank { fill: false },
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::power_home_panel_scroll;
    use crate::app::layout::AppLayout;
    use crate::app::tests::{make_app_stub, make_item, make_items};
    use crate::app::{palette, BrowseLevel, FeedHomeVideoGroup, FeedHomeVideoState, LibraryTab};
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

    fn assert_selected_home_video_panel(term: &Terminal<TestBackend>, title: &str) {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let (title_y, title_x) = (0..area.height)
            .find_map(|y| {
                let line: String = (0..area.width).map(|x| buf[(x, y)].symbol()).collect();
                line.find(title).map(|x| (y, x as u16))
            })
            .expect("selected home-video title should be present in the buffer");
        assert_eq!(
            buf[(title_x, title_y)].fg,
            palette::YELLOW,
            "selected home-video title should be yellow"
        );

        let row_is = |y: u16, glyph: &str| (0..area.width).all(|x| buf[(x, y)].symbol() == glyph);
        let top_y = (0..area.height)
            .find(|&y| row_is(y, "▁"))
            .expect("selected home-video top border should render");
        let bottom_y = (0..area.height)
            .find(|&y| row_is(y, "▔"))
            .expect("selected home-video bottom border should render");
        assert!(top_y < title_y && title_y < bottom_y);
    }

    #[test]
    fn renders_home_pills_and_only_selected_section() {
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
            v[0].overview = "Newest metadata overview appears in the shared Home hero.".into();
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
            ("Music".into(), "l1".into(), music, 0),
            ("YouTube".into(), "l2".into(), youtube, 0),
        ];

        let backend = TestBackend::new(80, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 30);
            app.render_power_home_list(f, area, true, &mut layout.power);
        })
        .unwrap();

        let out = buffer_to_string(&term);
        println!("\n{out}");

        assert!(out.contains("Taskmaster"));
        assert!(out.contains("QI XL"));
        assert!(out.contains("8 Diagram Pole Fighter"));
        assert!(out.contains("Continue Watching"));
        assert!(out.contains("Music"));
        assert!(out.contains("YouTube"));
        assert!(!out.contains("King Of America"));
        assert!(!out.contains("Either/Or"));
        assert!(!out.contains("NXL Not-E3 Showcase"));
        // Durations render as minutes only, never hours (67m for 4020s, not 1h07m).
        assert!(out.contains("47m"));
        assert!(out.contains("67m"));
        assert!(!out.contains("1h"));
        assert_eq!(layout.power.home.hitmap.len(), 3);
        assert_eq!(layout.power.selector_tabs.len(), 3);

        app.power_home_select_section(1);
        let backend = TestBackend::new(80, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            let area = Rect::new(0, 0, 80, 30);
            app.render_power_home_list(f, area, true, &mut layout.power);
        })
        .unwrap();

        let out = buffer_to_string(&term);
        println!("\n{out}");

        assert!(!out.contains("Taskmaster"));
        assert!(out.contains("King Of America"));
        assert!(out.contains("Newest metadata overview appears"));
        assert!(out.contains("Either/Or"));
        assert!(!out.contains("NXL Not-E3 Showcase"));
        assert_eq!(layout.power.home.hitmap.len(), 3);
    }

    #[test]
    fn home_list_does_not_draw_selected_media_box() {
        let mut app = make_app_stub();
        let mut cont = make_items(3);
        for (i, it) in cont.iter_mut().enumerate() {
            it.name = format!("Continue {i}");
        }
        app.home.continue_items = cont;
        app.home.latest = vec![
            ("Music".into(), "l1".into(), make_items(2), 0),
            ("YouTube".into(), "l2".into(), make_items(2), 0),
        ];

        let backend = TestBackend::new(26, 16);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            app.render_power_home_list(f, Rect::new(2, 2, 20, 14), true, &mut layout.power);
        })
        .unwrap();

        let out = buffer_to_string(&term);
        assert!(!out.contains('\u{2581}'), "unexpected top border:\n{out}");
        assert!(
            !out.contains('\u{2594}'),
            "unexpected bottom border:\n{out}"
        );
    }

    fn make_home_video_panel_app() -> crate::app::App {
        let mut app = make_app_stub();
        app.image_protocol_enabled = true;
        app.power_left_tab = 1;

        let mut library = make_item("Home Videos", "CollectionFolder");
        library.id = "lib-homevideos".into();
        library.is_folder = true;
        library.collection_type = "homevideos".into();

        let mut selected = make_item("Selected Home Video", "Video");
        selected.id = "video-selected".into();
        selected.overview = "Selected home-video overview.".into();
        selected.runtime_ticks = 25 * 60 * TICKS_PER_SECOND;
        let mut other = make_item("Other Home Video", "Video");
        other.id = "video-other".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-homevideos".into(),
                title: "Home Videos".into(),
                items: vec![selected.clone(), other],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: Some(FeedHomeVideoState {
                all_items: vec![make_item("Other Feed Video", "Video"), selected],
                groups: vec![FeedHomeVideoGroup {
                    folder: make_item("Feed", "Folder"),
                    items: Vec::new(),
                }],
                loading: false,
                selected_group: 0,
                video_cursor: 0,
                video_scroll: 0,
            }),
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    #[test]
    fn selected_regular_home_video_keeps_detail_below_title() {
        let mut app = make_home_video_panel_app();
        app.libs[0].feed_home_video = None;

        let backend = TestBackend::new(60, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            app.render_power_home_video_list(
                f,
                Rect::new(0, 0, 60, 30),
                0,
                true,
                &mut layout.power,
            );
        })
        .unwrap();

        let out = buffer_to_string(&term);
        assert_selected_home_video_panel(&term, "Selected Home Video");
        let title = out
            .find("Selected Home Video")
            .expect("selected home-video title should render");
        let overview = out
            .find("Selected home-video overview.")
            .unwrap_or_else(|| panic!("selected home-video detail should render:\n{out}"));
        let other = out
            .find("Other Home Video")
            .expect("following home-video row should render");
        assert!(
            title < overview && overview < other,
            "unexpected render order:\n{out}"
        );
        assert_eq!(layout.power.cursor_screen_y, Some(2));
        assert_eq!(layout.power.left_row_map[0], Some(0));
        let other_row = layout
            .power
            .left_row_map
            .iter()
            .position(|row| *row == Some(1))
            .expect("unselected home-video row should map to the display");
        assert!(other_row + 1 < layout.power.left_row_map.len());
        assert_eq!(
            layout
                .power
                .left_row_map
                .get(other_row + 1)
                .copied()
                .flatten(),
            None,
            "unselected home-video rows should occupy one line"
        );
    }

    #[test]
    fn selected_grouped_feed_home_video_keeps_detail_and_scroll_state() {
        let mut app = make_home_video_panel_app();
        app.client
            .lock()
            .unwrap()
            .config
            .feed_view_libraries
            .push("home videos".into());
        let feed_state = app.libs[0].feed_home_video.as_mut().unwrap();
        feed_state.video_cursor = 1;
        feed_state.video_scroll = 1;

        let backend = TestBackend::new(60, 30);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = AppLayout::default();
        term.draw(|f| {
            app.render_power_feed_home_video_group_view(
                f,
                Rect::new(0, 0, 60, 30),
                0,
                true,
                &mut layout.power,
            );
        })
        .unwrap();

        let out = buffer_to_string(&term);
        assert_selected_home_video_panel(&term, "Selected Home Video");
        assert!(out.contains("All"), "feed selector should render:\n{out}");
        let title = out
            .find("Selected Home Video")
            .expect("selected feed home-video title should render");
        let overview = out
            .find("Selected home-video overview.")
            .unwrap_or_else(|| panic!("selected feed home-video detail should render:\n{out}"));
        assert!(title < overview, "unexpected render order:\n{out}");
        assert_eq!(
            app.libs[0].feed_home_video.as_ref().unwrap().video_scroll,
            1
        );
        assert_eq!(layout.power.left_row_map[0], Some(1));
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
