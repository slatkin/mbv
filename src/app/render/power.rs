use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, PowerFocus, palette};
use textwrap::wrap;
use super::super::ui_util::{build_queue_rows, fmt_duration, fmt_duration_approx, natural_sort_key, trunc_overview, trunc_str, QueueRow};
use unicode_width::UnicodeWidthStr;

/// For folder-based music libraries where albums are stored as directories named
/// "Artist (YYYY) Album Title", parse out the three components.
/// Returns `(artist, year, album_title)` on success.
fn parse_album_folder_name(name: &str) -> Option<(String, u32, String)> {
    let mut search_from = 0;
    while let Some(rel) = name[search_from..].find(" (") {
        let sp_pos = search_from + rel;    // position of the space before '('
        let after_open = sp_pos + 2;      // position of first char after '('
        if let Some(close_rel) = name[after_open..].find(')') {
            let year_str = &name[after_open..after_open + close_rel];
            if year_str.len() == 4 {
                if let Ok(year) = year_str.parse::<u32>() {
                    let close_pos = after_open + close_rel; // position of ')'
                    if name[close_pos..].starts_with(") ") {
                        let artist = name[..sp_pos].to_string();
                        let album  = name[close_pos + 2..].to_string();
                        return Some((artist, year, album));
                    }
                }
            }
        }
        search_from = sp_pos + 2;
    }
    None
}


/// Strips a leading article ("The ", "A ", "An ") from `s` (case-insensitive).
/// Returns a slice of the original string starting after the article.
fn strip_article(s: &str) -> &str {
    for prefix in &["the ", "a ", "an "] {
        if s.len() > prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return &s[prefix.len()..];
        }
    }
    s
}

/// Returns the effective sort key for an item: `sort_name` when Emby provides it,
/// otherwise the item's display name with any leading article stripped.
fn effective_sort_str(item: &crate::api::MediaItem) -> &str {
    if !item.sort_name.is_empty() { &item.sort_name } else { strip_article(&item.name) }
}

/// Returns the letter-group bucket label for `item` given `total` items in the list.
/// Uses `sort_name` when available (so "The Wire" → 'W'), otherwise the article-stripped
/// name. "#" for titles starting with a digit or non-letter; ranges for 50–999 items;
/// individual letters for 250+ items.
fn letter_bucket(item: &crate::api::MediaItem, total: usize) -> String {
    let key = effective_sort_str(item);
    let first = key.chars().next().map(|c| c.to_ascii_uppercase()).unwrap_or('\0');
    if !first.is_ascii_alphabetic() {
        return "#".to_string();
    }
    if total >= 250 {
        return first.to_string();
    }
    match first {
        'A'..='C' => "A\u{2013}C",
        'D'..='F' => "D\u{2013}F",
        'G'..='I' => "G\u{2013}I",
        'J'..='L' => "J\u{2013}L",
        'M'..='O' => "M\u{2013}O",
        'P'..='R' => "P\u{2013}R",
        'S'..='U' => "S\u{2013}U",
        _         => "V\u{2013}Z",
    }.to_string()
}

impl App {
    pub(super) fn render_power_view(&mut self, f: &mut Frame, area: Rect) {
        if area.height < 4 { return; }
        // Apply the tab saved from the previous session once libs have loaded.
        if self.power_left_tab_pending > 0 && !self.libs.is_empty() {
            self.power_left_tab = self.power_left_tab_pending.min(self.libs.len());
            self.power_left_tab_pending = 0;
        }
        // Safety clamp -- power_left_tab should already be valid, but guard against
        // any edge case where libs haven't populated yet.
        if self.power_left_tab > self.libs.len() {
            self.power_left_tab = 0;
        }

        // Left panel (fixed 40 cols, card + queue) | Right panel (library, remaining).
        let left_w: u16 = 40;
        let right_w = area.width.saturating_sub(left_w);

        // Full-width header: FOAM line + breadcrumb pill right-aligned.
        // Pill shows the nav path as "Library · Level" (bottom level omitted unless
        // it is also the top level). Separator dots are white; the hovered crumb
        // turns white. Clicking a crumb navigates back to that level.
        {
            // Build (display_name, target_truncation_depth) pairs.
            let crumb_depths: Vec<(String, usize)> = if self.power_left_tab == 0 {
                vec![("mbv".to_string(), 0)]
            } else {
                let lib_idx = self.power_left_tab - 1;
                let lib = &self.libs[lib_idx];
                let skip = if lib.nav_stack.first()
                    .map(|l| l.title == lib.library.name).unwrap_or(false) { 1 } else { 0 };
                let mut cd: Vec<(String, usize)> = vec![(lib.library.name.clone(), skip)];
                for (j, lvl) in lib.nav_stack.iter().enumerate().skip(skip) {
                    cd.push((lvl.title.clone(), j + 1));
                }
                // Drop the current (deepest) level from the pill unless it's the only one.
                if cd.len() > 1 { cd.pop(); }
                cd
            };

            let pill_style = Style::default().fg(palette::BASE).bg(palette::FOAM);
            let sep_style  = Style::default().fg(palette::WHITE).bg(palette::FOAM);
            const SEP: &str = " \u{00b7} "; // " · "
            const SEP_W: usize = 3;

            // Budget: leave at least a few dashes on the left.
            let budget = (area.width as usize).saturating_sub(4);
            let raw_w = 1
                + crumb_depths.iter().map(|(s, _)| s.width()).sum::<usize>()
                + crumb_depths.len().saturating_sub(1) * SEP_W
                + 1;
            // If too wide, truncate the last displayed crumb so it fits.
            let last_crumb_budget = if raw_w > budget && crumb_depths.len() > 1 {
                let fixed_w = 1
                    + crumb_depths[..crumb_depths.len() - 1].iter().map(|(s, _)| s.width()).sum::<usize>()
                    + (crumb_depths.len() - 1) * SEP_W
                    + 1;
                budget.saturating_sub(fixed_w)
            } else {
                budget
            };

            // Pre-compute display strings (with truncation applied).
            let displays: Vec<(String, usize)> = crumb_depths.iter().enumerate()
                .map(|(i, (name, depth))| {
                    let s = if i == crumb_depths.len() - 1 {
                        trunc_str(name, last_crumb_budget).to_string()
                    } else {
                        name.clone()
                    };
                    (s, *depth)
                })
                .collect();

            // Pill geometry.
            let pill_w: usize = 1
                + displays.iter().map(|(s, _)| s.width()).sum::<usize>()
                + displays.len().saturating_sub(1) * SEP_W
                + 1;
            let left_line_w = (area.width as usize).saturating_sub(pill_w);
            let crumb_row = area.y;
            // x of first crumb = pill_start + 1 leading space
            let mut x_cursor: u16 = area.x + left_line_w as u16 + 1;

            // Music-group libraries don't use breadcrumb navigation -- the group
            // selector bar inside the view replaces it. Suppress click regions for them.
            let is_music_group_lib = self.power_left_tab > 0 && {
                let li = self.power_left_tab - 1;
                self.libs[li].library.collection_type == "music"
                    && self.music_levels.first().map(|s| s == "group").unwrap_or(false)
            };

            // Build spans and register hover/click regions in one pass.
            let mut pill_spans: Vec<Span> = vec![Span::styled(" ", pill_style)];
            let mut new_power_crumbs: Vec<(u16, u16, u16, usize)> = Vec::new();
            for (i, (display, target_depth)) in displays.iter().enumerate() {
                if i > 0 {
                    pill_spans.push(Span::styled(SEP, sep_style));
                    x_cursor += SEP_W as u16;
                }
                let dw = display.width() as u16;
                let x_start = x_cursor;
                let x_end   = x_cursor + dw;
                let hovered = !is_music_group_lib
                    && self.mouse_row == crumb_row
                    && self.mouse_col >= x_start
                    && self.mouse_col < x_end;
                let crumb_fg = if hovered { palette::WHITE } else { palette::BASE };
                pill_spans.push(Span::styled(
                    display.clone(),
                    Style::default().fg(crumb_fg).bg(palette::FOAM),
                ));
                if !is_music_group_lib {
                    new_power_crumbs.push((x_start, x_end, crumb_row, *target_depth));
                }
                x_cursor = x_end;
            }
            pill_spans.push(Span::styled(" ", pill_style));
            self.layout_power_breadcrumbs = new_power_crumbs;

            let mut line_spans = vec![
                Span::styled("\u{2500}".repeat(left_line_w), Style::default().fg(palette::FOAM)),
            ];
            line_spans.extend(pill_spans);
            f.render_widget(
                Paragraph::new(Line::from(line_spans)),
                Rect { x: area.x, y: area.y, width: area.width, height: 1 },
            );
        }

        let content_h = area.height.saturating_sub(1);
        let left_area  = Rect { x: area.x,          y: area.y + 1, width: left_w,  height: content_h };
        let right_area = Rect { x: area.x + left_w + 1, y: area.y + 1, width: right_w.saturating_sub(1), height: content_h };

        let queue_focused = matches!(self.power_focus, PowerFocus::Queue);
        let left_focused  = !queue_focused;

        // The card fills the top of the left column; the queue list takes the rows
        // below it. At low heights the card can consume most of the column, so relocate
        // the queue under the library on the right instead of cramming it in.
        let (card_h, _) = self.render_power_card(f, left_area);
        let left_remaining = left_area.height.saturating_sub(card_h);

        const MIN_LIST_ROWS: u16 = 6;
        let (lib_area, queue_area) = if left_remaining < MIN_LIST_ROWS {
            // Not enough room for the queue in the left column -- split the right column:
            // library on top, relocated queue at the bottom.
            let h = right_area.height;
            let min_q = MIN_LIST_ROWS.min(h);
            let max_q = h.saturating_sub(MIN_LIST_ROWS).max(min_q);
            let queue_h = (h / 3).clamp(min_q, max_q);
            let lib_h = h.saturating_sub(queue_h);
            (Rect { height: lib_h, ..right_area },
             Rect { y: right_area.y + lib_h, height: queue_h, ..right_area })
        } else {
            // Normal mode: queue in left column below card,
            // library fills the entire right column.
            (right_area,
             Rect { y: left_area.y + card_h, height: left_remaining, ..left_area })
        };

        self.render_power_queue(f, queue_area, queue_focused);
        self.render_power_library(f, lib_area, left_focused);
    }

    fn render_power_library(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        // If a music-group library's nav_stack was truncated to just the group
        // level (e.g., stale breadcrumb click), immediately re-push the album level.
        if self.power_left_tab > 0 {
            self.ensure_music_group_album_level(self.power_left_tab - 1);
        }

        if self.power_left_tab == 0 {
            self.render_power_home_list(f, area, focused);
            return;
        }
        let lib_idx = self.power_left_tab.saturating_sub(1);
        let has_detail = self.power_left_tab > 0
            && self.libs[lib_idx].power_detail_item.is_some();
        if has_detail {
            self.render_power_detail(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_music_group_view(lib_idx) {
            self.render_power_music_group_view(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_album_level(lib_idx) {
            self.render_power_album_detail(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_series_view(lib_idx) {
            self.render_power_episode_detail(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_home_video_view(lib_idx) {
            self.render_power_home_video_list(f, area, lib_idx, focused);
        } else {
            self.render_power_list(f, area, focused);
        }
    }

    /// Returns the absolute y positions of visible group-header rows, so the caller
    /// can draw a ├ junction where each header line meets the vertical divider.
    fn render_power_queue(&mut self, f: &mut Frame, area: Rect, focused: bool) -> Vec<u16> {
        if area.height < 1 { return vec![]; }

        // Static "Queue" pill header at the top of the panel — pill on the right.
        {
            let pill = " Queue ";
            let pill_w = pill.width();
            let left = (area.width as usize).saturating_sub(pill_w);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("\u{2500}".repeat(left), Style::default().fg(palette::FOAM)),
                    Span::styled(pill, Style::default().fg(palette::BASE).bg(palette::FOAM)),
                ])),
                Rect { x: area.x, y: area.y, width: area.width, height: 1 },
            );
        }
        let area = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        // Store the content area (after header) so mouse clicks map to the right rows.
        self.power_queue_area = area;

        let n = self.player_tab.items.len();
        if n == 0 {
            self.power_queue_row_map.clear();
            f.render_widget(
                Paragraph::new("  Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                area,
            );
            return vec![];
        }

        let (active, active_idx, live_pos, live_runtime, live_paused) = self.effective_playback_state();
        let cursor = self.player_tab.playlist_cursor;
        let items = &self.player_tab.items;

        // Build display rows: audio grouped by album, episodes by series, the rest
        // flat. group_for_header[j] holds the label for the j-th Header.
        let (display, group_for_header) = build_queue_rows(items, true);
        let total = display.len();
        let visible = area.height as usize;

        // Visual row of the cursor item.
        let cursor_row = display.iter().position(|r| {
            if let QueueRow::Track { idx, .. } = r { *idx == cursor } else { false }
        }).unwrap_or(0);
        let offset = if cursor_row >= visible { cursor_row - visible + 1 } else { 0 };

        // Count how many group headers appear before the scroll offset, so we
        // index group_for_header correctly for the visible window.
        let mut header_idx = display[..offset].iter().filter(|r| matches!(r, QueueRow::Header)).count();

        let has_sb = total > visible; // column always reserved when scrollbar would appear
        let need_sb = has_sb && focused; // scrollbar only drawn when focused
        let render_w = area.width.saturating_sub(if has_sb { 1 } else { 0 }) as usize;
        let show_length = render_w > 30;
        let dur_w: usize = if show_length { 6 } else { 0 }; // "mm:ss" or "h:mm:ss"

        // Spinner character for the active item — computed once per frame, not per row.
        const SPINNER_FRAMES: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
        // Drive frame index from playback position (10M ticks/sec; 1.5M ticks = 150ms per frame).
        // live_pos is frozen when paused, so the spinner naturally freezes at the right frame.
        let spinner_frame: &str = SPINNER_FRAMES[(live_pos.max(0) / 1_500_000) as usize % SPINNER_FRAMES.len()];
        let spinner_color = if live_paused { palette::YELLOW } else { palette::IRIS };

        // Build visible ListItems and the row map simultaneously.
        self.power_queue_row_map.clear();
        let mut list_items: Vec<ListItem> = Vec::new();
        let mut header_ys: Vec<u16> = Vec::new();

        for (row_idx, entry) in display.iter().skip(offset).take(visible).enumerate() {
            match entry {
                QueueRow::Header => {
                    let group = group_for_header.get(header_idx).map(|s| s.as_str()).unwrap_or("");
                    header_idx += 1;
                    header_ys.push(area.y + row_idx as u16);
                    let label = trunc_str(group, render_w.saturating_sub(1));
                    list_items.push(ListItem::new(Line::from(
                        Span::styled(format!(" {}", label), Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                    )));
                    self.power_queue_row_map.push(None);
                }
                QueueRow::Spacer => {
                    list_items.push(ListItem::new(Line::raw("")));
                    self.power_queue_row_map.push(None);
                }
                QueueRow::Track { idx, in_group: _ } => {
                    let i = *idx;
                    let indent: usize = 0;
                    let item = &items[i];
                    let is_active = i == active_idx && active;
                    let is_cursor = i == cursor && focused;

                    let fg = if is_cursor {
                        palette::PINE
                    } else if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let row_style = Style::default().fg(fg);

                    let (pt, rt) = if is_active {
                        let pos = if live_pos > 0 { live_pos } else { item.playback_position_ticks };
                        (pos, live_runtime)
                    } else {
                        (item.playback_position_ticks, item.runtime_ticks)
                    };
                    let pct_str = if pt > 0 && rt > 0 && !item.is_audio() {
                        format!(" {}%", pt * 100 / rt.max(1))
                    } else {
                        String::new()
                    };

                    let marker = if is_cursor {
                        Span::styled("\u{258c}", Style::default().fg(palette::PINE))
                    } else {
                        Span::raw(" ")
                    };

                    // Show queue position (1-based) for all items, right-aligned
                    // so single-digit numbers line up with double-digit ones.
                    let queue_pos = idx + 1;
                    let num_w = items.len().to_string().len();
                    let label = format!("{:>num_w$}. {}", queue_pos, item.name);

                    let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                    let dur = if len_secs > 0 {
                        if item.is_audio() { fmt_duration(len_secs) } else { fmt_duration_approx(len_secs) }
                    } else { String::new() };
                    let dim_color = if focused { palette::SUBTLE } else { palette::MUTED };

                    // Spinner shown right after the title while the item is playing.
                    let spinner_char: &str = if is_active { spinner_frame } else { "" };

                    // Reserve 2 extra chars for " ⠋" when active.
                    let spinner_w: usize = if is_active { 2 } else { 0 };
                    // Title truncated to leave room for indent + marker + spinner + duration + pct.
                    let extra = dur_w + pct_str.chars().count() + spinner_w;
                    let title_w = render_w.saturating_sub(indent + 1 + extra); // 1 marker
                    let title = trunc_str(&label, title_w);

                    // Now-playing title text is always emby blue, regardless of focus state.
                    let title_color = if is_active { palette::FOAM } else { fg };

                    let mut spans: Vec<Span> = Vec::new();
                    if indent > 0 { spans.push(Span::raw(" ")); }
                    spans.push(marker);
                    // Prefix is "{n:>w}. " — render it dim, then insert spinner between
                    // prefix and name when active so it reads " 3. ⠋ Title".
                    let prefix_chars = format!("{:>num_w$}. ", queue_pos).chars().count();
                    let tc = title.chars().count();
                    if tc > prefix_chars {
                        let split = title.char_indices().nth(prefix_chars).map(|(i, _)| i).unwrap_or(title.len());
                        spans.push(Span::styled(title[..split].to_string(), Style::default().fg(dim_color)));
                        if is_active {
                            spans.push(Span::styled(spinner_char.to_string(), Style::default().fg(spinner_color)));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(title[split..].to_string(), Style::default().fg(title_color)));
                    } else {
                        if is_active {
                            spans.push(Span::styled(spinner_char.to_string(), Style::default().fg(spinner_color)));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(title, Style::default().fg(title_color)));
                    }
                    if !pct_str.is_empty() {
                        let pct_color = if is_active { palette::IRIS } else { palette::MUTED };
                        spans.push(Span::styled(pct_str, Style::default().fg(pct_color)));
                    }
                    if show_length && !dur.is_empty() {
                        let dur_color = dim_color;
                        // Right-align duration to the right edge of the queue panel.
                        let used: usize = spans.iter().map(|s| s.content.as_ref().width()).sum();
                        let pad = render_w.saturating_sub(used + dur.width());
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.push(Span::styled(dur, Style::default().fg(dur_color)));
                    }

                    list_items.push(ListItem::new(Line::from(spans)).style(row_style));
                    self.power_queue_row_map.push(Some(i));
                }
            }
        }

        let mut state = ListState::default();
        state.select(Some(cursor_row.saturating_sub(offset)));
        let render_area = Rect { width: render_w as u16, ..area };
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            render_area,
            &mut state,
        );

        if need_sb {
            let max_off = total.saturating_sub(visible);
            let mut sb = ScrollbarState::new(max_off + 1).position(offset);
            let sb_area = Rect { x: area.x + area.width.saturating_sub(1), width: 1, ..area };
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                sb_area, &mut sb,
            );
        }
        header_ys
    }

    /// Renders the right-panel list for home-video libraries (e.g. YouTube).
    /// Every row is expanded: title + meta (date added / duration / %) + one overview line.
    /// The thumbnail is handled separately by `render_power_card`.
    fn render_power_home_video_list(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        if area.height == 0 { return; }
        self.ensure_lib_loaded_for(lib_idx);

        let mut content_area = area;
        self.power_left_area = content_area;

        let (items, cursor) = {
            let lib = &self.libs[lib_idx];
            match lib.nav_stack.last() {
                Some(lvl) => (lvl.items.clone(), lvl.cursor),
                None => return,
            }
        };

        let n = items.len();

        // Item count label (matches render_power_list style).
        if focused && content_area.height > 0 {
            let count_label = format!(" {} items", n);
            f.render_widget(
                Paragraph::new(Span::styled(count_label, Style::default().fg(palette::SUBTLE))),
                Rect { height: 1, ..content_area },
            );
            content_area = Rect {
                y: content_area.y + 1,
                height: content_area.height.saturating_sub(1),
                ..content_area
            };
        }

        if n == 0 { return; }

        let is_feed_lib = {
            let c = self.client.lock().unwrap();
            c.config.feed_view_libraries.contains(&self.libs[lib_idx].library.name.to_lowercase())
        };

        const MONTHS: [&str; 12] = [
            "January","February","March","April","May","June",
            "July","August","September","October","November","December",
        ];

        // Each item: title row + meta row + separator = 3 rows; +1 if it has an overview.
        let item_heights: Vec<u16> = items.iter().map(|item| {
            if item.overview.is_empty() { 3 } else { 4 }
        }).collect();

        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > content_area.height;
        let text_w = (content_area.width as usize).saturating_sub(if needs_scrollbar { 1 } else { 0 });

        // Scroll so the cursor item is always visible.
        let scroll = {
            let mut s = 0usize;
            while s < cursor {
                let visible_h: u16 = item_heights[s..=cursor].iter().sum();
                if visible_h <= content_area.height { break; }
                s += 1;
            }
            s
        };

        let mut row_y = content_area.y;

        for (i, item) in items.iter().enumerate().skip(scroll) {
            if row_y >= content_area.y + content_area.height { break; }
            let item_h = item_heights[i];
            let selected = i == cursor;

            // Cursor marker
            let marker = if selected && focused {
                Span::styled("\u{258c}", Style::default().fg(palette::PINE))
            } else {
                Span::raw(" ")
            };
            f.render_widget(
                Paragraph::new(marker),
                Rect { x: content_area.x, y: row_y, width: 1, height: 1 },
            );

            let tx = content_area.x + 1;
            let tw = (text_w.saturating_sub(1)) as u16;

            // — Title —
            let title_color = if selected && focused { palette::IRIS } else { palette::TEXT };
            let title_style = if selected && focused {
                Style::default().fg(title_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(title_color)
            };
            let title_trunc = trunc_str(&item.display_name(), tw as usize);
            f.render_widget(
                Paragraph::new(Span::styled(title_trunc, title_style)),
                Rect { x: tx, y: row_y, width: tw, height: 1 },
            );

            // — Meta line: date added / duration / playback % —
            if row_y + 1 < content_area.y + content_area.height {
                let mut meta_spans: Vec<Span> = Vec::new();
                if item.played {
                    meta_spans.push(Span::styled("\u{2713} ", Style::default().fg(palette::PINE)));
                }
                let mut parts: Vec<String> = Vec::new();
                if is_feed_lib && !item.date_added.is_empty() {
                    let formatted = item.date_added.splitn(3, '-').collect::<Vec<_>>()
                        .as_slice().windows(3).next()
                        .and_then(|p| {
                            let y = p[0];
                            let d: u32 = p[2].parse().ok()?;
                            let m: usize = p[1].parse::<usize>().ok()?.checked_sub(1)?;
                            Some(format!("Added {} {}, {}", d, MONTHS.get(m)?, y))
                        })
                        .unwrap_or_else(|| item.date_added.clone());
                    parts.push(formatted);
                }
                let dur_s = item.runtime_ticks / crate::api::TICKS_PER_SECOND;
                if dur_s > 0 {
                    parts.push(fmt_duration_approx(dur_s));
                }
                if !parts.is_empty() {
                    meta_spans.push(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)));
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
                    Rect { x: tx, y: row_y + 1, width: tw, height: 1 },
                );
            }

            // — Overview (first wrapped line) —
            if !item.overview.is_empty() && item_h >= 4
                && row_y + 2 < content_area.y + content_area.height {
                {
                    let ov_text = trunc_overview(&item.overview);
                    let ov_first = wrap(&ov_text, (tw as usize).max(1))
                        .into_iter().next()
                        .map(|s| s.into_owned())
                        .unwrap_or_default();
                    let ov_color = if selected && focused { palette::WHITE } else { palette::MUTED };
                    f.render_widget(
                        Paragraph::new(Span::styled(ov_first, Style::default().fg(ov_color))),
                        Rect { x: tx, y: row_y + 2, width: tw, height: 1 },
                    );
                }
            }

            // — Separator —
            let sep_y = row_y + item_h - 1;
            if sep_y < content_area.y + content_area.height {
                let sep_str = "\u{2500}".repeat(text_w);
                f.render_widget(
                    Paragraph::new(Span::styled(sep_str, Style::default().fg(palette::MUTED))),
                    Rect { x: content_area.x, y: sep_y, width: text_w as u16, height: 1 },
                );
            }

            row_y += item_h;
        }

        // Scrollbar (hidden when unfocused, consistent with queue panel).
        if needs_scrollbar && focused {
            let max_off = n.saturating_sub(1);
            let mut sb = ScrollbarState::new(max_off + 1).position(scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                content_area,
                &mut sb,
            );
        }
    }

    /// Renders a cached card image centered in `area`, returning `(rows_used, image_loading)`.
    /// Uses `last_card_height` as a stable placeholder while the image is loading.
    fn render_card_image(&mut self, f: &mut Frame, area: Rect, cache_key: &str, max_h: u16) -> (u16, bool) {
        // On short terminals (<= 30 rows) cap the card image at 12 rows so the queue
        // list keeps adequate space; taller terminals cap at 18 rows.
        let max_h = max_h.min(if self.terminal_height <= 30 { 12 } else { 18 });
        let image_loading = self.card_image_loading.contains(cache_key);
        if let Some(Some(state)) = self.card_image_states.get_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
            let avail = ratatui::layout::Size {
                width: area.width,
                height: max_h.saturating_sub(1),
            };
            let actual = state.size_for(
                ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail,
            );
            let img_x = area.x + (area.width.saturating_sub(actual.width)) / 2;
            let img_rect = Rect { x: img_x, y: area.y, width: actual.width, height: actual.height };
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                img_rect, state,
            );
            self.last_card_height = actual.height + 1;
            (actual.height + 1, false)
        } else {
            // No image loaded yet — if a fetch is in-flight and we have never
            // rendered a card before, reserve the full height cap so the queue
            // panel doesn't expand then collapse when the first image arrives.
            let placeholder = if self.last_card_height == 0 && image_loading {
                max_h
            } else {
                self.last_card_height
            };
            (placeholder, image_loading)
        }
    }

    /// Renders the card image and returns `(rows_used, image_loading)`.
    /// `rows_used` is 0 if the queue is empty or the image is not yet ready.
    /// `image_loading` is true when a fetch is in-flight (caller should defer
    /// rendering the rest of the view until the image arrives).
    fn render_power_card(&mut self, f: &mut Frame, area: Rect) -> (u16, bool) {
        // If a movie detail is pinned, show that item's image instead of the queue cursor item.
        // Only show library-driven images when the library panel has focus; switch back to
        // the queue selection image when the queue panel is focused.
        let lib_focused = matches!(self.power_focus, PowerFocus::Left);
        let power_detail_pinned = lib_focused
            && self.power_left_tab > 0
            && self.libs[self.power_left_tab - 1].power_detail_item.is_some();
        if power_detail_pinned {
            // (handled below)
        } else if lib_focused && self.power_left_tab > 0 && self.is_album_level(self.power_left_tab - 1) {
            // When browsing a music album's tracks, show the album art in the card slot.
            let lib_idx = self.power_left_tab - 1;
            let (album_id, fallback_id) = {
                let lib = &self.libs[lib_idx];
                let lvl = match lib.nav_stack.last() { Some(l) => l, None => return (0, false) };
                let fid = lvl.items.first().map(|t| t.id.clone()).unwrap_or_default();
                (lvl.parent_id.clone(), fid)
            };
            let fetch_id = if !album_id.is_empty() { album_id.clone() } else { fallback_id };
            let cache_key = format!("{}:pwr_al", album_id);
            self.fetch_card_image(cache_key.clone(), fetch_id, String::new(), &["AudioChild", "Primary"]);
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        } else if lib_focused && self.power_left_tab > 0
            && self.is_series_view(self.power_left_tab - 1)
        {
            // Series view: show the selected episode's image when at episode level,
            // or the current season's poster when still loading.
            let lib_idx = self.power_left_tab - 1;
            let stack_len = self.libs[lib_idx].nav_stack.len();
            let at_episodes = self.libs[lib_idx].nav_stack.last()
                .and_then(|l| l.items.first())
                .map(|i| i.item_type == "Episode")
                .unwrap_or(false);
            let (cache_key, item_id, series_id) = if at_episodes {
                let lib = &self.libs[lib_idx];
                let lvl = lib.nav_stack.last().unwrap();
                match lvl.items.get(lvl.cursor) {
                    Some(ep) => (
                        format!("{}:pwr_ep", ep.id),
                        ep.id.clone(),
                        ep.series_id.clone(),
                    ),
                    None => return (0, false),
                }
            } else {
                // Transitional loading state (switch_season in flight): episodes
                // haven't arrived yet. Return blank placeholder rows so neither
                // the season poster nor the queue image flashes during the gap.
                let is_switch_loading = self.libs[lib_idx].nav_stack.last()
                    .map(|l| l.loading && l.items.is_empty())
                    .unwrap_or(false);
                if is_switch_loading {
                    return (self.last_card_height, false);
                }
                // At-season level (before any drill-in): use the season's own image.
                let lib = &self.libs[lib_idx];
                let season_lvl = if stack_len >= 2 {
                    &lib.nav_stack[stack_len - 2]
                } else {
                    lib.nav_stack.last().unwrap()
                };
                match season_lvl.items.get(season_lvl.cursor) {
                    Some(s) => (format!("{}:pwr_ep", s.id), s.id.clone(), String::new()),
                    None => return (0, false),
                }
            };
            self.fetch_card_image(cache_key.clone(), item_id, series_id, &["Primary", "Backdrop"]);
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        } else if lib_focused && self.power_left_tab > 0
            && self.is_home_video_view(self.power_left_tab - 1)
        {
            // Home video / feed library: show the selected item's thumbnail.
            let lib_idx = self.power_left_tab - 1;
            let (item_id, series_id) = {
                let lib = &self.libs[lib_idx];
                let lvl = match lib.nav_stack.last() { Some(l) => l, None => return (0, false) };
                match lvl.items.get(lvl.cursor) {
                    Some(item) => (item.id.clone(), item.series_id.clone()),
                    None => return (0, false),
                }
            };
            let cache_key = format!("{}:pwr_hv", item_id);
            self.fetch_card_image(cache_key.clone(), item_id, series_id, &["Primary", "Backdrop"]);
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        if power_detail_pinned {
            let (detail_id, series_id) = {
                let lib_idx = self.power_left_tab - 1;
                let d = self.libs[lib_idx].power_detail_item.as_ref().unwrap();
                (d.id.clone(), d.series_id.clone())
            };
            let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
            let cache_key = format!("{}:P", detail_id);
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), detail_id, series_id, img_types);
            }
            return self.render_card_image(f, area, &cache_key, area.height.min(18));
        }

        let cursor = self.player_tab.playlist_cursor;
        let n = self.player_tab.items.len();
        if n == 0 { return (0, false); }
        let item = &self.player_tab.items[cursor];
        let img_types: &[&str] = match item.item_type.as_str() {
            "MusicAlbum" => &["AudioChild"],
            "Audio"      => &["Primary"],
            "Movie"      => &["Backdrop", "Primary", "Logo"],
            _            => &["Primary", "Backdrop", "Logo"],
        };
        let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
        // For audio tracks, key by album_id so all tracks on the same album share
        // one cached image. Fetch still uses the track ID (proven URL), but the
        // result is stored under the album key so the second track hits the cache.
        let cache_key = if item.item_type == "Audio" && !item.album_id.is_empty() {
            format!("{}:P", item.album_id)
        } else {
            format!("{}:P", item_id)
        };
        let is_music_item = matches!(img_types, &["Primary"] | &["AudioChild"]);
        if self.images_enabled() || is_music_item {
            self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
        }

        // Prefetch images for nearby items so they are ready before the cursor reaches them.
        // Collect data first (releasing the borrow on items) then call fetch (&mut self).
        const PREFETCH_AHEAD: usize = 3;
        const PREFETCH_BEHIND: usize = 1;
        let start = cursor.saturating_sub(PREFETCH_BEHIND);
        let end = (cursor + PREFETCH_AHEAD + 1).min(n);
        let prefetch: Vec<(String, String, String, String)> = self.player_tab.items[start..end].iter()
            .enumerate()
            .filter(|(i, _)| start + i != cursor)
            .map(|(_, p)| {
                let key = if p.item_type == "Audio" && !p.album_id.is_empty() {
                    format!("{}:P", p.album_id)
                } else {
                    format!("{}:P", p.id)
                };
                (key, p.id.clone(), p.series_id.clone(), p.item_type.clone())
            })
            .collect();
        for (pkey, pid, psid, ptype) in prefetch {
            let ptypes: &[&str] = match ptype.as_str() {
                "MusicAlbum" => &["AudioChild"],
                "Audio"      => &["Primary"],
                "Movie"      => &["Backdrop", "Primary", "Logo"],
                _            => &["Primary", "Backdrop", "Logo"],
            };
            let is_music = matches!(ptypes, &["Primary"] | &["AudioChild"]);
            if self.images_enabled() || is_music {
                self.fetch_card_image(pkey, pid, psid, ptypes);
            }
        }
        self.render_card_image(f, area, &cache_key, area.height)
    }

    /// Renders the power-view home tab: "Keep Watching" group + per-library "New X" groups,
    /// all with yellow bold headers and a single flat cursor (`home.power_home_cursor`).
    fn render_power_home_list(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height == 0 { return; }

        // Build flat item list and display rows.
        // DisplayRow::Header → yellow bold section label (not selectable).
        // DisplayRow::Item(flat_idx) → item row, selected when flat_idx == power_home_cursor.
        enum HomeRow { Spacer, Header(String), Item(usize) }

        let continue_items = self.home.continue_items.clone();
        let latest = self.home.latest.clone();

        let total: usize = continue_items.len()
            + latest.iter().map(|(_, _, items, _)| items.len()).sum::<usize>();

        if total == 0 {
            f.render_widget(
                Paragraph::new(Span::styled("(nothing)", Style::default().fg(palette::MUTED))),
                area,
            );
            return;
        }

        let cursor = self.home.power_home_cursor.min(total - 1);

        let mut display_rows: Vec<HomeRow> = Vec::new();
        let mut flat_idx = 0usize;
        let mut first_group = true;

        // Continue Watching group.
        if !continue_items.is_empty() {
            display_rows.push(HomeRow::Header("Keep Watching".to_string()));
            for _ in &continue_items {
                display_rows.push(HomeRow::Item(flat_idx));
                flat_idx += 1;
            }
            first_group = false;
        }

        // Per-library latest groups — blank spacer before each header except the very first.
        for (title, _, items, _) in &latest {
            if items.is_empty() { continue; }
            if !first_group { display_rows.push(HomeRow::Spacer); }
            display_rows.push(HomeRow::Header(title.clone()));
            for _ in items {
                display_rows.push(HomeRow::Item(flat_idx));
                flat_idx += 1;
            }
            first_group = false;
        }

        // Build a combined flat items vec for rendering.
        let mut flat_items: Vec<crate::api::MediaItem> = Vec::with_capacity(total);
        flat_items.extend(continue_items.iter().cloned());
        for (_, _, items, _) in &latest {
            flat_items.extend(items.iter().cloned());
        }

        // Locate the display row for the current cursor.
        let display_cursor = display_rows.iter().position(|r| {
            matches!(r, HomeRow::Item(i) if *i == cursor)
        }).unwrap_or(0);

        let visible = area.height as usize;
        let offset = if display_cursor >= visible { display_cursor - visible + 1 } else { 0 };

        // Build row map for mouse click handling.
        self.power_left_row_map.clear();
        for row in display_rows.iter().skip(offset).take(visible) {
            self.power_left_row_map.push(match row {
                HomeRow::Spacer | HomeRow::Header(_) => None,
                HomeRow::Item(idx) => Some(*idx),
            });
        }

        let avail = (area.width as usize).saturating_sub(2);
        let list_items: Vec<ListItem> = display_rows.iter().skip(offset).take(visible).map(|row| {
            match row {
                HomeRow::Spacer => ListItem::new(Line::default()),
                HomeRow::Header(label) => {
                    ListItem::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(
                            trunc_str(label, avail).to_string(),
                            Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD),
                        ),
                    ]))
                }
                HomeRow::Item(idx) => {
                    let item = &flat_items[*idx];
                    let selected = *idx == cursor;
                    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
                        format!(" {}", fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND))
                    } else {
                        String::new()
                    };
                    let name_w = avail.saturating_sub(dur_str.width());
                    let title = trunc_str(&item.display_name(), name_w);
                    let fg = if focused { palette::WHITE } else { palette::SUBTLE };
                    let mut spans: Vec<Span> = if selected && focused {
                        vec![
                            Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                            Span::styled(title, Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                        ]
                    } else {
                        vec![
                            Span::raw(" "),
                            Span::styled(title, Style::default().fg(fg)),
                        ]
                    };
                    if !dur_str.is_empty() {
                        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                    }
                    ListItem::new(Line::from(spans))
                }
            }
        }).collect();

        let mut state = ListState::default();
        state.select(Some(display_cursor.saturating_sub(offset)));
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            area,
            &mut state,
        );

        let display_n = display_rows.len();
        if focused && display_n > visible {
            let max_off = display_n.saturating_sub(visible);
            let mut sb = ScrollbarState::new(max_off + 1).position(offset);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area,
                &mut sb,
            );
        }
    }

    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar by `render_power_view`.
    fn render_power_list(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height == 0 { return; }

        // Ensure the library is loaded when a library tab is selected.
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
        }

        let mut content_area = area;

        // Store for click / page-size calculations.
        self.power_left_area = content_area;

        // Gather items and cursor from the appropriate source.
        let (items, cursor) = if self.power_left_tab == 0 {
            let items = self.home.continue_items.clone();
            let cursor = self.home.continue_cursor.min(items.len().saturating_sub(1));
            (items, cursor)
        } else {
            let lib_idx = self.power_left_tab - 1;
            let lib = &self.libs[lib_idx];
            let (items, cur) = if let Some(s) = &lib.search {
                let items: Vec<crate::api::MediaItem> = s.results.iter()
                    .filter_map(|&i| s.items.get(i).cloned())
                    .collect();
                (items, s.cursor)
            } else {
                match lib.nav_stack.last() {
                    Some(lvl) => (lvl.items.clone(), lvl.cursor),
                    None => (vec![], 0),
                }
            };
            (items, cur)
        };

        // When at the album level of a music library, group albums under artist headers.
        let show_grouped = if self.power_left_tab > 0 {
            self.is_viewing_album_folders(self.power_left_tab - 1)
        } else {
            false
        };

        let n = items.len();

        // Letter grouping: applies to non-music library lists with 50+ items (not during search).
        let use_letter_groups = !show_grouped
            && self.power_left_tab > 0
            && n >= 50
            && {
                let lib_idx = self.power_left_tab - 1;
                self.libs[lib_idx].library.collection_type != "music"
                    && self.libs[lib_idx].search.is_none()
            };

        // First row area: search input box (when searching) or item count label.
        if focused && self.power_left_tab > 0 && content_area.height > 0 {
            let lib_idx = self.power_left_tab - 1;
            let has_search = self.libs[lib_idx].search.is_some();
            if has_search && content_area.height >= 3 {
                // 3-row bordered search input, matching the home-search visual style.
                let search_area = Rect { height: 3, ..content_area };
                content_area = Rect {
                    y: content_area.y + 3,
                    height: content_area.height.saturating_sub(3),
                    ..content_area
                };
                let s = self.libs[lib_idx].search.as_ref().unwrap();
                let input_text = if s.loading {
                    format!("{}█ [loading…]", s.query)
                } else {
                    format!("{}█", s.query)
                };
                f.render_widget(
                    Paragraph::new(Span::styled(input_text, Style::default().fg(palette::FOAM)))
                        .block(Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(palette::IRIS))
                            .title(Span::styled(" Search ", Style::default().fg(palette::YELLOW)))
                            ),
                    search_area,
                );

            } else if !has_search {
                let count_label = format!(" {} items", n);
                f.render_widget(
                    Paragraph::new(Span::styled(count_label, Style::default().fg(palette::SUBTLE))),
                    Rect { height: 1, ..content_area },
                );
                content_area = Rect {
                    y: content_area.y + 1,
                    height: content_area.height.saturating_sub(1),
                    ..content_area
                };
            }
        }

        if n == 0 {
            let msg = if self.power_left_tab > 0 {
                let lib_idx = self.power_left_tab - 1;
                if self.libs[lib_idx].nav_stack.last().map(|l| l.loading).unwrap_or(false) {
                    "Loading..."
                } else {
                    "(empty)"
                }
            } else {
                "(empty)"
            };
            f.render_widget(
                Paragraph::new(Span::styled(msg, Style::default().fg(palette::MUTED))),
                content_area,
            );
            return;
        }

        let visible = content_area.height as usize;

        if show_grouped {
            self.power_left_sorted_indices.clear();
            // Build a display row list that interleaves artist headers with album rows.
            enum DisplayRow { ArtistHeader(String), Album(usize) }

            // Precompute (artist, row_prefix, album_name) for each item.
            // When proper Emby metadata is available (item.artist non-empty), use it directly.
            // Otherwise fall back to parsing "Artist (YYYY) Album" from the folder name.
            // (artist, year_str, album_name) — year_str is empty if no year.
            let album_info: Vec<(String, String, String)> = items.iter().map(|item| {
                if !item.artist.is_empty() {
                    let year_str = if item.production_year > 0 {
                        item.production_year.to_string()
                    } else {
                        String::new()
                    };
                    (item.artist.clone(), year_str, item.display_name())
                } else if let Some((artist, year, album)) = parse_album_folder_name(&item.name) {
                    let year_str = if year > 0 { year.to_string() } else { String::new() };
                    (artist, year_str, album)
                } else {
                    ("Unknown Artist".to_string(), String::new(), item.display_name())
                }
            }).collect();

            let mut display_rows: Vec<DisplayRow> = Vec::new();
            let mut last_artist = String::new();
            for (idx, (artist, _, _)) in album_info.iter().enumerate() {
                if artist != &last_artist {
                    display_rows.push(DisplayRow::ArtistHeader(artist.clone()));
                    last_artist = artist.clone();
                }
                display_rows.push(DisplayRow::Album(idx));
            }

            // Locate the display row for the current cursor item and derive scroll offset.
            let display_cursor = display_rows.iter().position(|r| {
                matches!(r, DisplayRow::Album(i) if *i == cursor)
            }).unwrap_or(0);
            let offset = if display_cursor >= visible { display_cursor - visible + 1 } else { 0 };

            let avail = (area.width as usize).saturating_sub(2);

            let list_items: Vec<ListItem> = display_rows.iter().skip(offset).take(visible).map(|row| {
                match row {
                    DisplayRow::ArtistHeader(name) => {
                        let artist_label = trunc_str(name, avail);
                        ListItem::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(artist_label, Style::default().fg(palette::YELLOW)),
                        ]))
                    }
                    DisplayRow::Album(idx) => {
                        let selected = *idx == cursor;
                        let (_, year_str, album_name) = &album_info[*idx];
                        // prefix width: "   (YYYY) " = year.len()+6, or "   " = 3 if no year.
                        let prefix_w = if year_str.is_empty() { 3 } else { year_str.len() + 6 };
                        let name_w = avail.saturating_sub(prefix_w);
                        let trunc_name = trunc_str(album_name, name_w);
                        let fg = if focused { palette::WHITE } else { palette::SUBTLE };
                        let name_color = if selected && focused { palette::IRIS } else { fg };
                        let mut spans: Vec<Span> = Vec::new();
                        if selected && focused {
                            spans.push(Span::styled("\u{258c}", Style::default().fg(palette::PINE)));
                        } else {
                            spans.push(Span::raw(" "));
                        }
                        if year_str.is_empty() {
                            spans.push(Span::raw("   "));
                        } else {
                            spans.push(Span::styled("   (", Style::default().fg(palette::SUBTLE)));
                            spans.push(Span::styled(year_str.clone(), Style::default().fg(palette::PINE)));
                            spans.push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
                        }
                        let name_style = if selected && focused {
                            Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(name_color)
                        };
                        spans.push(Span::styled(trunc_name.to_string(), name_style));
                        ListItem::new(Line::from(spans))
                    }
                }
            }).collect();

            let mut state = ListState::default();
            state.select(Some(display_cursor.saturating_sub(offset)));
            f.render_stateful_widget(List::new(list_items).highlight_style(Style::default()), content_area, &mut state);

            self.power_left_row_map.clear();
            let display_n = display_rows.len();
            if focused && display_n > visible {
                let max_off = display_n.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None).end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area, &mut sb,
                );
            }
        } else if use_letter_groups {
            // Build display rows: inject a Spacer+LetterHeader at each bucket boundary.
            // The spacer is omitted before the very first header.
            enum DisplayRow { Spacer, LetterHeader(String), Item(usize) }

            // Sort item indices by the same effective key used for bucketing so that
            // items within each group appear in article-stripped alphabetical order.
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by_key(|&i| natural_sort_key(effective_sort_str(&items[i])));
            // Publish the sorted order so cursor navigation can follow display order.
            self.power_left_sorted_indices = sorted_indices.clone();

            let mut display_rows: Vec<DisplayRow> = Vec::new();
            let mut last_bucket = String::new();
            for &idx in &sorted_indices {
                let item = &items[idx];
                let bucket = letter_bucket(item, n);
                if bucket != last_bucket {
                    if !last_bucket.is_empty() {
                        display_rows.push(DisplayRow::Spacer);
                    }
                    display_rows.push(DisplayRow::LetterHeader(bucket.clone()));
                    last_bucket = bucket;
                }
                display_rows.push(DisplayRow::Item(idx));
            }
            let total_display = display_rows.len();

            // Find the visual row of the current cursor item for scrolling.
            let display_cursor = display_rows.iter().position(|r| {
                matches!(r, DisplayRow::Item(i) if *i == cursor)
            }).unwrap_or(0);
            let offset = if display_cursor >= visible { display_cursor - visible + 1 } else { 0 };

            // Build row map so mouse clicks can map visual row → item index.
            self.power_left_row_map.clear();
            for row in display_rows.iter().skip(offset).take(visible) {
                self.power_left_row_map.push(match row {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) => None,
                    DisplayRow::Item(idx) => Some(*idx),
                });
            }

            let avail = (area.width as usize).saturating_sub(2);
            let list_items: Vec<ListItem> = display_rows.iter().skip(offset).take(visible).map(|row| {
                match row {
                    DisplayRow::Spacer => ListItem::new(Line::default()),
                    DisplayRow::LetterHeader(label) => {
                        ListItem::new(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(label.clone(),
                                Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                        ]))
                    }
                    DisplayRow::Item(idx) => {
                        let item = &items[*idx];
                        let selected = *idx == cursor;
                        let (item_name, dur_str) = if item.is_folder {
                            let name = if item.item_type == "Folder" && item.total_count > 0 {
                                format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                            } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
                                format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                            } else {
                                item.display_name()
                            };
                            (name, String::new())
                        } else {
                            let dur = if item.runtime_ticks > 0 {
                                format!(" {}", fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND))
                            } else {
                                String::new()
                            };
                            (item.display_name(), dur)
                        };
                        let name_w = avail.saturating_sub(dur_str.width());
                        let title = trunc_str(&item_name, name_w);
                        let fg = if focused { palette::WHITE } else { palette::SUBTLE };
                        let mut spans: Vec<Span> = if selected && focused {
                            vec![
                                Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                                Span::styled(title, Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                            ]
                        } else {
                            vec![
                                Span::raw(" "),
                                Span::styled(title, Style::default().fg(fg)),
                            ]
                        };
                        if !dur_str.is_empty() {
                            spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                        }
                        ListItem::new(Line::from(spans))
                    }
                }
            }).collect();

            let mut state = ListState::default();
            state.select(Some(display_cursor.saturating_sub(offset)));
            f.render_stateful_widget(List::new(list_items).highlight_style(Style::default()), content_area, &mut state);

            if focused && total_display > visible {
                let max_off = total_display.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None).end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area, &mut sb,
                );
            }
        } else {
            self.power_left_row_map.clear();
            self.power_left_sorted_indices.clear();
            let offset = if cursor >= visible { cursor - visible + 1 } else { 0 };

            let list_items: Vec<ListItem> = items.iter().skip(offset).take(visible).enumerate().map(|(i, item)| {
                let abs = offset + i;
                let selected = abs == cursor;

                // Compute name and duration as separate strings so they can be styled
                // independently: name in the normal fg, duration in OVERLAY (no parens).
                let (item_name, dur_str) = if item.is_folder {
                    let name = if item.item_type == "Folder" && item.total_count > 0 {
                        format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                    } else if item.unplayed_item_count > 0 && item.item_type != "Series" {
                        format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                    } else {
                        item.display_name()
                    };
                    (name, String::new())
                } else {
                    let dur = if item.runtime_ticks > 0 {
                        format!(" {}", fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND))
                    } else {
                        String::new()
                    };
                    (item.display_name(), dur)
                };

                let avail = (area.width as usize).saturating_sub(2);
                let name_w = avail.saturating_sub(dur_str.width());
                let title = trunc_str(&item_name, name_w);
                let fg = if focused { palette::WHITE } else { palette::SUBTLE };

                let mut spans: Vec<Span> = if selected && focused {
                    vec![
                        Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                        Span::styled(title, Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                    ]
                } else {
                    vec![
                        Span::raw(" "),
                        Span::styled(title, Style::default().fg(fg)),
                    ]
                };
                if !dur_str.is_empty() {
                    spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                }
                ListItem::new(Line::from(spans))
            }).collect();

            let mut state = ListState::default();
            state.select(Some(cursor.saturating_sub(offset)));
            f.render_stateful_widget(List::new(list_items).highlight_style(Style::default()), content_area, &mut state);

            if focused && n > visible {
                let max_off = n.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None).end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area, &mut sb,
                );
            }
        }
    }

    /// Renders the movie detail panel (title, metadata, overview, director) into `area`.
    /// Called instead of `render_power_list` when `power_detail_item` is Some.
    fn render_power_detail(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        // Clone so self is free for scroll-state writes below.
        let item = match self.libs[lib_idx].power_detail_item.clone() { Some(it) => it, None => return };
        if area.height == 0 { return; }

        let inner_x = area.x + 1;
        let inner_w = (area.width as usize).saturating_sub(2);
        let inner_w16 = area.width.saturating_sub(2);
        let max_y = area.y + area.height;
        let mut row = area.y;

        let title_color = if focused { palette::YELLOW } else { palette::SUBTLE };
        let text_color  = if focused { palette::WHITE  } else { palette::SUBTLE };

        // — Primary poster image (right-aligned in a bordered block, starts on second row) —
        const IMG_COLS: u16 = 28;
        const IMG_MAX_ROWS: u16 = 12;
        let img_start_row = area.y + 1; // row immediately after title

        // Fetch the Primary image using a key distinct from the backdrop key.
        let primary_cache_key = format!("{}:det_primary", item.id);
        if self.images_enabled() {
            self.fetch_card_image(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        // Pre-compute the *actual* rendered dimensions. size_for() respects aspect ratio so
        // the image may be narrower than IMG_COLS (e.g. a portrait poster). We need the real
        // width to position it flush-right and to compute the text shadow width.
        // The borrow on card_image_states ends at the closing } of this block.
        let (img_actual_w, img_height): (u16, u16) = {
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                let avail = ratatui::layout::Size { width: IMG_COLS, height: IMG_MAX_ROWS };
                let actual = state.size_for(
                    ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)),
                    avail,
                );
                (actual.width, actual.height)
            } else {
                (0, 0)
            }
        };

        // Image is flush with the right edge; shadow extends 1 extra row below (bottom padding).
        let img_x = area.x + area.width.saturating_sub(img_actual_w);
        // img_end_row is exclusive: image rows + 1 blank padding row below.
        let img_end_row = img_start_row + img_height + 1;

        // Narrow text width: leave 1-col gap to the left of the image.
        // img_x = area.x + area.width - img_actual_w; text spans [inner_x, inner_x + narrow_w16).
        // Last text col = inner_x + narrow_w16 - 1; gap col = img_x - 1; so narrow_w16 = img_x - inner_x - 1.
        // = (area.width - img_actual_w) - 1 - 1 = inner_w16 - img_actual_w - 1 + 1 ... simplify:
        // narrow_w16 = area.width - img_actual_w - 2 = inner_w16 - img_actual_w
        let narrow_w   = inner_w.saturating_sub(img_actual_w as usize);
        let narrow_w16 = inner_w16.saturating_sub(img_actual_w);

        // Return the appropriate (char_width, u16_width) for a given absolute row.
        let text_dims = |r: u16| -> (usize, u16) {
            if img_height > 0 && r >= img_start_row && r < img_end_row {
                (narrow_w, narrow_w16)
            } else {
                (inner_w, inner_w16)
            }
        };

        // — Title (row 0 — full width, image hasn't started yet) —
        if row < max_y {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&item.name, inner_w),
                    Style::default().fg(title_color),
                ))),
                Rect { x: inner_x, y: row, width: inner_w16, height: 1 },
            );
            row += 1;
        }

        // — Meta: genre  year  duration (SUBTLE) —
        if row < max_y {
            let dur_str = if item.runtime_ticks > 0 {
                fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
            } else {
                String::new()
            };
            let year_str = if item.production_year > 0 { item.production_year.to_string() } else { String::new() };
            let meta = [item.genre.as_str(), year_str.as_str(), dur_str.as_str()]
                .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  ");
            let (tw, tw16) = text_dims(row);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&meta, tw),
                    Style::default().fg(palette::SUBTLE),
                ))),
                Rect { x: inner_x, y: row, width: tw16, height: 1 },
            );
            row += 1;
        }

        // — Technical: video_info then audio_info on separate rows (MUTED) —
        for tech_str in [item.video_info.as_str(), item.audio_info.as_str()] {
            if row < max_y && !tech_str.is_empty() {
                let (tw, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(tech_str, tw),
                        Style::default().fg(palette::MUTED),
                    ))),
                    Rect { x: inner_x, y: row, width: tw16, height: 1 },
                );
                row += 1;
            }
        }

        // — Play status block: blank / status / blank —
        {
            let (active, active_idx, _, _, _) = self.effective_playback_state();
            let now_playing_id: Option<String> = if active {
                self.player_tab.items.get(active_idx).map(|i| i.id.clone())
            } else {
                None
            };
            let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());

            // blank row above
            if row < max_y { row += 1; }

            // status row
            if row < max_y {
                let (_tw, tw16) = text_dims(row);
                if is_playing {
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            "Playing",
                            Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD),
                        ))),
                        Rect { x: inner_x, y: row, width: tw16, height: 1 },
                    );
                } else {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::styled("Press ", Style::default().fg(palette::SUBTLE)),
                            Span::styled("[ENTER]", Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)),
                            Span::styled(" to play", Style::default().fg(palette::SUBTLE)),
                        ])),
                        Rect {
                            x: inner_x,
                            y: row,
                            width: tw16,
                            height: 1,
                        },
                    );
                }
                row += 1;
            }

            // blank row below
            if row < max_y { row += 1; }
        }

        // — Overview + Director (single scrollable block) —
        // Director flows naturally after the description with a blank separator;
        // nothing is pinned to the bottom.
        if (!item.overview.is_empty() || !item.director.is_empty()) && row < max_y {
            let avail = max_y.saturating_sub(row) as usize;
            let ov_start_y = row;

            // How many display rows at the top of this block still overlap with the image?
            let shadow_lines = img_end_row.saturating_sub(ov_start_y) as usize;

            // When the user has scrolled, lines with abs_line_idx >= shadow_lines may appear
            // in the on-screen rows that still overlap the image (disp_idx < shadow_lines).
            // Wrap using scroll + shadow_lines as the narrow boundary so that every line
            // that will appear next to the image on screen is wrapped at narrow width.
            let cur_scroll = self.libs[lib_idx].power_detail_scroll;
            let shadow_boundary = cur_scroll + shadow_lines;

            // Word-wrap the overview, switching from narrow to full width at the shadow boundary.
            let mut all_lines: Vec<String> = Vec::new();
            let mut cur = String::new();
            for word in item.overview.split_whitespace() {
                let line_idx = all_lines.len();
                let wrap_w = if line_idx < shadow_boundary { narrow_w } else { inner_w };
                let word_w = word.width();
                if cur.is_empty() {
                    cur.push_str(word);
                } else if cur.width() + 1 + word_w <= wrap_w {
                    cur.push(' ');
                    cur.push_str(word);
                } else {
                    all_lines.push(std::mem::take(&mut cur));
                    cur.push_str(word);
                }
            }
            if !cur.is_empty() { all_lines.push(cur); }

            // Director flows after the overview: blank gap then the director line.
            let director_line_idx: Option<usize> = if !item.director.is_empty() {
                all_lines.push(String::new()); // blank separator
                let idx = all_lines.len();
                all_lines.push(format!("Director: {}", item.director));
                Some(idx)
            } else {
                None
            };

            let total = all_lines.len();
            let max_scroll = total.saturating_sub(avail);
            let scroll = self.libs[lib_idx].power_detail_scroll.min(max_scroll);
            self.libs[lib_idx].power_detail_scroll = scroll;
            self.power_detail_max_scroll = max_scroll;
            self.power_detail_page_h = avail.max(1);

            if avail > 0 {
                for (disp_idx, line_text) in all_lines.iter().skip(scroll).take(avail).enumerate() {
                    if row >= max_y { break; }
                    // Use disp_idx (on-screen position) to pick width: the first shadow_lines
                    // rows are next to the image regardless of scroll position.
                    let tw16 = if disp_idx < shadow_lines { narrow_w16 } else { inner_w16 };
                    let abs_line_idx = scroll + disp_idx;
                    let fg = if Some(abs_line_idx) == director_line_idx {
                        palette::MUTED
                    } else {
                        text_color
                    };
                    if !line_text.is_empty() {
                        f.render_widget(
                            Paragraph::new(Line::from(Span::styled(
                                line_text.clone(), Style::default().fg(fg),
                            ))),
                            Rect { x: inner_x, y: row, width: tw16, height: 1 },
                        );
                    }
                    row += 1;
                }

                if max_scroll > 0 {
                    let ov_area = Rect {
                        x: area.x, y: ov_start_y,
                        width: area.width, height: avail as u16,
                    };
                    let mut sb = ScrollbarState::new(max_scroll + 1).position(scroll);
                    f.render_stateful_widget(
                        Scrollbar::new(ScrollbarOrientation::VerticalRight)
                            .thumb_symbol("\u{2590}")
                            .track_symbol(Some(" "))
                            .begin_symbol(None).end_symbol(None)
                            .style(Style::default().fg(palette::SUBTLE)),
                        ov_area, &mut sb,
                    );
                }
            }
        }

        // — Render Primary image last so it layers over text cleanly —
        // No border drawn; the 1-col left gap and 1-row bottom gap are handled via shadow math.
        if img_height > 0 {
            if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                let img_rect = Rect {
                    x: img_x,
                    y: img_start_row,
                    width: img_actual_w,
                    height: img_height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(
                        Some(ratatui_image::FilterType::Lanczos3),
                    )),
                    img_rect,
                    state,
                );
            }
        }
    }

    /// Renders the music album detail panel (track list) into `area` — the lib
    /// slot below the card. The card itself already shows the album art (handled
    /// in `render_power_card`). Mirrors `render_power_detail` for movies.
    fn render_power_album_detail(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        if area.height == 0 { return; }

        let (items, cursor) = {
            let lib = &self.libs[lib_idx];
            let lvl = match lib.nav_stack.last() { Some(l) => l, None => return };
            (lvl.items.clone(), lvl.cursor)
        };
        let n = items.len();
        if items.is_empty() { return; }
        let first = &items[0];
        let album_title  = first.album.clone();
        let album_artist = first.artist.clone();
        let album_year   = first.production_year;

        let inner_w = area.width as usize;
        let max_y   = area.y + area.height;
        let mut row   = area.y;

        // — Album title: yellow, left-aligned, no background —
        if row < max_y {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {}", trunc_str(&album_title, inner_w.saturating_sub(1))),
                    Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD),
                ))),
                Rect { x: area.x, y: row, width: area.width, height: 1 },
            );
            row += 1;
        }

        // — Album artist: same colour as inactive tabs (SUBTLE) —
        if row < max_y && !album_artist.is_empty() {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {}", trunc_str(&album_artist, inner_w.saturating_sub(1))),
                    Style::default().fg(palette::SUBTLE),
                ))),
                Rect { x: area.x, y: row, width: area.width, height: 1 },
            );
            row += 1;
        }

        // — Release year: same colour as the VOL label (MUTED) —
        if row < max_y && album_year > 0 {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    format!(" {}", album_year),
                    Style::default().fg(palette::MUTED),
                ))),
                Rect { x: area.x, y: row, width: area.width, height: 1 },
            );
            row += 1;
        }

        // — Blank spacer row —
        if row < max_y { row += 1; }

        // — Scrollable track list —
        let table_area = Rect { x: area.x, y: row, width: area.width, height: max_y.saturating_sub(row) };
        if table_area.height == 0 { return; }

        let (active, active_idx, _, _, _) = self.effective_playback_state();
        let now_playing_id: Option<String> = if active {
            self.player_tab.items.get(active_idx).map(|i| i.id.clone())
        } else {
            None
        };

        let show_length = table_area.width > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (table_area.width as usize)
            .saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items.iter().enumerate().map(|(i, item)| {
            let is_cursor  = i == cursor;
            let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());
            let row_style = if is_playing {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
            } else if is_cursor && focused {
                Style::default().fg(palette::YELLOW)
            } else if focused {
                Style::default().fg(palette::WHITE)
            } else {
                Style::default().fg(palette::SUBTLE)
            };
            let marker = if is_cursor && focused {
                Span::styled("\u{258c}", Style::default().fg(palette::PINE))
            } else {
                Span::raw(" ")
            };
            let track_num = if item.index_number > 0 {
                format!("{}. ", item.index_number)
            } else {
                format!("{}. ", i + 1)
            };
            let num_w = track_num.chars().count();
            let title = trunc_str(&item.name, title_col_w.saturating_sub(num_w));
            let title_cell = Cell::from(Line::from(vec![
                marker,
                Span::styled(track_num, Style::default().fg(palette::SUBTLE)),
                Span::raw(title),
            ]));
            let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration_approx(len_secs) } else { "\u{2014}".to_string() };
            if show_length {
                Row::new([
                    title_cell,
                    Cell::from(Line::from(length).alignment(Alignment::Right))
                        .style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(""),
                ]).style(row_style)
            } else {
                Row::new([title_cell, Cell::from(""), Cell::from("")]).style(row_style)
            }
        }).collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Min(10),
            Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
            Constraint::Length(1),
        ])
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);

        let visible_rows = table_area.height as usize;
        if n > visible_rows {
            let max_offset = n.saturating_sub(visible_rows);
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(state.offset());
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                table_area, &mut sb_state,
            );
        }
    }

    /// Renders the combined TV series view: episode metadata at the top, season
    /// tabs + green divider above the episode list at the bottom.
    fn render_power_episode_detail(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        if area.height == 0 { return; }

        // ── Collect nav state ───────────────────────────────────────────────
        let stack_len = self.libs[lib_idx].nav_stack.len();
        let (seasons, season_cursor, items, ep_cursor) = {
            let lib = &self.libs[lib_idx];
            let last = match lib.nav_stack.last() { Some(l) => l, None => return };
            let at_episodes = last.items.first()
                .map(|i| i.item_type == "Episode").unwrap_or(false);
            if at_episodes && stack_len >= 2 {
                let season_lvl = &lib.nav_stack[stack_len - 2];
                let have_seasons = season_lvl.items.first()
                    .map(|i| i.item_type == "Season").unwrap_or(false);
                if have_seasons {
                    (season_lvl.items.clone(), season_lvl.cursor,
                     last.items.clone(), last.cursor)
                } else {
                    (vec![], 0, last.items.clone(), last.cursor)
                }
            } else if last.items.first().map(|i| i.item_type == "Season").unwrap_or(false) {
                // Loading state: at season level, episodes still arriving.
                (last.items.clone(), last.cursor, vec![], 0)
            } else {
                (vec![], 0, last.items.clone(), last.cursor)
            }
        };

        let inner_x   = area.x + 1;
        let inner_w   = (area.width as usize).saturating_sub(2);
        let inner_w16 = area.width.saturating_sub(2);
        let max_y = area.y + area.height;
        let mut row = area.y;

        // ── Selected episode metadata (top of panel) ─────────────────────────
        let text_color  = if focused { palette::WHITE } else { palette::SUBTLE };

        let mut metadata_img_end_row: u16 = area.y; // updated inside the block
        if let Some(item) = items.get(ep_cursor).cloned() {
            // ── Series Primary image (right-aligned, text wraps around it) ───
            const IMG_COLS: u16 = 24;
            const IMG_MAX_ROWS: u16 = 14;
            let img_start_row = area.y + 1; // row after title
            let primary_cache_key = format!("{}:ser_primary", item.series_id);
            if !item.series_id.is_empty() && self.images_enabled() {
                self.fetch_card_image(
                    primary_cache_key.clone(),
                    item.series_id.clone(),
                    String::new(),
                    &["Primary"],
                );
            }
            const IMG_PLACEHOLDER_H: u16 = 12;
            let img_loading = !item.series_id.is_empty()
                && self.images_enabled()
                && self.card_image_loading.contains(&primary_cache_key);
            let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) = {
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    let avail = ratatui::layout::Size { width: IMG_COLS, height: IMG_MAX_ROWS };
                    let actual = state.size_for(
                        ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)),
                        avail,
                    );
                    (actual.width, actual.height, false)
                } else if img_loading {
                    // Reserve space while the image fetch is in flight.
                    (IMG_COLS, IMG_PLACEHOLDER_H, true)
                } else {
                    (0, 0, false)
                }
            };
            let img_x = area.x + area.width.saturating_sub(img_actual_w);
            let img_end_row = img_start_row + img_height + 1;
            metadata_img_end_row = img_end_row;
            let narrow_w   = inner_w.saturating_sub(img_actual_w as usize);
            let narrow_w16 = inner_w16.saturating_sub(img_actual_w);
            let text_dims = |r: u16| -> (usize, u16) {
                if img_height > 0 && r >= img_start_row && r < img_end_row {
                    (narrow_w, narrow_w16)
                } else {
                    (inner_w, inner_w16)
                }
            };

            // ── Series title (YELLOW) ────────────────────────────────────────
            if !item.series_name.is_empty() && row < max_y {
                let (tw, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(&item.series_name, tw),
                        Style::default().fg(palette::YELLOW),
                    ))),
                    Rect { x: inner_x, y: row, width: tw16, height: 1 },
                );
                row += 1;
            }

            // ── Series metadata (year range + genre) ─────────────────────────
            if row < max_y {
                // Try to get the series item from the nav level two above episode level.
                let series_item = if stack_len >= 3 {
                    let series_lvl = &self.libs[lib_idx].nav_stack[stack_len - 3];
                    series_lvl.items.get(series_lvl.cursor).cloned()
                } else { None };
                let (ser_start, ser_end, ser_genre) = if let Some(ref si) = series_item {
                    (si.production_year, si.end_year, si.genre.clone())
                } else {
                    (0u32, 0u32, item.genre.clone())
                };
                let genre_upper = ser_genre.to_uppercase();
                let year_range = match (ser_start, ser_end) {
                    (s, e) if s > 0 && e > 0 && e != s => format!("{}-{}", s, e),
                    (s, _) if s > 0 => format!("{}", s),
                    _ => String::new(),
                };
                let ser_meta = [year_range.as_str(), genre_upper.as_str()]
                    .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  ");
                if !ser_meta.is_empty() {
                    let (tw, tw16) = text_dims(row);
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            trunc_str(&ser_meta, tw),
                            Style::default().fg(palette::SUBTLE),
                        ))),
                        Rect { x: inner_x, y: row, width: tw16, height: 1 },
                    );
                    row += 1;
                }
            }

            // Blank spacer after series block
            if row < max_y { row += 1; }

            // ── Episode title (PINE/green) ────────────────────────────────────
            if row < max_y {
                let ep_title_color = if focused { palette::PINE } else { palette::SUBTLE };
                let (tw, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(&item.name, tw),
                        Style::default().fg(ep_title_color),
                    ))),
                    Rect { x: inner_x, y: row, width: tw16, height: 1 },
                );
                row += 1;
            }

            // ── Episode metadata (year + duration only) ───────────────────────
            if row < max_y {
                let dur_str = if item.runtime_ticks > 0 {
                    fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                } else { String::new() };
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else { String::new() };
                let ep_meta = [year_str.as_str(), dur_str.as_str()]
                    .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  ");
                if !ep_meta.is_empty() {
                    let (tw, tw16) = text_dims(row);
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            trunc_str(&ep_meta, tw),
                            Style::default().fg(palette::SUBTLE),
                        ))),
                        Rect { x: inner_x, y: row, width: tw16, height: 1 },
                    );
                    row += 1;
                }
            }

            // Blank spacer
            if row < max_y { row += 1; }

            // Overview (word-wrapped, respects image shadow width).
            // Fill the space alongside/below the series image, reserving room for
            // the play-status line, divider, and a usable episode list below.
            let used = row.saturating_sub(area.y);
            const OV_RESERVED: u16 = 8; // play status + spacer + divider + a few episode rows
            let max_ov_rows = (area.height
                .saturating_sub(used)
                .saturating_sub(OV_RESERVED) as usize)
                .clamp(1, 12);
            if !item.overview.is_empty() && row < max_y {
                let mut ov_lines: Vec<String> = Vec::new();
                let mut cur_line = String::new();
                // Determine wrap width for first line (may be in shadow next to image).
                // Each line is wrapped independently at the width for that row.
                let mut wrap_row = row;
                for word in item.overview.split_whitespace() {
                    let word_w = word.width();
                    let (wrap_w, _) = text_dims(wrap_row);
                    if cur_line.is_empty() {
                        cur_line.push_str(word);
                    } else if cur_line.width() + 1 + word_w <= wrap_w {
                        cur_line.push(' ');
                        cur_line.push_str(word);
                    } else {
                        ov_lines.push(std::mem::take(&mut cur_line));
                        wrap_row += 1;
                        cur_line.push_str(word);
                    }
                }
                if !cur_line.is_empty() { ov_lines.push(cur_line); }
                for line_text in ov_lines.iter().take(max_ov_rows) {
                    if row >= max_y { break; }
                    let (_, tw16) = text_dims(row);
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            line_text.clone(), Style::default().fg(text_color),
                        ))),
                        Rect { x: inner_x, y: row, width: tw16, height: 1 },
                    );
                    row += 1;
                }
                if row < max_y { row += 1; }
            }

            // ── Render series image last so it layers over text ───────────────
            if img_height > 0 {
                let img_rect = Rect {
                    x: img_x,
                    y: img_start_row,
                    width: img_actual_w,
                    height: img_height,
                };
                if img_is_placeholder {
                    // Image still loading — draw a dim placeholder block to hold the space.
                    f.render_widget(
                        Block::default()
                            .style(Style::default().bg(palette::OVERLAY)),
                        img_rect,
                    );
                } else if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Scale(
                            Some(ratatui_image::FilterType::Lanczos3),
                        )),
                        img_rect, state,
                    );
                }
            }
        }

        // Push below the image if it extends past the text.
        if row < metadata_img_end_row { row = metadata_img_end_row; }

        // ── Grey divider with season tabs overlaid ───────────────────────────
        if row < max_y { row += 1; } // blank spacer above divider
        if row < max_y {
            // Draw the full-width blue rule first.
            let line_str = "\u{2500}".repeat(area.width as usize);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    line_str,
                    Style::default().fg(palette::FOAM),
                ))),
                Rect { x: area.x, y: row, width: area.width, height: 1 },
            );

            // Overlay season tabs on the same row (drawn on top of the rule).
            if !seasons.is_empty() {
                // Labels are bare numbers: "01", "02", …
                let tab_labels: Vec<String> = seasons.iter().enumerate().map(|(i, s)| {
                    let n = if s.index_number > 0 { s.index_number as usize } else { i + 1 };
                    format!("{:02}", n)
                }).collect();
                // pill width = 1 space + 2 digits + 1 space = 4; gap between pills = 1
                let per_tab = 5usize;
                let n_tabs  = tab_labels.len();

                // "Series: " prefix (8 chars) + optional "‹ " (2 chars)
                let prefix_w = 8 + if season_cursor > 0 { 2 } else { 0 };
                let avail = (area.width as usize).saturating_sub(prefix_w + 2);
                let tabs_per_page = ((avail + 1) / per_tab).max(1);
                let scroll_start = if season_cursor < tabs_per_page {
                    0
                } else {
                    season_cursor.saturating_sub(tabs_per_page - 1)
                };
                let scroll_end = (scroll_start + tabs_per_page).min(n_tabs);

                let mut spans: Vec<Span> = Vec::new();
                // "Series: " label — white, no background (line shows through)
                spans.push(Span::styled("Series: ", Style::default().fg(ratatui::style::Color::White)));
                if scroll_start > 0 {
                    spans.push(Span::styled("\u{2039} ", Style::default().fg(palette::FOAM)));
                }
                for (idx, label) in tab_labels[scroll_start..scroll_end].iter().enumerate() {
                    if idx > 0 {
                        // Single transparent space — blue line shows through
                        spans.push(Span::raw(" "));
                    }
                    let abs_idx = scroll_start + idx;
                    let (fg, bold) = if abs_idx == season_cursor {
                        (palette::YELLOW, true)
                    } else {
                        (palette::BASE, false)
                    };
                    let style = if bold {
                        Style::default().fg(fg).bg(palette::FOAM).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(fg).bg(palette::FOAM)
                    };
                    spans.push(Span::styled(format!(" {} ", label), style));
                }
                if scroll_end < n_tabs {
                    spans.push(Span::styled(" \u{203a}", Style::default().fg(palette::FOAM)));
                }
                f.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect { x: area.x, y: row, width: area.width, height: 1 },
                );
            }
            row += 1;
        }

        // ── Loading state: episodes not yet arrived ─────────────────────────
        if items.is_empty() {
            if row < max_y {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        " Loading\u{2026}",
                        Style::default().fg(palette::MUTED),
                    ))),
                    Rect { x: area.x, y: row, width: area.width, height: 1 },
                );
            }
            return;
        }

        // ── Episode list ─────────────────────────────────────────────────────
        let table_area = Rect {
            x: area.x, y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        if table_area.height == 0 { return; }

        let (active2, active_idx2, _, _, _) = self.effective_playback_state();
        let now_playing_id2: Option<String> = if active2 {
            self.player_tab.items.get(active_idx2).map(|i| i.id.clone())
        } else { None };

        // Prefetch card images for episodes near the cursor.
        const EP_AHEAD: usize = 4;
        const EP_BEHIND: usize = 2;
        let n = items.len();
        {
            let pf_start = ep_cursor.saturating_sub(EP_BEHIND);
            let pf_end   = (ep_cursor + EP_AHEAD + 1).min(n);
            let prefetch: Vec<(String, String, String)> = items[pf_start..pf_end].iter()
                .map(|ep| (
                    format!("{}:pwr_ep", ep.id),
                    ep.id.clone(),
                    ep.series_id.clone(),
                ))
                .collect();
            for (cache_key, ep_id, series_id) in prefetch {
                self.fetch_card_image(cache_key, ep_id, series_id, &["Primary", "Backdrop"]);
            }
            // Prefetch adjacent seasons' posters under the same key format so the
            // card shows something immediately when the user switches seasons.
            let series_id_adj = items.first().map(|ep| ep.series_id.clone()).unwrap_or_default();
            for delta in [-1i64, 1i64] {
                let adj = season_cursor as i64 + delta;
                if adj >= 0 && (adj as usize) < seasons.len() {
                    let s = &seasons[adj as usize];
                    let ck = format!("{}:pwr_ep", s.id);
                    self.fetch_card_image(ck, s.id.clone(), series_id_adj.clone(), &["Primary", "Backdrop"]);
                }
            }
        }

        let show_length = table_area.width > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (table_area.width as usize)
            .saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items.iter().enumerate().map(|(i, ep)| {
            let is_cursor  = i == ep_cursor;
            let is_playing = now_playing_id2.as_deref() == Some(ep.id.as_str());
            let row_style = if is_playing {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
            } else if is_cursor && focused {
                Style::default().fg(palette::YELLOW)
            } else if focused {
                Style::default().fg(palette::WHITE)
            } else {
                Style::default().fg(palette::SUBTLE)
            };
            let marker = if is_cursor && focused {
                Span::styled("\u{258c}", Style::default().fg(palette::PINE))
            } else {
                Span::raw(" ")
            };
            let ep_num_w = n.to_string().len();
            let ep_label = if ep.index_number > 0 {
                format!("{:>ep_num_w$}. ", ep.index_number)
            } else {
                format!("{:>ep_num_w$}. ", i + 1)
            };
            let label_w = ep_label.chars().count();
            let title = trunc_str(&ep.name, title_col_w.saturating_sub(label_w));
            let title_cell = Cell::from(Line::from(vec![
                marker,
                Span::styled(ep_label, Style::default().fg(palette::SUBTLE)),
                Span::raw(title),
            ]));
            let len_secs = ep.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration_approx(len_secs) } else { "\u{2014}".to_string() };
            if show_length {
                Row::new([
                    title_cell,
                    Cell::from(Line::from(length).alignment(Alignment::Right))
                        .style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(""),
                ]).style(row_style)
            } else {
                Row::new([title_cell, Cell::from(""), Cell::from("")]).style(row_style)
            }
        }).collect();

        let mut state = TableState::default();
        state.select(Some(ep_cursor));
        let table = Table::new(rows, [
            Constraint::Min(10),
            Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
            Constraint::Length(1),
        ])
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);

        let visible_rows = table_area.height as usize;
        if n > visible_rows {
            let max_offset = n.saturating_sub(visible_rows);
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(state.offset());
            // Inset one row at the top so the scrollbar doesn't sit flush against
            // the season pill bar.
            let sb_area = Rect {
                y: table_area.y + 1,
                height: table_area.height.saturating_sub(1),
                ..table_area
            };
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                sb_area, &mut sb_state,
            );
        }
    }


    /// Renders the combined music group view: a horizontal group-selector bar
    /// at the top (like the TV season bar) and the grouped-by-artist album
    /// list below.
    fn render_power_music_group_view(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        if area.height == 0 { return; }

        // ── Collect nav state ─────────────────────────────────────────────────
        let stack_len = self.libs[lib_idx].nav_stack.len();
        let (groups, group_cursor, albums, album_cursor) = {
            let lib = &self.libs[lib_idx];
            let last = match lib.nav_stack.last() { Some(l) => l, None => return };
            if stack_len >= 2 {
                let group_lvl = &lib.nav_stack[stack_len - 2];
                (group_lvl.items.clone(), group_lvl.cursor,
                 last.items.clone(), last.cursor)
            } else {
                (vec![], 0, last.items.clone(), last.cursor)
            }
        };

        let max_y = area.y + area.height;
        let mut row = area.y;

        // ── Group selector bar (blank · pills · blank) ───────────────────────
        // Blank spacer above pills.
        if row < max_y { row += 1; }

        // Pills row.
        if row < max_y && !groups.is_empty() {
            const MAX_LABEL: usize = 12;
            let tab_labels: Vec<String> = groups.iter()
                .map(|g| trunc_str(&g.name, MAX_LABEL).to_string())
                .collect();
            let n_tabs = tab_labels.len();

            // Actual display width of each pill: " label " = label_w + 2.
            // Gap between consecutive pills = 1 char.
            let pill_widths: Vec<usize> = tab_labels.iter()
                .map(|l| l.width() + 2)
                .collect();
            let bar_w = area.width as usize;

            // Greedy count: how many pills fit starting at `start` within `avail` chars.
            // Uses actual individual pill widths so short labels pack tightly.
            let count_fitting = |start: usize, avail: usize| -> usize {
                let mut used = 0usize;
                let mut count = 0usize;
                for i in start..n_tabs {
                    let need = if count == 0 { pill_widths[i] } else { 1 + pill_widths[i] };
                    if used + need > avail { break; }
                    used += need;
                    count += 1;
                }
                count
            };

            // Walk scroll_start forward until group_cursor is in the visible window.
            let mut scroll_start = 0usize;
            loop {
                let avail = bar_w
                    .saturating_sub(if scroll_start > 0 { 2 } else { 0 }) // "‹ "
                    .saturating_sub(2); // reserve for " ›"
                let cnt = count_fitting(scroll_start, avail);
                if cnt == 0 || scroll_start + cnt > group_cursor { break; }
                scroll_start += 1;
            }

            let has_left = scroll_start > 0;
            let avail_pills = bar_w
                .saturating_sub(if has_left { 2 } else { 0 })
                .saturating_sub(2); // reserve for " ›"
            let cnt = count_fitting(scroll_start, avail_pills);
            let scroll_end = (scroll_start + cnt).min(n_tabs);
            let has_right = scroll_end < n_tabs;

            let mut spans: Vec<Span> = Vec::new();
            if has_left {
                spans.push(Span::styled("\u{2039} ", Style::default().fg(palette::FOAM)));
            }
            for (idx, label) in tab_labels[scroll_start..scroll_end].iter().enumerate() {
                if idx > 0 {
                    spans.push(Span::raw(" "));
                }
                let abs_idx = scroll_start + idx;
                let selected = abs_idx == group_cursor;
                let style = if selected {
                    Style::default().fg(palette::YELLOW).bg(palette::FOAM).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::BASE).bg(palette::FOAM)
                };
                spans.push(Span::styled(format!(" {} ", label), style));
            }
            if has_right {
                spans.push(Span::styled(" \u{203a}", Style::default().fg(palette::FOAM)));
            }
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect { x: area.x, y: row, width: area.width, height: 1 },
            );
        }
        if row < max_y { row += 1; }

        // Blank spacer below pills.
        if row < max_y { row += 1; }

        // ── Loading / empty state ─────────────────────────────────────────────
        if albums.is_empty() {
            if row < max_y {
                let is_loading = self.libs[lib_idx].nav_stack.last()
                    .map(|l| l.loading)
                    .unwrap_or(false);
                let msg = if is_loading { " Loading\u{2026}" } else { " (empty)" };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        msg,
                        Style::default().fg(palette::MUTED),
                    ))),
                    Rect { x: area.x, y: row, width: area.width, height: 1 },
                );
            }
            return;
        }

        // ── Album list (grouped by artist) ────────────────────────────────────
        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        if list_area.height == 0 { return; }

        // Store for click/page-size calculations (used by mouse handler and PageUp/Down).
        self.power_left_area = list_area;
        self.power_left_sorted_indices.clear();
        self.power_left_row_map.clear();

        let visible = list_area.height as usize;
        let avail_chars = (list_area.width as usize).saturating_sub(2);

        enum DisplayRow { ArtistHeader(String), Album(usize) }

        // Extract (artist, year, album_name) for each album item.
        let album_info: Vec<(String, String, String)> = albums.iter().map(|item| {
            if !item.artist.is_empty() {
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                (item.artist.clone(), year_str, item.display_name())
            } else if let Some((artist, year, album)) = parse_album_folder_name(&item.name) {
                let year_str = if year > 0 { year.to_string() } else { String::new() };
                (artist, year_str, album)
            } else {
                ("Unknown Artist".to_string(), String::new(), item.display_name())
            }
        }).collect();

        // Build display rows: inject artist header at each artist boundary.
        let mut display_rows: Vec<DisplayRow> = Vec::new();
        let mut last_artist = String::new();
        for (idx, (artist, _, _)) in album_info.iter().enumerate() {
            if artist != &last_artist {
                display_rows.push(DisplayRow::ArtistHeader(artist.clone()));
                last_artist = artist.clone();
            }
            display_rows.push(DisplayRow::Album(idx));
        }

        // Locate the cursor row and derive the scroll offset.
        let display_cursor = display_rows.iter().position(|r| {
            matches!(r, DisplayRow::Album(i) if *i == album_cursor)
        }).unwrap_or(0);
        let offset = if display_cursor >= visible { display_cursor - visible + 1 } else { 0 };

        let list_items: Vec<ListItem> = display_rows.iter().skip(offset).take(visible).map(|dr| {
            match dr {
                DisplayRow::ArtistHeader(name) => {
                    let artist_label = trunc_str(name, avail_chars);
                    ListItem::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(artist_label, Style::default().fg(palette::YELLOW)),
                    ]))
                }
                DisplayRow::Album(idx) => {
                    let selected = *idx == album_cursor;
                    let (_, year_str, album_name) = &album_info[*idx];
                    let prefix_w = if year_str.is_empty() { 3 } else { year_str.len() + 6 };
                    let name_w = avail_chars.saturating_sub(prefix_w);
                    let trunc_name = trunc_str(album_name, name_w);
                    let fg = if focused { palette::WHITE } else { palette::SUBTLE };
                    let name_color = if selected && focused { palette::IRIS } else { fg };
                    let mut spans: Vec<Span> = Vec::new();
                    if selected && focused {
                        spans.push(Span::styled("\u{258c}", Style::default().fg(palette::PINE)));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    if year_str.is_empty() {
                        spans.push(Span::raw("   "));
                    } else {
                        spans.push(Span::styled("   (", Style::default().fg(palette::SUBTLE)));
                        spans.push(Span::styled(year_str.clone(), Style::default().fg(palette::PINE)));
                        spans.push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
                    }
                    let name_style = if selected && focused {
                        Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(name_color)
                    };
                    spans.push(Span::styled(trunc_name.to_string(), name_style));
                    ListItem::new(Line::from(spans))
                }
            }
        }).collect();

        let mut state = ListState::default();
        state.select(Some(display_cursor.saturating_sub(offset)));
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            list_area,
            &mut state,
        );

        let display_n = display_rows.len();
        if focused && display_n > visible {
            let max_off = display_n.saturating_sub(visible);
            let mut sb = ScrollbarState::new(max_off + 1).position(offset);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                list_area,
                &mut sb,
            );
        }
    }
}
