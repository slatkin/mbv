use crate::app::PLAYLIST_VIEW_CARDS;
use std::time::Duration;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, palette};
use super::super::ui_util::{fmt_duration, trunc_str};

impl App {
    pub(super) fn render_combined(&mut self, f: &mut Frame, area: Rect) {
        self.home_rect = area;
        self.layout_carousel_left_arrow = None;
        self.layout_carousel_right_arrow = None;
        self.layout_carousel_up_arrow = None;
        self.layout_carousel_down_arrow = None;
        self.layout_home_card_strips.clear();
        if self.home_search.is_some() {
            self.render_home_search(f, area);
        } else if self.home_card_view {
            self.render_home_cards(f, area);
        } else {
            self.render_home_panel(f, area);
        }
    }

    pub(super) fn render_playlist_panel(&mut self, f: &mut Frame, area: Rect) {
        if self.home_search.is_some() {
            self.render_home_search(f, area);
            return;
        }
        let (active, current_idx, live_pos, live_runtime, _live_paused) = self.effective_playback_state();

        self.playlist_rect = area;

        if self.playlist_view == PLAYLIST_VIEW_CARDS {
            let v_pad: u16 = if area.height >= 30 { 2 } else if area.height >= 20 { 1 } else { 0 };
            let inner = Rect {
                x: area.x,
                y: area.y + v_pad,
                width: area.width,
                height: area.height.saturating_sub(v_pad * 2),
            };
            self.layout_playlist_inner = inner;

            if self.player_tab.items.is_empty() {
                f.render_widget(
                    Paragraph::new("Add items with p from Home or library tabs")
                        .style(Style::default().fg(palette::MUTED)),
                    inner,
                );
                return;
            }

            self.render_playlist_cards(f, inner);
            return;
        }

        let inner = area;
        self.layout_playlist_inner = inner;

        if self.player_tab.items.is_empty() {
            f.render_widget(
                Paragraph::new("Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                inner,
            );
            return;
        }

        let cursor = self.player_tab.playlist_cursor;
        // The list occupies 90% of the available width, centered.
        let table_w = inner.width * 90 / 100;
        let table_area = Rect { x: inner.x + (inner.width - table_w) / 2, width: table_w, ..inner };
        let show_ep_cols = self.player_tab.items.iter().any(|it| it.item_type == "Episode");

        // Fixed column widths + inter-column gaps of 1.
        let title_col_width = (table_area.width as i32
            - if show_ep_cols { 21 } else { 13 }).max(0) as usize;

        const SPINNER_FRAMES: &[&str] = &["⠋","⠙","⠹","⠸","⠼","⠴","⠦","⠧","⠇","⠏"];
        let spinner_char: &str = {
            let ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis();
            SPINNER_FRAMES[(ms / 150) as usize % SPINNER_FRAMES.len()]
        };

        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let now_playing = i == current_idx && active;
            let row_style = if i == cursor {
                Style::default().fg(palette::YELLOW)
            } else {
                Style::default().fg(palette::WHITE)
            };

            let indicator = if i == cursor {
                Cell::from("▐").style(Style::default().fg(palette::IRIS))
            } else {
                Cell::from(" ")
            };

            let title = item.playback_label();
            let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "—".to_string() };
            let (pos_ticks, rt_ticks) = if i == current_idx && active {
                let pos = if live_pos > 0 { live_pos } else { item.playback_position_ticks };
                (pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            // Spinner prefix "⠋ " costs 2 chars when now-playing.
            let spin_w: usize = if now_playing { 2 } else { 0 };
            // Now-playing title text is emby blue (not bold); others inherit row_style.
            let title_span_style = if now_playing {
                Style::default().fg(palette::FOAM)
            } else {
                Style::default()
            };
            let title_cell = if pos_ticks > 0 && rt_ticks > 0 && !item.is_audio() {
                let pct = (pos_ticks * 100 / rt_ticks.max(1)) as u64;
                // Now-playing progress is green; other in-progress rows are yellow.
                let pct_style = if now_playing { palette::IRIS } else { palette::YELLOW };
                let pct_str = format!(" {pct}%");
                let max_title = title_col_width.saturating_sub(pct_str.chars().count() + spin_w);
                let mut spans: Vec<Span> = if now_playing {
                    vec![Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)), Span::raw(" ")]
                } else { vec![] };
                spans.push(Span::styled(trunc_str(&title, max_title), title_span_style));
                spans.push(Span::styled(pct_str, Style::default().fg(pct_style)));
                Cell::from(Line::from(spans))
            } else {
                let max_title = title_col_width.saturating_sub(spin_w);
                let mut spans: Vec<Span> = if now_playing {
                    vec![Span::styled(spinner_char.to_string(), Style::default().fg(palette::IRIS)), Span::raw(" ")]
                } else { vec![] };
                spans.push(Span::styled(trunc_str(&title, max_title), title_span_style));
                Cell::from(Line::from(spans))
            };

            if show_ep_cols {
                let ep_tag = if item.item_type == "Episode" && item.parent_index_number > 0 {
                    format!("S{:02}/E{:02}", item.parent_index_number, item.index_number)
                } else { String::new() };
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(Line::from(ep_tag).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(""),
                ]).style(row_style)
            } else {
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(""),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(""),
                ]).style(row_style)
            }
        }).collect();

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(if show_ep_cols { 8 } else { 0 }),
            Constraint::Length(7),
            Constraint::Length(1),
        ])
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);
    }

    fn render_playlist_cards(&mut self, f: &mut Frame, area: Rect) {
        self.layout_carousel_left_arrow = None;
        self.layout_carousel_right_arrow = None;
        self.layout_queue_strip_slots.clear();
        let n = self.player_tab.items.len();
        if n == 0 { return; }
        let expected_max = n * 2;
        if self.card_image_states.len() > expected_max + 10 {
            self.evict_card_images();
        }

        let cursor = self.player_tab.playlist_cursor;
        let (active, active_idx, ..) = self.effective_playback_state();

        // Filmstrip layout: large active card on top + scrolling strip below.
        const FILMSTRIP_STRIP_H: u16 = 10;
        if self.terminal_height > 34 {
            self.render_playlist_filmstrip(f, area, cursor, active, active_idx, FILMSTRIP_STRIP_H);
            return;
        }

        let cards_h = area.height;
        let compact      = self.terminal_height < 28;
        let max_h        = if cards_h < 12 { cards_h } else { ((cards_h as u32 * 24 / 25) as u16).min(24) }.max(4);
        let side_h       = ((max_h as u32 * 4 / 5) as u16).max(3);
        let center_h     = if compact { side_h } else { side_h + 2 };
        let center_v_pad = (cards_h.saturating_sub(center_h)) / 2;
        let side_v_pad   = center_v_pad + (center_h.saturating_sub(side_h)) / 2;

        const SIDE_HIDE_W: u16 = 60;
        let show_sides = area.width >= SIDE_HIDE_W;

        const GAP: u16 = 1;
        let (center_w, side_w, x_left, x_center, x_right) = if show_sides {
            let avail_w  = area.width.saturating_sub(GAP * 4 + 4);
            let cw = (avail_w as u32 * 2 / 5) as u16;
            let sw = avail_w.saturating_sub(cw) / 2;
            let xl = area.x + GAP + 2;
            let xc = xl + sw + GAP;
            let xr = xc + cw + GAP;
            (cw, sw, xl, xc, xr)
        } else {
            let avail_w = area.width.saturating_sub(GAP * 2);
            (avail_w, 0, area.x, area.x + GAP, area.x)
        };

        let slots: [(Option<usize>, Rect, bool); 3] = [
            (
                if show_sides && cursor > 0 { Some(cursor - 1) } else { None },
                Rect { x: x_left + 2, y: area.y + side_v_pad, width: side_w.saturating_sub(3), height: side_h },
                false,
            ),
            (
                Some(cursor),
                Rect { x: x_center, y: area.y + center_v_pad, width: center_w, height: center_h },
                true,
            ),
            (
                if show_sides && cursor + 1 < n { Some(cursor + 1) } else { None },
                Rect { x: x_right + 1, y: area.y + side_v_pad, width: side_w.saturating_sub(3), height: side_h },
                false,
            ),
        ];

        self.layout_carousel_slots = [
            (slots[0].0, slots[0].1),
            (slots[1].0, slots[1].1),
            (slots[2].0, slots[2].1),
        ];

        for (maybe_idx, card_rect, is_center) in &slots {
            let i = match maybe_idx { None => continue, Some(i) => *i };
            if card_rect.width < 3 { continue; }

            let (item_id, series_id, name, series, season, episode, runtime, is_ep,
                 pos_ticks, rt_ticks, played) = {
                let item = &self.player_tab.items[i];
                let is_ep = item.item_type == "Episode" && item.parent_index_number > 0;
                let (pos, rt) = if active && active_idx == i {
                    let s = self.player.status.lock().unwrap();
                    (s.position_ticks, s.runtime_ticks)
                } else {
                    (item.playback_position_ticks, item.runtime_ticks)
                };
                (item.id.clone(), item.series_id.clone(), item.name.clone(), item.series_name.clone(),
                 item.parent_index_number, item.index_number, item.runtime_ticks,
                 is_ep, pos, rt, item.played)
            };

            let selected    = i == cursor;
            let now_playing = active && active_idx == i;

            let (cache_key, img_types): (String, &[&str]) = if *is_center {
                (format!("{}:A", item_id), &["Primary", "Backdrop", "Logo"])
            } else {
                (format!("{}:S", item_id), &["Logo", "Primary", "Backdrop"])
            };
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
            }

            let ep_tag = if is_ep { format!("S{:02}E{:02}", season, episode) } else { String::new() };
            let count_label = if *is_center { Some(format!("{}/{}", cursor + 1, n)) } else { None };
            self.render_card_slot(f, *card_rect, *is_center, selected, now_playing, false, false, false,
                &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played,
                count_label.as_deref(), None, false);
        }

        // Prefetch images for items around the cursor, gated on nav idle.
        if self.images_enabled() && self.last_nav_at.elapsed() >= Duration::from_millis(150) {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end   = (cursor + 3).min(n.saturating_sub(1));
            for pi in prefetch_start..=prefetch_end {
                let (item_id, series_id) = {
                    let item = &self.player_tab.items[pi];
                    (item.id.clone(), item.series_id.clone())
                };
                self.fetch_card_image(format!("{}:A", item_id.clone()), item_id.clone(), series_id.clone(), &["Primary", "Backdrop", "Logo"]);
                if pi != cursor {
                    self.fetch_card_image(format!("{}:S", item_id), item_id, series_id, &["Logo", "Primary", "Backdrop"]);
                }
            }
        }

        let lr_arrow_style = Style::default().fg(palette::WHITE);
        let y_mid = area.y + center_v_pad + center_h / 2;
        if show_sides && cursor > 0 {
            let r = Rect { x: x_left, y: y_mid, width: 1, height: 1 };
            self.layout_carousel_left_arrow = Some(r);
            f.render_widget(Paragraph::new("◀").style(lr_arrow_style), r);
        }
        if show_sides && cursor + 1 < n {
            let r = Rect { x: x_right + side_w - 1, y: y_mid, width: 1, height: 1 };
            self.layout_carousel_right_arrow = Some(r);
            f.render_widget(Paragraph::new("▶").style(lr_arrow_style), r);
        }
    }

    fn render_playlist_filmstrip(
        &mut self, f: &mut Frame, area: Rect,
        cursor: usize, active: bool, active_idx: usize, strip_h: u16,
    ) {
        let n = self.player_tab.items.len();
        if n == 0 { return; }
        let cursor = cursor.min(n - 1);

        let large_h = area.height.saturating_sub(strip_h);
        let compact   = self.terminal_height < 28;
        let max_h     = if large_h < 12 { large_h } else { ((large_h as u32 * 24 / 25) as u16).min(24) }.max(4);
        let side_h_lg = ((max_h as u32 * 4 / 5) as u16).max(3);
        let center_h  = if compact { side_h_lg } else { side_h_lg + 2 };
        let block_h = center_h + strip_h;
        let top_pad = area.height.saturating_sub(block_h) / 2;
        let card_w    = (area.width as u32 * 3 / 5) as u16;
        let card_x    = area.x + (area.width - card_w) / 2;
        let large_rect = Rect { x: card_x, y: area.y + top_pad, width: card_w, height: center_h };

        self.layout_carousel_slots = [
            (None, Rect::default()),
            (Some(cursor), large_rect),
            (None, Rect::default()),
        ];

        let item = &self.player_tab.items[cursor];
        let is_ep = item.item_type == "Episode" && item.parent_index_number > 0;
        let ep_tag = if is_ep { format!("S{:02}E{:02}", item.parent_index_number, item.index_number) } else { String::new() };
        let name      = item.name.clone();
        let series    = item.series_name.clone();
        let runtime   = item.runtime_ticks;
        let item_id   = item.id.clone();
        let series_id = item.series_id.clone();
        let (pos_ticks, rt_ticks) = if active && active_idx == cursor {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks)
        } else {
            (item.playback_position_ticks, item.runtime_ticks)
        };
        let played      = item.played;
        let now_playing = active && active_idx == cursor;
        let cache_key   = format!("{}:A", item_id);
        if self.images_enabled() {
            self.fetch_card_image(cache_key.clone(), item_id.clone(), series_id.clone(),
                &["Primary", "Backdrop", "Logo"]);
        }
        let count_label = Some(format!("{}/{}", cursor + 1, n));
        self.render_card_slot(f, large_rect, true, true, now_playing, false, false, false,
            &cache_key, &name, &series, &ep_tag, runtime, pos_ticks, rt_ticks, played,
            count_label.as_deref(), None, false);

        let strip_y    = area.y + top_pad + center_h;
        let strip_zone = Rect { x: area.x, y: strip_y, width: area.width, height: strip_h };

        const GAP: u16 = 1;
        let avail_w   = area.width.saturating_sub(GAP * 4 + 4);
        let center_w  = (avail_w as u32 * 2 / 5) as u16;
        let side_w    = avail_w.saturating_sub(center_w) / 2;
        let item_w    = side_w.max(8);
        let item_step = item_w + GAP;
        let n_fit     = (area.width / item_step).max(1) as usize;

        let center_pos  = n_fit / 2;
        let strip_start = cursor as i64 - center_pos as i64;

        let total_strip_w = n_fit as u16 * item_step - GAP;
        let strip_x0      = area.x + area.width.saturating_sub(total_strip_w) / 2;

        let strip_card_h = strip_zone.height.saturating_sub(2).max(3);
        let strip_v_pad  = (strip_zone.height.saturating_sub(strip_card_h)) / 2;

        for i in 0..n_fit {
            let item_idx = strip_start + i as i64;
            if item_idx < 0 || item_idx >= n as i64 { continue; }
            let item_idx = item_idx as usize;
            let x    = strip_x0 + i as u16 * item_step;
            let rect = Rect { x, y: strip_zone.y + strip_v_pad, width: item_w, height: strip_card_h };
            self.layout_queue_strip_slots.push((item_idx, rect));

            let it = &self.player_tab.items[item_idx];
            let it_ep = it.item_type == "Episode" && it.parent_index_number > 0;
            let it_ep_tag = if it_ep { format!("S{:02}E{:02}", it.parent_index_number, it.index_number) } else { String::new() };
            let it_name   = it.name.clone();
            let it_series = it.series_name.clone();
            let it_rt     = it.runtime_ticks;
            let it_id     = it.id.clone();
            let it_sid    = it.series_id.clone();
            let it_pos    = it.playback_position_ticks;
            let it_played = it.played;

            if item_idx == cursor {
                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(palette::PINE));
                let inner = block.inner(rect);
                f.render_widget(block, rect);
                let it_ck = format!("{}:S", it_id);
                if self.images_enabled() {
                    self.fetch_card_image(it_ck.clone(), it_id, it_sid, &["Logo", "Primary", "Backdrop"]);
                }
                self.render_card_slot(f, inner, false, false, false, true, false, false,
                    &it_ck, &it_name, &it_series, &it_ep_tag, it_rt, it_pos, it_rt, it_played,
                    None, None, false);
            } else {
                let it_ck = format!("{}:S", it_id);
                if self.images_enabled() {
                    self.fetch_card_image(it_ck.clone(), it_id, it_sid, &["Logo", "Primary", "Backdrop"]);
                }
                self.render_card_slot(f, rect, false, false, false, false, false, false,
                    &it_ck, &it_name, &it_series, &it_ep_tag, it_rt, it_pos, it_rt, it_played,
                    None, None, false);
            }
        }

        if self.images_enabled() {
            let ps = cursor.saturating_sub(3);
            let pe = (cursor + 3 + n_fit).min(n.saturating_sub(1));
            for pi in ps..=pe {
                let (iid, sid) = { let it = &self.player_tab.items[pi]; (it.id.clone(), it.series_id.clone()) };
                self.fetch_card_image(format!("{}:A", iid.clone()), iid.clone(), sid.clone(), &["Primary", "Backdrop", "Logo"]);
                self.fetch_card_image(format!("{}:S", iid), iid, sid, &["Logo", "Primary", "Backdrop"]);
            }
        }

        let lr_style = Style::default().fg(palette::WHITE);
        let arrow_y  = strip_zone.y + strip_zone.height / 2;
        if cursor > 0 {
            let r = Rect { x: area.x, y: arrow_y, width: 1, height: 1 };
            self.layout_carousel_left_arrow = Some(r);
            f.render_widget(Paragraph::new("◀").style(lr_style), r);
        }
        if cursor + 1 < n {
            let r = Rect { x: area.x + area.width - 1, y: arrow_y, width: 1, height: 1 };
            self.layout_carousel_right_arrow = Some(r);
            f.render_widget(Paragraph::new("▶").style(lr_style), r);
        }
    }
}
