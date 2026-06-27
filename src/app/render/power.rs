use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, PowerFocus, palette};
use super::super::ui_util::{build_queue_rows, fmt_duration, trunc_str, QueueRow};
use unicode_width::UnicodeWidthStr;



impl App {
    pub(super) fn render_power_view(&mut self, f: &mut Frame, area: Rect) {
        if area.height < 4 { return; }

        // Left panel (fixed 44 cols) | Right panel (queue, remaining).
        let left_w: u16 = 44;
        let right_w = area.width.saturating_sub(left_w);

        let left_area  = Rect { x: area.x, y: area.y + 1, width: left_w,  height: area.height.saturating_sub(1) };
        let right_area = Rect { x: area.x + left_w, y: area.y, width: right_w, height: area.height };

        let queue_focused = matches!(self.power_focus, PowerFocus::Queue);
        let left_focused  = !queue_focused;

        // Static "Queue" header spanning the full terminal width at the top row.
        // (The left panel is shifted down one row so this row is clear on both sides.)
        {
            let full_w = area.width as usize;
            let pill = " QUEUE ";
            let pill_w = pill.len();
            let right = 3usize.min(full_w.saturating_sub(pill_w));
            let left = full_w.saturating_sub(pill_w + right);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("\u{2500}".repeat(left), Style::default().fg(palette::FOAM)),
                    Span::styled(pill, Style::default().fg(palette::WHITE).bg(palette::FOAM)),
                    Span::styled("\u{2500}".repeat(right), Style::default().fg(palette::FOAM)),
                ])),
                Rect { x: area.x, y: right_area.y, width: area.width, height: 1 },
            );
        }
        // Content area for the queue list sits one row below the static header.
        let right_content = Rect { y: right_area.y + 1, height: right_area.height.saturating_sub(1), ..right_area };

        // The card fills the left column; the Continue/library list takes the rows
        // below it. At low heights the card can consume the whole column, so relocate
        // the list under the queue on the right instead of cramming it in.
        let (card_h, image_loading) = self.render_power_card(f, left_area);
        let left_remaining = left_area.height.saturating_sub(card_h);

        const MIN_LIST_ROWS: u16 = 6;
        let _ = if left_remaining < MIN_LIST_ROWS {
            // Split the right column: queue on top, relocated list on the bottom.
            let h = right_content.height;
            let min_l = MIN_LIST_ROWS.min(h);
            let max_l = h.saturating_sub(MIN_LIST_ROWS).max(min_l);
            let list_h = (h / 3).clamp(min_l, max_l);
            let queue_h = h.saturating_sub(list_h);
            let queue_area = Rect { height: queue_h, ..right_content };
            let list_area = Rect { y: right_content.y + queue_h, height: list_h, ..right_content };
            let mut header_ys = self.render_power_queue(f, queue_area, queue_focused);
            // Only skip left-panel content while the image is loading, not the whole view.
            if !image_loading {
                if self.power_detail_item.is_some() {
                    self.render_power_detail(f, list_area, left_focused);
                } else if self.power_left_tab > 0 && self.is_album_level(self.power_left_tab - 1) {
                    self.render_power_album_detail(f, list_area, self.power_left_tab - 1, left_focused);
                } else {
                    let (list_bar_y, _crumbs) = self.render_power_list(f, list_area, left_focused);
                    if let Some(by) = list_bar_y { header_ys.push(by); }
                }
            }
            (None, Vec::new(), header_ys)
        } else {
            let lib_area = Rect { y: left_area.y + card_h, height: left_remaining, ..left_area };
            let header_ys = self.render_power_queue(f, right_content, queue_focused);
            if !image_loading {
                if self.power_detail_item.is_some() {
                    self.render_power_detail(f, lib_area, left_focused);
                } else if self.power_left_tab > 0 && self.is_album_level(self.power_left_tab - 1) {
                    self.render_power_album_detail(f, lib_area, self.power_left_tab - 1, left_focused);
                } else {
                    self.render_power_list(f, lib_area, left_focused);
                }
            }
            (None::<u16>, Vec::<(u16, char)>::new(), header_ys)
        };


    }

    

    /// Returns the absolute y positions of visible group-header rows, so the caller
    /// can draw a ├ junction where each header line meets the vertical divider.
    fn render_power_queue(&mut self, f: &mut Frame, area: Rect, focused: bool) -> Vec<u16> {
        if area.height < 1 { return vec![]; }
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

        let (active, active_idx, live_pos, live_runtime, _) = self.effective_playback_state();
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

        let need_sb = total > visible;
        let render_w = area.width.saturating_sub(if need_sb { 1 } else { 0 }) as usize;
        let show_length = render_w > 30;
        let dur_w: usize = if show_length { 6 } else { 0 }; // "mm:ss" or "h:mm:ss"

        // Spinner character for the active item — computed once per frame, not per row.
        const SPINNER_FRAMES: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
        let spinner_frame: &str = {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            SPINNER_FRAMES[(ms / 150) as usize % SPINNER_FRAMES.len()]
        };

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
                    let label = trunc_str(group, render_w.saturating_sub(2));
                    list_items.push(ListItem::new(Line::from(
                        Span::styled(format!("  {}", label.to_uppercase()), Style::default().fg(palette::YELLOW).add_modifier(ratatui::style::Modifier::BOLD)),
                    )));
                    self.power_queue_row_map.push(None);
                }
                QueueRow::Spacer => {
                    list_items.push(ListItem::new(Line::raw("")));
                    self.power_queue_row_map.push(None);
                }
                QueueRow::Track { idx, in_group } => {
                    let i = *idx;
                    let indent: usize = if *in_group { 1 } else { 0 };
                    let item = &items[i];
                    let is_active = i == active_idx && active;
                    let is_cursor = i == cursor && focused;

                    let fg = if is_cursor {
                        palette::YELLOW
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

                    // For audio under an album header: show track number + bare name.
                    // For episodes under a series header: bare episode name (series already shown).
                    // Otherwise use the standard playback label.
                    let label = if item.is_audio() {
                        if item.index_number > 0 {
                            format!("{:02}. {}", item.index_number, item.name)
                        } else {
                            item.name.clone()
                        }
                    } else if *in_group && item.item_type == "Episode" {
                        item.name.clone()
                    } else {
                        item.playback_label()
                    };

                    let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                    let dur = if len_secs > 0 { fmt_duration(len_secs) } else { String::new() };
                    let dim_color = if focused { palette::SUBTLE } else { palette::MUTED };

                    // Spinner shown right after the title while the item is playing.
                    let spinner_char: &str = if is_active { spinner_frame } else { "" };

                    // Reserve 2 extra chars for " ⠋" when active.
                    let spinner_w: usize = if is_active { 2 } else { 0 };
                    // Title truncated to leave room for indent + marker + spinner + duration + pct.
                    let extra = dur_w + pct_str.chars().count() + spinner_w;
                    let title_w = render_w.saturating_sub(indent + 2 + extra); // 1 marker + 1 space
                    let title = trunc_str(&label, title_w);

                    // Now-playing title text is always emby blue, regardless of focus state.
                    let title_color = if is_active { palette::FOAM } else { fg };

                    let mut spans: Vec<Span> = Vec::new();
                    if indent > 0 { spans.push(Span::raw(" ")); }
                    spans.push(marker);
                    spans.push(Span::raw(" "));
                    // For audio tracks with an index: "01. ⠋ Title" when active,
                    // "01. Title" otherwise. Spinner goes between the dim prefix and the name.
                    if item.is_audio() && item.index_number > 0 {
                        let prefix_chars = 4; // "01. "
                        let tc = title.chars().count();
                        if tc > prefix_chars {
                            let split = title.char_indices().nth(prefix_chars).map(|(i, _)| i).unwrap_or(title.len());
                            spans.push(Span::styled(title[..split].to_string(), Style::default().fg(dim_color)));
                            if is_active {
                                spans.push(Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)));
                                spans.push(Span::raw(" "));
                            }
                            spans.push(Span::styled(title[split..].to_string(), Style::default().fg(title_color)));
                        } else {
                            if is_active {
                                spans.push(Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)));
                                spans.push(Span::raw(" "));
                            }
                            spans.push(Span::styled(title, Style::default().fg(title_color)));
                        }
                    } else {
                        if is_active {
                            spans.push(Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)));
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
                        // Right-align duration to the blue header box's right edge, which
                        // sits 2 cols in from render_w (the header's 2-dash tail).
                        let used: usize = spans.iter().map(|s| s.content.as_ref().width()).sum();
                        let pad = render_w.saturating_sub(3 + used + dur.width());
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

    /// Renders the card image and returns `(rows_used, image_loading)`.
    /// `rows_used` is 0 if the queue is empty or the image is not yet ready.
    /// `image_loading` is true when a fetch is in-flight (caller should defer
    /// rendering the rest of the view until the image arrives).
    fn render_power_card(&mut self, f: &mut Frame, area: Rect) -> (u16, bool) {
        // If a movie detail is pinned, show that item's image instead of the queue cursor item.
        if self.power_detail_item.is_some() {
            // (handled below)
        } else if self.power_left_tab > 0 && self.is_album_level(self.power_left_tab - 1) {
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
            let image_loading = self.card_image_loading.contains(&cache_key);
            if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                let avail = ratatui::layout::Size { width: area.width.saturating_sub(2), height: area.height };
                let actual = state.size_for(
                    ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail,
                );
                let img_x = area.x + 1 + (area.width.saturating_sub(2).saturating_sub(actual.width)) / 2;
                let img_rect = Rect { x: img_x, y: area.y, width: actual.width, height: actual.height };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                    img_rect, state,
                );
                return (actual.height, false);
            } else {
                return (0, image_loading);
            }
        }

        if self.power_detail_item.is_some() {
            let (detail_id, series_id) = {
                let d = self.power_detail_item.as_ref().unwrap();
                (d.id.clone(), d.series_id.clone())
            };
            let img_types: &[&str] = &["Backdrop", "Primary", "Logo"];
            let cache_key = format!("{}:P", detail_id);
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), detail_id, series_id, img_types);
            }
            let image_loading = self.card_image_loading.contains(&cache_key);
            if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                let avail = ratatui::layout::Size { width: area.width.saturating_sub(2), height: area.height };
                let actual = state.size_for(
                    ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail,
                );
                let img_x = area.x + 1 + (area.width.saturating_sub(2).saturating_sub(actual.width)) / 2;
                let img_rect = Rect { x: img_x, y: area.y, width: actual.width, height: actual.height };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                    img_rect, state,
                );
                return (actual.height, false);
            } else {
                return (0, image_loading);
            }
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
        let image_loading = self.card_image_loading.contains(&cache_key);
        if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
            type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
            let avail = ratatui::layout::Size { width: area.width.saturating_sub(2), height: area.height };
            let actual = state.size_for(
                ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail,
            );
            let img_x = area.x + 1 + (area.width.saturating_sub(2).saturating_sub(actual.width)) / 2;
            let img_rect = Rect { x: img_x, y: area.y, width: actual.width, height: actual.height };
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                img_rect, state,
            );
            (actual.height, false)
        } else {
            (0, image_loading)
        }
    }

    /// Renders the Continue/library list (header + items) into `area` — the rows below
    /// the card, or a relocated slot under the queue at low heights.
    /// Returns (bar_y, crumb_chars): bar_y is the header row for a ┤/├ divider junction,
    /// and crumb_chars are ancestor-level digit indicators to overlay on the divider.
    fn render_power_list(&mut self, f: &mut Frame, area: Rect, focused: bool) -> (Option<u16>, Vec<(u16, char)>) {
        if area.height == 0 { return (None, vec![]); }

        // Ensure the library is loaded when a library tab is selected.
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
        }

        // Header: "───── NAME" — FOAM line with a right-aligned label pill (matches queue group-header style).
        let header_name = if self.power_left_tab == 0 {
            "Keep Watching".to_string()
        } else {
            self.libs[self.power_left_tab - 1].library.name.clone()
        };
        if area.height < 1 { return (None, vec![]); }
        let bar_y = area.y;
        let w = area.width as usize;

        // FOAM line on the left, right-aligned label pill (dark text on FOAM) at the
        // panel edge — matches the queue group-header style.
        // Ancestor breadcrumb levels are NOT shown in the header — they appear as [N] indicators
        // stacked vertically on the right divider (see crumb_chars below).
        let mut crumb_chars: Vec<(u16, char)> = Vec::new();
        let budget = w.saturating_sub(5);
        let label = if self.power_left_tab > 0 {
            let lib_idx = self.power_left_tab - 1;
            let lib = &self.libs[lib_idx];
            let skip = if lib.nav_stack.first()
                .map(|l| l.title == lib.library.name).unwrap_or(false) { 1 } else { 0 };
            let mut crumbs: Vec<String> = vec![lib.library.name.clone()];
            for lvl in lib.nav_stack.iter().skip(skip) {
                crumbs.push(lvl.title.clone());
            }
            // Always show only the current (last) level in the header.
            let lbl = trunc_str(crumbs.last().unwrap_or(&header_name), budget);
            // Build vertical digit indicators for ancestor levels (all but the last crumb).
            // Layout (vertical): │ 1 · 2 │ — digits with · between, surrounded by normal │.
            if crumbs.len() > 1 {
                let n = crumbs.len() - 1;
                let mut row: u16 = bar_y + 2; // +1 blank row above first digit
                for ci in 0..n {
                    if ci > 0 {
                        crumb_chars.push((row, '\u{00b7}')); // middle dot between digits
                        row += 1;
                    }
                    let digit = char::from_digit((ci + 1) as u32, 10).unwrap_or('?');
                    crumb_chars.push((row, digit));
                    row += 1;
                }
            }
            lbl
        } else {
            trunc_str(&header_name, budget)
        };
        let pill = format!(" {} ", label.to_uppercase());
        let pill_w = pill.width();
        let left = 2usize.min(w.saturating_sub(pill_w));
        let right = w.saturating_sub(pill_w + left);
        let header_spans: Vec<Span<'static>> = vec![
            Span::styled("\u{2500}".repeat(left), Style::default().fg(palette::FOAM)),
            Span::styled(pill, Style::default().fg(palette::WHITE).bg(palette::FOAM)),
            Span::styled("\u{2500}".repeat(right), Style::default().fg(palette::FOAM)),
        ];
        f.render_widget(
            Paragraph::new(Line::from(header_spans)),
            Rect { x: area.x, y: bar_y, width: area.width, height: 1 },
        );

        let content_area = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        if content_area.height == 0 { return (Some(bar_y), crumb_chars); }

        // Store for click / page-size calculations.
        self.power_left_area = content_area;

        // Gather items and cursor from the appropriate source.
        let (items, cursor) = if self.power_left_tab == 0 {
            let items = self.home.continue_items.clone();
            let cursor = self.home.continue_cursor.min(items.len().saturating_sub(1).max(0));
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

        let n = items.len();
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
            return (Some(bar_y), crumb_chars);
        }

        let visible = content_area.height as usize;
        let offset = if cursor >= visible { cursor - visible + 1 } else { 0 };

        let list_items: Vec<ListItem> = items.iter().skip(offset).take(visible).enumerate().map(|(i, item)| {
            let abs = offset + i;
            let selected = abs == cursor;

            // Compute name and duration as separate strings so they can be styled
            // independently: name in the normal fg, duration in OVERLAY (no parens).
            let (item_name, dur_str) = if item.is_folder {
                let name = if item.item_type == "Folder" && item.total_count > 0 {
                    format!("{} \u{b7} {} items", item.display_name(), item.total_count)
                } else if item.unplayed_item_count > 0 {
                    format!("{} [{}]", item.display_name(), item.unplayed_item_count)
                } else {
                    item.display_name()
                };
                (name, String::new())
            } else {
                let dur = if item.runtime_ticks > 0 {
                    let secs = (item.runtime_ticks / TICKS_PER_SECOND) as u64;
                    let h = secs / 3600;
                    let m = (secs % 3600) / 60;
                    if h > 0 { format!(" {h}h{m:02}m") } else { format!(" {m}m") }
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
                    Span::styled(title, Style::default().fg(palette::YELLOW)),
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
        (Some(bar_y), crumb_chars)
    }

    /// Renders the movie detail panel (title, metadata, overview, director) into `area`.
    /// Called instead of `render_power_list` when `power_detail_item` is Some.
    fn render_power_detail(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        // Clone so self is free for scroll-state writes below.
        let item = match self.power_detail_item.clone() { Some(it) => it, None => return };
        if area.height == 0 { return; }

        let inner_x = area.x + 1;
        let inner_w = (area.width as usize).saturating_sub(2);
        let inner_w16 = area.width.saturating_sub(2);
        let max_y = area.y + area.height;
        let mut row = area.y;

        let title_color = if focused { palette::YELLOW } else { palette::SUBTLE };
        let text_color  = if focused { palette::WHITE  } else { palette::SUBTLE };

        // — Title —
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
                let secs = (item.runtime_ticks / TICKS_PER_SECOND) as u64;
                let h = secs / 3600;
                let m = (secs % 3600) / 60;
                if h > 0 { format!("{}h{}m", h, m) } else { format!("{}m", m) }
            } else {
                String::new()
            };
            let year_str = if item.production_year > 0 { item.production_year.to_string() } else { String::new() };
            let meta = [item.genre.as_str(), year_str.as_str(), dur_str.as_str()]
                .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  ");
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&meta, inner_w),
                    Style::default().fg(palette::SUBTLE),
                ))),
                Rect { x: inner_x, y: row, width: inner_w16, height: 1 },
            );
            row += 1;
        }

        // — Technical: video_info  audio_info (MUTED) —
        if row < max_y && (!item.video_info.is_empty() || !item.audio_info.is_empty()) {
            let tech = [item.video_info.as_str(), item.audio_info.as_str()]
                .iter().filter(|s| !s.is_empty()).copied().collect::<Vec<_>>().join("  ");
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&tech, inner_w),
                    Style::default().fg(palette::MUTED),
                ))),
                Rect { x: inner_x, y: row, width: inner_w16, height: 1 },
            );
            row += 1;
        }

        // — Blank separator —
        if row < max_y { row += 1; }

        // — Overview (scrollable, word-wrapped) —
        if !item.overview.is_empty() && row < max_y {
            let dir_reserve: u16 = if item.director.is_empty() { 0 } else { 1 };
            let avail = max_y.saturating_sub(row).saturating_sub(dir_reserve) as usize;
            let ov_start_y = row;

            // Word-wrap ALL overview lines so we can scroll through them.
            let mut all_ov_lines: Vec<String> = Vec::new();
            let mut cur = String::new();
            for word in item.overview.split_whitespace() {
                let word_w = word.width();
                if cur.is_empty() {
                    cur.push_str(word);
                } else if cur.width() + 1 + word_w <= inner_w {
                    cur.push(' ');
                    cur.push_str(word);
                } else {
                    all_ov_lines.push(std::mem::take(&mut cur));
                    cur.push_str(word);
                }
            }
            if !cur.is_empty() { all_ov_lines.push(cur); }

            let total = all_ov_lines.len();
            let max_scroll = total.saturating_sub(avail);
            let scroll = self.power_detail_scroll.min(max_scroll);
            self.power_detail_scroll = scroll;
            self.power_detail_max_scroll = max_scroll;
            self.power_detail_page_h = avail.max(1);

            if avail > 0 {
                for line_text in all_ov_lines.iter().skip(scroll).take(avail) {
                    if row >= max_y.saturating_sub(dir_reserve) { break; }
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            line_text.clone(), Style::default().fg(text_color),
                        ))),
                        Rect { x: inner_x, y: row, width: inner_w16, height: 1 },
                    );
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

        // — Director (MUTED, pinned to last row) —
        let _ = row;
        if !item.director.is_empty() && max_y > area.y {
            let dir_str = format!("Director: {}", item.director);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&dir_str, inner_w),
                    Style::default().fg(palette::MUTED),
                ))),
                Rect { x: inner_x, y: max_y - 1, width: inner_w16, height: 1 },
            );
        }
    }

    /// Renders the music album detail panel (track list) into `area` — the lib
    /// slot below the card. The card itself already shows the album art (handled
    /// in `render_power_card`). Mirrors `render_power_detail` for movies.
    fn render_power_album_detail(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        if area.height == 0 { return; }

        let (items, cursor, album_name) = {
            let lib = &self.libs[lib_idx];
            let lvl = match lib.nav_stack.last() { Some(l) => l, None => return };
            (lvl.items.clone(), lvl.cursor, lvl.title.clone())
        };
        let n = items.len();
        let first = match items.first() { Some(t) => t.clone(), None => return };
        let artist = first.artist.clone();
        let year = first.production_year;

        let inner_x   = area.x + 1;
        let inner_w   = area.width.saturating_sub(2) as usize;
        let inner_w16 = area.width.saturating_sub(2);
        let max_y     = area.y + area.height;
        let mut row   = area.y;

        let title_color = if focused { palette::YELLOW } else { palette::SUBTLE };

        // — Album name —
        if row < max_y {
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    trunc_str(&album_name, inner_w),
                    Style::default().fg(title_color),
                ))),
                Rect { x: inner_x, y: row, width: inner_w16, height: 1 },
            );
            row += 1;
        }

        // — Artist  year (SUBTLE) —
        if row < max_y {
            let year_str = if year > 0 { year.to_string() } else { String::new() };
            let meta = [artist.as_str(), year_str.as_str()]
                .iter().filter(|s| !s.is_empty()).copied()
                .collect::<Vec<_>>().join("  ");
            if !meta.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(&meta, inner_w),
                        Style::default().fg(palette::SUBTLE),
                    ))),
                    Rect { x: inner_x, y: row, width: inner_w16, height: 1 },
                );
                row += 1;
            }
        }

        // — Blank separator —
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
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "\u{2014}".to_string() };
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
}
