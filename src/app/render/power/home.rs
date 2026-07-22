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

fn feed_added_date(date_added: &str) -> String {
    parse_ymd(date_added)
        .map(|(y, month, d)| format!("Added {d} {month}, {y}"))
        .unwrap_or_else(|| date_added.to_string())
}

/// Formats an Emby `PremiereDate` value (e.g. `2015-06-19T00:00:00.0000000Z`)
/// as a release date like "19 Jun 2015".
fn format_release_date(premiere_date: &str) -> String {
    parse_ymd(premiere_date)
        .map(|(y, month, d)| format!("{d} {} {y}", &month[..3]))
        .unwrap_or_else(|| premiere_date.to_string())
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
    let marker = super::selection_marker(selected && focused);
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
                Style::default().fg(palette::AQUA),
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
        super::render_horizontal_rule(
            f,
            Rect {
                x: content_area.x,
                y: sep_y,
                width: text_w as u16,
                height: 1,
            },
            palette::MUTED,
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
        let text_w = super::power_content_width(content_area.width, needs_scrollbar);

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
        let item_heights: Vec<u16> = items
            .iter()
            .map(|it| home_video_item_height(it, text_w_with_sb))
            .collect();
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
            Pills,
            Empty,
            Item(usize, Box<mbv_core::api::MediaItem>),
            Blank,
        }

        let continue_items = self.home.continue_items.clone();
        let latest = self.home.latest.clone();

        // --- Keep Watching hero panel -------------------------------------
        // A carousel-like image + metadata panel above the Keep Watching
        // list, reflecting the item currently under the cursor, separated
        // from the list below by blank rows. The panel is sized to
        // fit its metadata column's content (title + show name + duration
        // line + full overview) rather than a fixed height, so the overview
        // never gets clipped; it's still capped by how much of `area` can be
        // spared without starving the list underneath.
        // Borrows from `continue_items` (a local owned Vec, not `self`), so it's
        // fine to hold this reference across the `&mut self` calls below and
        // avoid a second clone of the item on top of the one already made above.
        let hero: Option<(&mbv_core::api::MediaItem, u16, KeepWatchingHeroLayout)> =
            if continue_items.is_empty() || area.width < 24 {
                None
            } else {
                continue_items
                    .get(self.home.power_home_cursor)
                    .or_else(|| continue_items.first())
                    .and_then(|item| {
                        let img_w = ((area.width as u32 * 2 / 5) as u16)
                            .clamp(12, 32)
                            .min(area.width.saturating_sub(12));
                        let meta_w = area.width.saturating_sub(img_w + 1) as usize;
                        let mut meta_layout = Self::keep_watching_hero_layout(item, meta_w);
                        let max_allowed = area.height.saturating_sub(7); // leave room for blank rows + list
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
                // Minimum height to prevent screen jitter when browsing the list,
                // capped by available space so short terminals still fit the list.
                let max_allowed = area.height.saturating_sub(7);
                l.height.max(10).min(max_allowed)
            })
            .unwrap_or(0);

        let list_area = if hero_h > 0 {
            Rect {
                y: area.y + hero_h + 2,
                height: area.height.saturating_sub(hero_h + 2),
                ..area
            }
        } else {
            area
        };
        layout.left_area = list_area;

        if let Some((item, img_w, meta_layout)) = &hero {
            let img_w = *img_w;
            let hero_area = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
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
        let continue_row_count;
        let new_section_count;
        if continue_items.is_empty() {
            rows.push(DisplayRow::Empty);
            continue_row_count = 1;
            new_section_count = 0u16;
        } else {
            continue_row_count = continue_items.len() as u16;
            for (idx, item) in continue_items.into_iter().enumerate() {
                rows.push(DisplayRow::Item(idx, Box::new(item)));
            }
            new_section_count = selected_new.as_ref().map(|s| s.items.len()).unwrap_or(0) as u16;
        }
        if let Some(section) = selected_new {
            rows.push(DisplayRow::Blank);
            rows.push(DisplayRow::Blank);
            rows.push(DisplayRow::Pills);
            rows.push(DisplayRow::Blank);
            rows.push(DisplayRow::Blank);
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
        layout.selector_tabs = Vec::new();

        let new_section_start = continue_row_count as usize + 5;

        // Colored section blocks use the same padded left edge as the selected
        // row blocks on the library tabs. The whole block stays inside
        // `list_area`; only row text decides its own internal spacing.
        let block_x = list_area.x;
        let block_w = list_area.width;

        // Background for the continue watching list.
        // Includes 1 top and 1 bottom padding row so the items are not flush
        // against the block borders; borders sit outside the block.
        let continue_bg_h = continue_row_count.min(list_area.height);
        if continue_bg_h >= 1 {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::CONTINUE_BG)),
                Rect {
                    x: block_x,
                    y: list_area.y.saturating_sub(1),
                    width: block_w,
                    height: continue_bg_h + 2,
                },
            );
            let border_style = Style::default().fg(palette::SOFT_WHITE);
            // Top border at the row above the block's top padding.
            let top_y = list_area.y.saturating_sub(2);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2581}".repeat(block_w as usize),
                    border_style,
                ))),
                Rect {
                    x: block_x,
                    y: top_y,
                    width: block_w,
                    height: 1,
                },
            );
            // Bottom border at the row below the block's bottom padding.
            let bot_y = list_area.y + continue_bg_h + 1;
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2594}".repeat(block_w as usize),
                    border_style,
                ))),
                Rect {
                    x: block_x,
                    y: bot_y,
                    width: block_w,
                    height: 1,
                },
            );
        }

        // Background for the newest section, same style as continue watching.
        if new_section_count >= 1 {
            let new_bg_y = list_area.y + (new_section_start as u16).saturating_sub(scroll_y);
            let new_bg_h = new_section_count.min(list_area.height);
            f.render_widget(
                Block::default().style(Style::default().bg(palette::CONTINUE_BG)),
                Rect {
                    x: block_x,
                    y: new_bg_y.saturating_sub(1),
                    width: block_w,
                    height: new_bg_h + 2,
                },
            );
            let border_style = Style::default().fg(palette::SOFT_WHITE);
            // Top border at the row above the block's top padding.
            let top_y = new_bg_y.saturating_sub(2);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2581}".repeat(block_w as usize),
                    border_style,
                ))),
                Rect {
                    x: block_x,
                    y: top_y,
                    width: block_w,
                    height: 1,
                },
            );
            // Bottom border at the row below the block's bottom padding.
            let bot_y = new_bg_y + new_bg_h + 1;
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2594}".repeat(block_w as usize),
                    border_style,
                ))),
                Rect {
                    x: block_x,
                    y: bot_y,
                    width: block_w,
                    height: 1,
                },
            );
        }

        let visible = list_area.height.min(content_h.saturating_sub(scroll_y));
        for k in 0..visible {
            let row_idx = scroll_y as usize + k as usize;
            let sy = list_area.y + k;
            // Rows inside colored backgrounds align to the panel's left edge
            // like every other row -- the shared left padding is applied once
            // upstream (POWER_TAB_LEFT_PAD), so no extra per-row indent here.
            let is_continue_row = row_idx < continue_row_count as usize;
            let is_new_row = row_idx >= new_section_start
                && row_idx < new_section_start + new_section_count as usize;
            let (rx, rw) = if is_continue_row || is_new_row {
                (list_area.x, list_area.width)
            } else {
                (list_area.x, list_w)
            };
            let row_rect = Rect {
                x: rx,
                y: sy,
                width: rw,
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
                    let avail = (rw as usize).saturating_sub(1);
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
            super::render_power_scrollbar(f, list_area, max_off, scroll_y as usize);
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
                prefix: Some("Newest: "),
                underlay: super::PillUnderlay::Rule(palette::GREEN),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::power_home_panel_scroll;
    use crate::app::layout::AppLayout;
    use crate::app::palette;
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
        assert!(out.contains("Music"));
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
    fn list_blocks_respect_power_tab_left_indent() {
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

        let buf = term.backend().buffer();
        let continue_top_border_y = 0;
        let newest_top_border_y = 8;

        for y in [continue_top_border_y, newest_top_border_y] {
            assert_ne!(buf[(0, y)].symbol(), "\u{2581}");
            assert_ne!(buf[(1, y)].symbol(), "\u{2581}");
            assert_eq!(buf[(2, y)].symbol(), "\u{2581}");
            assert_eq!(buf[(2, y)].fg, palette::SOFT_WHITE);
            assert_eq!(buf[(21, y)].symbol(), "\u{2581}");
            assert_eq!(buf[(21, y)].fg, palette::SOFT_WHITE);
            assert_ne!(buf[(22, y)].symbol(), "\u{2581}");
            assert_ne!(buf[(23, y)].symbol(), "\u{2581}");
        }
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
