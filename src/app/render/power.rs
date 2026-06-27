use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, PowerFocus, palette};
use super::super::ui_util::{fmt_duration, item_text_and_style, trunc_str};



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

        let bar_y = self.render_power_left_panel(f, left_area, left_focused);
        self.render_power_queue(f, right_area, queue_focused);

        // Vertical divider; use ┤ at the row where the horizontal bar meets it.
        for y in area.y..area.y + area.height {
            let ch = if Some(y) == bar_y { "\u{2524}" } else { "\u{2502}" };
            f.render_widget(
                Paragraph::new(Span::styled(ch, Style::default().fg(palette::IRIS))),
                Rect { x: divider_x, y, width: 1, height: 1 },
            );
        }
    }

    

    fn render_power_queue(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height < 1 { return; }
        self.power_queue_area = area;

        let n = self.player_tab.items.len();
        if n == 0 {
            self.power_queue_row_map.clear();
            f.render_widget(
                Paragraph::new("  Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                area,
            );
            return;
        }

        let (active, active_idx, live_pos, live_runtime, _) = self.effective_playback_state();
        let cursor = self.player_tab.playlist_cursor;
        let items = &self.player_tab.items;

        // Build display rows: album-name headers for audio items, then track rows.
        // display[j] = None → album header; Some(i) → items[i].
        // album_for_header[j] holds the album name when display[j] is None.
        let mut display: Vec<Option<usize>> = Vec::new();
        let mut album_for_header: Vec<String> = Vec::new();
        let mut last_album: Option<&str> = None;
        for (i, item) in items.iter().enumerate() {
            if item.is_audio() {
                let album = item.album.as_str();
                if last_album != Some(album) {
                    display.push(None);
                    album_for_header.push(item.album.clone());
                    last_album = Some(album);
                }
            } else {
                last_album = None;
            }
            display.push(Some(i));
        }
        let total = display.len();
        let visible = area.height as usize;

        // Visual row of the cursor item.
        let cursor_row = display.iter().position(|r| *r == Some(cursor)).unwrap_or(0);
        let offset = if cursor_row >= visible { cursor_row - visible + 1 } else { 0 };

        // Count how many album headers appear before the scroll offset, so we
        // index album_for_header correctly for the visible window.
        let mut header_idx = display[..offset].iter().filter(|r| r.is_none()).count();

        let need_sb = total > visible;
        let render_w = area.width.saturating_sub(if need_sb { 1 } else { 0 }) as usize;
        let show_length = render_w > 30;
        let dur_w: usize = if show_length { 6 } else { 0 }; // "mm:ss" or "h:mm:ss"

        // Build visible ListItems and the row map simultaneously.
        self.power_queue_row_map.clear();
        let mut list_items: Vec<ListItem> = Vec::new();

        for entry in display.iter().skip(offset).take(visible) {
            match entry {
                None => {
                    let album = album_for_header.get(header_idx).map(|s| s.as_str()).unwrap_or("");
                    header_idx += 1;
                    let label = trunc_str(album, render_w.saturating_sub(1));
                    list_items.push(ListItem::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(label, Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)),
                    ])));
                    self.power_queue_row_map.push(None);
                }
                Some(i) => {
                    let i = *i;
                    let item = &items[i];
                    let is_active = i == active_idx && active;
                    let is_cursor = i == cursor && focused;

                    let fg = if is_active {
                        palette::WHITE
                    } else if is_cursor {
                        palette::YELLOW
                    } else if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let row_style = if is_active {
                        Style::default().fg(fg).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(fg)
                    };

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
                    // Otherwise use the standard playback label.
                    let label = if item.is_audio() {
                        if item.index_number > 0 {
                            format!("{:2}. {}", item.index_number, item.name)
                        } else {
                            item.name.clone()
                        }
                    } else {
                        item.playback_label()
                    };

                    let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                    let dur = if len_secs > 0 { fmt_duration(len_secs) } else { String::new() };

                    // Title truncated to leave room for duration + pct.
                    let extra = dur_w + pct_str.chars().count();
                    let title_w = render_w.saturating_sub(1 + extra); // 1 for marker
                    let title = trunc_str(&label, title_w);

                    let mut spans = vec![marker, Span::raw(title)];
                    if !pct_str.is_empty() {
                        spans.push(Span::styled(pct_str, Style::default().fg(palette::YELLOW)));
                    }
                    if show_length && !dur.is_empty() {
                        let dur_color = if is_active {
                            if focused { palette::SUBTLE } else { palette::MUTED }
                        } else {
                            palette::SUBTLE
                        };
                        // Right-align duration within render_w.
                        let used: usize = spans.iter().map(|s| s.content.chars().count()).sum();
                        let pad = render_w.saturating_sub(used + dur.chars().count());
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
        let cache_key = format!("{}:P", item_id);
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

        // Header: iris bar on top, panel name below.
        let area = lib_area;
        let header_name = if self.power_left_tab == 0 {
            "Continue Watching".to_string()
        } else {
            self.libs[self.power_left_tab - 1].library.name.clone()
        };
        let budget = (area.width as usize).saturating_sub(2);
        if area.height < 2 { return None; }
        let bar_y = area.y;
        let uline = "\u{2500}".repeat(area.width as usize);
        f.render_widget(
            Paragraph::new(Span::styled(uline, Style::default().fg(palette::IRIS))),
            Rect { x: area.x, y: bar_y, width: area.width, height: 1 },
        );
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(trunc_str(&header_name, budget),
                    Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)),
            ])),
            Rect { x: area.x, y: area.y + 1, width: area.width, height: 1 },
        );

        let content_area = Rect { y: area.y + 2, height: area.height.saturating_sub(2), ..area };
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
