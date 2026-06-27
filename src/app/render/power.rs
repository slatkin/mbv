use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState};
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

        self.render_power_left_panel(f, left_area, left_focused);
        self.render_power_queue(f, right_area, queue_focused);

        // Vertical divider.
        for y in area.y..area.y + area.height {
            f.render_widget(
                Paragraph::new(Span::styled("\u{2502}", Style::default().fg(palette::IRIS))),
                Rect { x: divider_x, y, width: 1, height: 1 },
            );
        }
    }

    

    fn render_power_queue(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height < 1 { return; }

        let list_area = area;

        if list_area.height == 0 { return; }

        let n = self.player_tab.items.len();
        if n == 0 {
            f.render_widget(
                Paragraph::new("  Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                list_area,
            );
            return;
        }

        // Reuse the presentation-view queue table, adjusted for our area.
        let table_area = list_area;
        self.power_queue_area = table_area;

        let (active, active_idx, live_pos, live_runtime, _) = self.effective_playback_state();
        let cursor = self.player_tab.playlist_cursor;
        let show_length = table_area.width > 50;
        let title_col_w = (table_area.width as usize).saturating_sub(if show_length { 10 } else { 0 });

        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let is_active = i == active_idx && active;
            let row_style = if is_active {
                Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD)
            } else if i == cursor && focused {
                Style::default().fg(palette::YELLOW)
            } else {
                Style::default().fg(if focused { palette::WHITE } else { palette::SUBTLE })
            };
            let (pt, rt) = if is_active {
                let pos = if live_pos > 0 { live_pos } else { item.playback_position_ticks };
                (pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            let pct_str = if pt > 0 && rt > 0 && !item.is_audio() {
                let pct = (pt * 100 / rt.max(1)) as u64;
                format!(" {pct}%")
            } else { String::new() };
            let marker = if i == cursor && focused {
                Span::styled("\u{258c}", Style::default().fg(palette::IRIS))
            } else {
                Span::raw(" ")
            };
            let max_title = title_col_w.saturating_sub(1 + pct_str.chars().count());
            let title = trunc_str(&item.playback_label(), max_title);
            let mut spans = vec![marker, Span::raw(title)];
            if !pct_str.is_empty() {
                spans.push(Span::styled(pct_str, Style::default().fg(palette::YELLOW)));
            }
            let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "\u{2014}".to_string() };
            Row::new([
                Cell::from(Line::from(spans)),
                Cell::from(Line::from(length).alignment(Alignment::Right))
                    .style(Style::default().fg(if is_active { if focused { palette::SUBTLE } else { palette::MUTED } } else { palette::SUBTLE })),
            ]).style(row_style)
        }).collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Min(10),
            Constraint::Length(if show_length { 9 } else { 0 }),
        ])
        .column_spacing(0)
        .row_highlight_style(Style::default());
        let visible = table_area.height as usize;
        let need_sb = n > visible;
        // Reserve 1 char on the right for the scrollbar so it doesn't overlap the length column.
        let render_area = if need_sb {
            Rect { width: table_area.width.saturating_sub(1), ..table_area }
        } else {
            table_area
        };
        f.render_stateful_widget(table, render_area, &mut state);

        if need_sb {
            let offset = state.offset();
            let max_off = n.saturating_sub(visible);
            let mut sb = ScrollbarState::new(max_off + 1).position(offset);
            let sb_area = Rect { x: table_area.x + table_area.width.saturating_sub(1), width: 1, ..table_area };
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

    fn render_power_left_panel(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height == 0 { return; }

        // Render card into the full area; it returns the rows it actually used.
        // The library panel then fills whatever vertical space remains.
        let card_h = self.render_power_card(f, area);
        let lib_area = Rect { y: area.y + card_h, height: area.height.saturating_sub(card_h), ..area };

        if lib_area.height == 0 { return; }

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
        if area.height < 2 { return; }
        let uline = "\u{2500}".repeat(area.width as usize);
        f.render_widget(
            Paragraph::new(Span::styled(uline, Style::default().fg(palette::IRIS))),
            Rect { x: area.x, y: area.y, width: area.width, height: 1 },
        );
        let header_fg = if focused { palette::WHITE } else { palette::SUBTLE };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled(trunc_str(&header_name, budget), Style::default().fg(header_fg)),
            ])),
            Rect { x: area.x, y: area.y + 1, width: area.width, height: 1 },
        );

        let content_area = Rect { y: area.y + 2, height: area.height.saturating_sub(2), ..area };
        if content_area.height == 0 { return; }

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
            return;
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
    }

    

    

    
}
