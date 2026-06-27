use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, PowerFocus, palette};
use super::super::ui_util::{fmt_duration, item_text_and_style, trunc_str};
use unicode_width::UnicodeWidthStr;



impl App {
    pub(super) fn render_power_view(&mut self, f: &mut Frame, area: Rect) {
        if area.height < 4 { return; }

        // Drop focus if Continue Watching panel has been emptied while Left is focused.
        if matches!(self.power_focus, PowerFocus::Left)
            && self.power_left_tab == 0
            && self.home.continue_items.is_empty()
        {
            self.power_focus = PowerFocus::Queue;
        }

        // Left panel (fixed 38 cols) │ Right panel (queue, remaining).
        let left_w: u16 = 44;
        let right_w = area.width.saturating_sub(left_w + 1);
        let divider_x = area.x + left_w;

        let left_area  = Rect { x: area.x,        y: area.y, width: left_w,  height: area.height };
        let right_area = Rect { x: divider_x + 1, y: area.y, width: right_w, height: area.height };

        let queue_focused = matches!(self.power_focus, PowerFocus::Queue);
        let left_focused  = !queue_focused;

        let bar_y      = self.render_power_left_panel(f, left_area, left_focused);
        let divider_ys = self.render_power_queue(f, right_area, queue_focused);

        // Vertical divider; ┤ at bar_y, ├ at album-group divider rows, │ elsewhere.
        for y in area.y..area.y + area.height {
            let ch = if divider_ys.contains(&y) { "\u{251c}" }
                     else if Some(y) == bar_y   { "\u{2524}" }
                     else                       { "\u{2502}" };
            f.render_widget(
                Paragraph::new(Span::styled(ch, Style::default().fg(palette::IRIS))),
                Rect { x: divider_x, y, width: 1, height: 1 },
            );
        }
    }

    

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

        // Display row types for the queue.
        // Track(item_idx, in_group) — in_group drives the 2-space indent.
        #[derive(Clone)]
        enum DRow { Header, Spacer, Track(usize, bool) }

        // Build display rows.
        // Audio items group by album ("Artist: Album"), episodes group by series name.
        // Movies, home videos, and everything else are ungrouped (no header/divider).
        // group_for_header[j] holds the display label for the j-th DRow::Header.
        let mut display: Vec<DRow> = Vec::new();
        let mut group_for_header: Vec<String> = Vec::new();
        let mut last_group_key: Option<String> = None;
        for (i, item) in items.iter().enumerate() {
            let group = if item.is_audio() && !item.album.is_empty() {
                let key = format!("a:{}", item.album);
                let label = if item.artist.is_empty() {
                    item.album.clone()
                } else {
                    format!("{}: {}", item.artist, item.album)
                };
                Some((key, label))
            } else if item.item_type == "Episode" && !item.series_name.is_empty() {
                Some((format!("e:{}", item.series_name), item.series_name.clone()))
            } else {
                None
            };

            let in_group = group.is_some();
            if let Some((key, label)) = group {
                if last_group_key.as_deref() != Some(key.as_str()) {
                    if last_group_key.is_some() {
                        display.push(DRow::Spacer);
                    }
                    display.push(DRow::Header);
                    group_for_header.push(label);
                    last_group_key = Some(key);
                }
            } else {
                last_group_key = None;
            }
            display.push(DRow::Track(i, in_group));
        }
        let total = display.len();
        let visible = area.height as usize;

        // Visual row of the cursor item.
        let cursor_row = display.iter().position(|r| {
            if let DRow::Track(idx, _) = r { *idx == cursor } else { false }
        }).unwrap_or(0);
        let offset = if cursor_row >= visible { cursor_row - visible + 1 } else { 0 };

        // Count how many group headers appear before the scroll offset, so we
        // index group_for_header correctly for the visible window.
        let mut header_idx = display[..offset].iter().filter(|r| matches!(r, DRow::Header)).count();

        // Collect absolute y positions of divider rows in the visible window.
        let mut divider_ys: Vec<u16> = Vec::new();

        let need_sb = total > visible;
        let render_w = area.width.saturating_sub(if need_sb { 1 } else { 0 }) as usize;
        let show_length = render_w > 30;
        let dur_w: usize = if show_length { 6 } else { 0 }; // "mm:ss" or "h:mm:ss"

        // Build visible ListItems and the row map simultaneously.
        self.power_queue_row_map.clear();
        let mut list_items: Vec<ListItem> = Vec::new();

        for (row_idx, entry) in display.iter().skip(offset).take(visible).enumerate() {
            match entry {
                DRow::Header => {
                    let group = group_for_header.get(header_idx).map(|s| s.as_str()).unwrap_or("");
                    header_idx += 1;
                    // " TITLE ──────" — title overlaid on the divider line.
                    let max_label = render_w.saturating_sub(3); // 1 prefix + 1 space + 1+ dashes
                    let label = trunc_str(group, max_label);
                    let label_w = label.width();
                    let dashes = render_w.saturating_sub(1 + label_w + 1);
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(label, Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                        Span::raw(" "),
                        Span::styled("\u{2500}".repeat(dashes), Style::default().fg(palette::IRIS)),
                    ])));
                    self.power_queue_row_map.push(None);
                    divider_ys.push(area.y + row_idx as u16);
                }
                DRow::Spacer => {
                    list_items.push(ListItem::new(Line::raw("")));
                    self.power_queue_row_map.push(None);
                }
                DRow::Track(idx, in_group) => {
                    let i = *idx;
                    let indent: usize = if *in_group { 2 } else { 0 };
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
                        Span::styled("\u{258c}", Style::default().fg(palette::IRIS))
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

                    // Braille spinner shown right after the title while the item is playing.
                    const SPINNER_FRAMES: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
                    let spinner_char: &str = if is_active {
                        let ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis();
                        SPINNER_FRAMES[(ms / 150) as usize % SPINNER_FRAMES.len()]
                    } else { "" };

                    // Reserve 2 extra chars for " ⠋" when active.
                    let spinner_w: usize = if is_active { 2 } else { 0 };
                    // Title truncated to leave room for indent + marker + spinner + duration + pct.
                    let extra = dur_w + pct_str.chars().count() + spinner_w;
                    let title_w = render_w.saturating_sub(indent + 1 + extra); // 1 for marker
                    let title = trunc_str(&label, title_w);

                    // Now-playing title text is always emby blue, regardless of focus state.
                    let title_color = if is_active { palette::FOAM } else { fg };

                    let mut spans: Vec<Span> = Vec::new();
                    if indent > 0 { spans.push(Span::raw("  ")); }
                    spans.push(marker);
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
                        let pct_color = if is_active { palette::IRIS } else { palette::YELLOW };
                        spans.push(Span::styled(pct_str, Style::default().fg(pct_color)));
                    }
                    if show_length && !dur.is_empty() {
                        let dur_color = dim_color;
                        // Right-align duration within render_w.
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

        divider_ys
    }

    /// Renders the card image and returns the number of rows it actually occupied.
    /// Returns 0 if the queue is empty or the image is not yet loaded.
    fn render_power_card(&mut self, f: &mut Frame, area: Rect) -> u16 {
        let cursor = self.player_tab.playlist_cursor;
        let n = self.player_tab.items.len();
        if n == 0 { return 0; }
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
        if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
            type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
            let avail = ratatui::layout::Size { width: area.width, height: area.height };
            let actual = state.size_for(
                ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail,
            );
            let img_x = area.x + (area.width.saturating_sub(actual.width)) / 2;
            let img_rect = Rect { x: img_x, y: area.y, width: actual.width, height: actual.height };
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                img_rect, state,
            );
            actual.height
        } else {
            0
        }
    }

    /// Returns the y coordinate of the horizontal bar drawn at the top of the lib section,
    /// so the caller can place a ┤ junction on the vertical divider at that row.
    fn render_power_left_panel(&mut self, f: &mut Frame, area: Rect, focused: bool) -> Option<u16> {
        if area.height == 0 { return None; }

        // Render card into the full area; it returns the rows it actually used.
        // The library panel then fills whatever vertical space remains.
        let card_h = self.render_power_card(f, area);
        let lib_area = Rect { y: area.y + card_h, height: area.height.saturating_sub(card_h), ..area };

        if lib_area.height == 0 { return None; }

        // Ensure the library is loaded when a library tab is selected.
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
        }

        // Header: " NAME ─────" overlaid on the iris bar (matches queue group-header style).
        let area = lib_area;
        let header_name = if self.power_left_tab == 0 {
            "Continue".to_string()
        } else {
            self.libs[self.power_left_tab - 1].library.name.clone()
        };
        if area.height < 1 { return None; }
        let bar_y = area.y;
        let w = area.width as usize;

        // Build the text spans (prefix space + label), then append " ────" to fill width.
        let mut header_spans: Vec<Span<'static>> = vec![Span::raw(" ")];
        if self.power_left_tab > 0 {
            let lib_idx = self.power_left_tab - 1;
            let lib = &self.libs[lib_idx];
            let skip = if lib.nav_stack.first()
                .map(|l| l.title == lib.library.name).unwrap_or(false) { 1 } else { 0 };
            let mut crumbs: Vec<String> = vec![lib.library.name.clone()];
            for lvl in lib.nav_stack.iter().skip(skip) {
                crumbs.push(lvl.title.clone());
            }
            if crumbs.len() > 1 {
                for (ci, name) in crumbs.iter().enumerate() {
                    let is_last = ci + 1 == crumbs.len();
                    let display: String = if is_last { name.clone() } else { format!("[{}]", ci + 1) };
                    let style = if is_last {
                        Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::MUTED)
                    };
                    header_spans.push(Span::styled(display, style));
                    if !is_last {
                        header_spans.push(Span::styled("/", Style::default().fg(palette::IRIS)));
                    }
                }
            } else {
                let budget = w.saturating_sub(3);
                header_spans.push(Span::styled(
                    trunc_str(&header_name, budget),
                    Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD),
                ));
            }
        } else {
            let budget = w.saturating_sub(3);
            header_spans.push(Span::styled(
                trunc_str(&header_name, budget),
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD),
            ));
        }
        let used: usize = header_spans.iter().map(|s| s.content.as_ref().width()).sum();
        let dashes = w.saturating_sub(used + 1);
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled("\u{2500}".repeat(dashes), Style::default().fg(palette::IRIS)));
        f.render_widget(
            Paragraph::new(Line::from(header_spans)),
            Rect { x: area.x, y: bar_y, width: area.width, height: 1 },
        );

        let content_area = Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area };
        if content_area.height == 0 { return Some(bar_y); }

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
            return Some(bar_y);
        }

        let visible = content_area.height as usize;
        let offset = if cursor >= visible { cursor - visible + 1 } else { 0 };

        let list_items: Vec<ListItem> = items.iter().skip(offset).take(visible).enumerate().map(|(i, item)| {
            let abs = offset + i;
            let selected = abs == cursor;
            let (text, _) = item_text_and_style(item, selected);
            let title = trunc_str(&text, (area.width as usize).saturating_sub(2));
            let fg = if focused { palette::WHITE } else { palette::SUBTLE };
            let line = if selected && focused {
                Line::from(vec![
                    Span::styled("\u{258c}", Style::default().fg(palette::IRIS)),
                    Span::styled(title, Style::default().fg(palette::YELLOW)),
                ])
            } else {
                Line::from(vec![
                    Span::raw(" "),
                    Span::styled(title, Style::default().fg(fg)),
                ])
            };
            ListItem::new(line)
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
        Some(bar_y)
    }
}
