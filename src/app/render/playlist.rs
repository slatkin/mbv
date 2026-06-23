use crate::app::{PLAYLIST_VIEW_CARDS, PLAYLIST_VIEW_PRESENTATION};
use std::time::Duration;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Scrollbar,
    ScrollbarOrientation, ScrollbarState, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, palette};
use super::super::ui_util::{fmt_duration, trunc_str};

impl App {
    pub(super) fn render_playlist_bar(&self, f: &mut Frame, area: Rect) {
        self.render_playlist_bar_bg(f, area, palette::OVERLAY);
    }

    pub(super) fn render_playlist_bar_bg(&self, f: &mut Frame, area: Rect, bg: ratatui::style::Color) {
        let name = self.queue_playlist_name().to_string();
        let max_name = (area.width as usize).saturating_sub(12);
        let name_trunc = trunc_str(&name, max_name);
        let focused = bg != palette::OVERLAY;
        let label_fg = if focused { palette::OVERLAY } else { palette::SUBTLE };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(" "),
                Span::styled("Playlist: ", Style::default().fg(label_fg)),
                Span::styled(name_trunc, Style::default().fg(palette::WHITE)),
            ])).style(Style::default().bg(bg)),
            area,
        );
    }

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
            let is_playlist = self.queue_is_saved_playlist();
            let bar_h = if is_playlist { 1u16 } else { 0 };
            let inner = Rect {
                x: area.x,
                y: area.y + v_pad,
                width: area.width,
                height: area.height.saturating_sub(v_pad * 2 + bar_h),
            };
            self.layout_playlist_inner = inner;

            if is_playlist {
                self.render_playlist_bar(f, Rect {
                    y: area.y + area.height.saturating_sub(1),
                    height: 1,
                    x: area.x,
                    width: area.width,
                });
            }

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

        if self.playlist_view == PLAYLIST_VIEW_PRESENTATION {
            self.layout_playlist_inner = area;

            if self.player_tab.items.is_empty() {
                f.render_widget(
                    Paragraph::new("Add items with p from Home or library tabs")
                        .style(Style::default().fg(palette::MUTED)),
                    area,
                );
                return;
            }

            self.render_playlist_presentation(f, area);
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
        let is_playlist = self.queue_is_saved_playlist();
        let bar_h = if is_playlist { 1u16 } else { 0 };
        let table_area = Rect { height: inner.height.saturating_sub(bar_h), ..inner };
        if is_playlist {
            self.render_playlist_bar(f, Rect {
                y: inner.y + table_area.height,
                height: 1,
                ..inner
            });
        }
        let show_ep_cols = self.player_tab.items.iter().any(|it| it.item_type == "Episode");

        // Fixed column widths + 5 inter-column gaps of 1 = 5 overhead
        let title_col_width = (table_area.width as i32
            - if show_ep_cols { 32 } else { 24 }).max(0) as usize;

        let rows: Vec<Row> = self.player_tab.items.iter().enumerate().map(|(i, item)| {
            let row_style = if i == current_idx && active {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
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
            let media_type_str = if !item.item_type.is_empty() { item.item_type.clone() } else { "—".to_string() };
            let (pos_ticks, rt_ticks) = if i == current_idx && active {
                let pos = if live_pos > 0 { live_pos } else { item.playback_position_ticks };
                (pos, live_runtime)
            } else {
                (item.playback_position_ticks, item.runtime_ticks)
            };
            let title_cell = if pos_ticks > 0 && rt_ticks > 0 && !item.is_audio() {
                let pct = (pos_ticks * 100 / rt_ticks.max(1)) as u64;
                let pct_str = format!(" {pct}%");
                let max_title = title_col_width.saturating_sub(pct_str.chars().count());
                Cell::from(Line::from(vec![
                    Span::raw(trunc_str(&title, max_title)),
                    Span::styled(pct_str, Style::default().fg(palette::YELLOW)),
                ]))
            } else {
                Cell::from(trunc_str(&title, title_col_width))
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
                    Cell::from(Line::from(media_type_str).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(""),
                ]).style(row_style)
            } else {
                Row::new([
                    indicator,
                    title_cell,
                    Cell::from(""),
                    Cell::from(Line::from(length).alignment(Alignment::Right)),
                    Cell::from(Line::from(media_type_str).alignment(Alignment::Right)).style(Style::default().fg(palette::SUBTLE)),
                    Cell::from(""),
                ]).style(row_style)
            }
        }).collect();

        let header_style = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
        let header = Row::new([
            Cell::from(""),
            Cell::from("Title").style(header_style),
            Cell::from(""),
            Cell::from(Line::from("Length").alignment(Alignment::Right)).style(header_style),
            Cell::from(Line::from("Type").alignment(Alignment::Right)).style(header_style),
            Cell::from(""),
        ]);

        let mut state = TableState::default();
        state.select(Some(cursor));
        let table = Table::new(rows, [
            Constraint::Length(1),
            Constraint::Min(10),
            Constraint::Length(if show_ep_cols { 8 } else { 0 }),
            Constraint::Length(7),
            Constraint::Length(10),
            Constraint::Length(1),
        ])
        .header(header)
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

    fn render_playlist_presentation(&mut self, f: &mut Frame, area: Rect) {
        let n = self.player_tab.items.len();
        if n == 0 { return; }

        let (active, active_idx, live_pos, live_runtime, live_paused) = self.effective_playback_state();
        let cursor = self.player_tab.playlist_cursor;

        let left_w = ((area.width as u32 * 2 / 5) as u16).clamp(20, 60);
        let right_x = area.x + left_w + 1;
        let right_w = area.width.saturating_sub(left_w + 1);
        let is_playlist = self.queue_is_saved_playlist();
        let bar_h = if is_playlist { 1u16 } else { 0 };
        let left_area  = Rect { x: area.x,  y: area.y, width: left_w,  height: area.height };
        let right_area = Rect { x: right_x, y: area.y, width: right_w, height: area.height.saturating_sub(bar_h) };
        if is_playlist {
            self.render_playlist_bar(f, Rect {
                x: right_x, y: area.y + right_area.height, width: right_w, height: 1,
            });
        }

        let show_controls = active || self.connected_session_id.is_some();

        let card_area = if show_controls {
            Rect { height: left_area.height.saturating_sub(5), ..left_area }
        } else {
            left_area
        };

        let text_bottom = {
            let (item_id, series_id, now_playing, pos_ticks, rt_ticks, played, img_types,
                 card_name, card_series, card_ep_tag, stack_subs) = {
                let item = &self.player_tab.items[cursor];
                let now_playing = active && active_idx == cursor;
                let rt_ticks = if now_playing { live_runtime } else { item.runtime_ticks };
                let pos_ticks = if show_controls { 0 } else if now_playing { live_pos } else { item.playback_position_ticks };
                let rt_ticks = if item.is_audio() { 0 } else { rt_ticks };
                let img_types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio"      => &["Primary"],
                    "Movie"      => &["Backdrop", "Primary", "Logo"],
                    _            => &["Primary", "Backdrop", "Logo"],
                };
                let (card_name, card_series, card_ep_tag, stack_subs) =
                    if item.item_type == "Episode" && !item.series_name.is_empty() {
                        let ep_tag = format!("Series {:02} Episode {:02}",
                            item.parent_index_number, item.index_number);
                        (item.series_name.clone(), item.name.clone(), ep_tag, true)
                    } else if item.is_audio() && !item.artist.is_empty() {
                        (item.artist.clone(), item.name.clone(), item.album.clone(), true)
                    } else {
                        (item.playback_label(), String::new(), String::new(), false)
                    };
                (item.id.clone(), item.series_id.clone(), now_playing,
                 pos_ticks, rt_ticks, item.played, img_types,
                 card_name, card_series, card_ep_tag, stack_subs)
            };
            let cache_key = format!("{}:A", item_id);
            let is_music_item = matches!(img_types, &["Primary"] | &["AudioChild"]);
            if self.images_enabled() || is_music_item {
                self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
            }
            self.render_card_slot(f, card_area, true, true, now_playing, true, true, true,
                &cache_key, &card_name, &card_series, &card_ep_tag, 0, pos_ticks, rt_ticks, played,
                None, None, stack_subs).unwrap_or(left_area.bottom().saturating_sub(3))
        };
        let title_row   = text_bottom + 2;
        let seekbar_row = text_bottom + 3;
        let buttons_row = text_bottom + 4;

        if show_controls && title_row < left_area.bottom() {
            let playing_title = if let Some(ref remote) = self.connected_session_state {
                remote.now_playing.clone().unwrap_or_default()
            } else {
                self.player_tab.items.get(active_idx)
                    .map(|it| it.name.clone())
                    .unwrap_or_default()
            };
            if !playing_title.is_empty() {
                let title_trunc = trunc_str(&playing_title, left_area.width as usize);
                f.render_widget(Paragraph::new(Line::from(
                    Span::styled(title_trunc, Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)),
                )).alignment(Alignment::Center),
                Rect { x: left_area.x, y: title_row, width: left_area.width, height: 1 });
            }
        }

        if show_controls && live_pos > 0 && live_runtime > 0 && seekbar_row < left_area.bottom() {
            let full_w  = left_area.width as usize;
            let bar_w   = (full_w as u32 * 3 / 5) as usize;
            let pad     = (full_w.saturating_sub(bar_w)) / 2;
            let fraction = (live_pos as f64 / live_runtime as f64).clamp(0.0, 1.0);
            let filled  = ((fraction * bar_w as f64).round() as usize).min(bar_w);
            let bar_x   = left_area.x + pad as u16;
            let bar_end = bar_x + bar_w as u16;
            f.render_widget(Paragraph::new(Line::from(vec![
                Span::raw(" ".repeat(pad)),
                Span::styled("━".repeat(filled),         Style::default().fg(palette::IRIS)),
                Span::styled("─".repeat(bar_w - filled), Style::default().fg(palette::IRIS_DIM)),
            ])), Rect { x: left_area.x, y: seekbar_row, width: left_area.width, height: 1 });
            let elapsed_str = fmt_duration(live_pos / crate::api::TICKS_PER_SECOND);
            let total_str   = fmt_duration(live_runtime / crate::api::TICKS_PER_SECOND);
            let elapsed_w   = elapsed_str.chars().count() as u16;
            let total_w     = total_str.chars().count() as u16;
            let time_style  = Style::default().fg(palette::MUTED);
            let elapsed_x   = bar_x.saturating_sub(elapsed_w + 1).max(left_area.x);
            f.render_widget(Paragraph::new(Span::styled(elapsed_str, time_style)),
                Rect { x: elapsed_x, y: seekbar_row, width: elapsed_w, height: 1 });
            let total_x = bar_end + 1;
            if total_x + total_w <= left_area.x + left_area.width {
                f.render_widget(Paragraph::new(Span::styled(total_str, time_style)),
                    Rect { x: total_x, y: seekbar_row, width: total_w, height: 1 });
            }
            self.layout_seekbar_area = Rect { x: bar_x, y: seekbar_row, width: bar_w as u16, height: 1 };
        } else {
            self.layout_seekbar_area = Rect::default();
        }

        if show_controls && buttons_row < left_area.bottom() && self.use_nerd_fonts {
            let paused = live_paused;
            let btn_style = Style::default().fg(Color::Rgb(203, 212, 241));
            let pp_icon = if !paused { "\u{F03E4}" } else { "\u{F040A}" };
            let btn_icons = ["\u{F04AE}", "\u{F04A}", pp_icon, "\u{F04DB}", "\u{F04E}", "\u{F04AD}"];
            let mut btn_spans: Vec<Span> = Vec::new();
            for icon in btn_icons.iter() {
                btn_spans.push(Span::styled(format!("  {icon}  "), btn_style));
            }
            const BTNS_W: u16 = 30;
            let btn_x = left_area.x + left_area.width.saturating_sub(BTNS_W) / 2;
            f.render_widget(
                Paragraph::new(Line::from(btn_spans)).alignment(Alignment::Center),
                Rect { x: left_area.x, y: buttons_row, width: left_area.width, height: 1 },
            );
            self.layout_button_area = Rect { x: btn_x, y: buttons_row, width: BTNS_W, height: 1 };
        } else {
            self.layout_button_area = Rect::default();
        }
        self.layout_tracks_area  = Rect::default();
        self.layout_vol_area     = Rect::default();
        self.layout_sub_area     = Rect::default();
        self.layout_audio_area   = Rect::default();

        // Prefetch images for nearby items.
        {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end   = (cursor + 3).min(n.saturating_sub(1));
            for pi in prefetch_start..=prefetch_end {
                if pi == cursor { continue; }
                let item = &self.player_tab.items[pi];
                let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
                let img_types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio"      => &["Primary"],
                    "Movie"      => &["Backdrop", "Primary", "Logo"],
                    _            => &["Primary", "Backdrop", "Logo"],
                };
                let is_music = matches!(item.item_type.as_str(), "Audio" | "MusicAlbum");
                if self.images_enabled() || is_music {
                    self.fetch_card_image(format!("{}:A", item_id), item_id, series_id, img_types);
                }
            }
        }

        self.layout_playlist_inner = right_area;

        let show_length = area.width > 80;
        let title_col_w = (right_w as usize).saturating_sub(if show_length { 10 } else { 0 });

        enum PRow {
            Header(String),
            Item {
                label:     String,
                pos_ticks: i64,
                rt_ticks:  i64,
                is_audio:  bool,
                is_active: bool,
                is_cursor: bool,
                in_group:  bool,
                length:    String,
            },
        }

        let mut prows: Vec<PRow> = Vec::with_capacity(n + 4);
        let mut visual_cursor: usize = 0;

        {
            let items = &self.player_tab.items;
            let mut i = 0usize;
            while i < n {
                let item = &items[i];
                let is_episode = item.item_type == "Episode" && !item.series_name.is_empty();
                let is_audio   = item.is_audio() && !item.album_id.is_empty();
                let run_end = if is_episode {
                    let series = &item.series_name;
                    let mut j = i + 1;
                    while j < n && items[j].item_type == "Episode" && &items[j].series_name == series {
                        j += 1;
                    }
                    j
                } else if is_audio {
                    let aid = &item.album_id;
                    let mut j = i + 1;
                    while j < n && items[j].is_audio() && &items[j].album_id == aid {
                        j += 1;
                    }
                    j
                } else {
                    i + 1
                };
                let use_group = (is_episode || is_audio) && (run_end - i) > 2;
                if use_group {
                    let header = if is_audio {
                        if item.artist.is_empty() {
                            item.album.clone()
                        } else {
                            format!("{} - {}", item.artist, item.album)
                        }
                    } else {
                        item.series_name.clone()
                    };
                    prows.push(PRow::Header(header));
                }
                for j in i..run_end {
                    let it = &items[j];
                    let (pt, rt) = if j == active_idx && active {
                        let pos = if live_pos > 0 { live_pos } else { it.playback_position_ticks };
                        (pos, live_runtime)
                    } else {
                        (it.playback_position_ticks, it.runtime_ticks)
                    };
                    let label = if use_group { it.name.clone() } else { it.playback_label() };
                    let len_secs = it.runtime_ticks / TICKS_PER_SECOND;
                    let length = if len_secs > 0 { fmt_duration(len_secs) } else { "—".to_string() };
                    if j == cursor { visual_cursor = prows.len(); }
                    prows.push(PRow::Item {
                        label,
                        pos_ticks: pt,
                        rt_ticks:  rt,
                        is_audio:  it.is_audio(),
                        is_active: j == active_idx && active,
                        is_cursor: j == cursor,
                        in_group:  use_group,
                        length,
                    });
                }
                i = run_end;
            }
        }

        let rows: Vec<Row> = prows.iter().map(|prow| match prow {
            PRow::Header(series) => {
                let name = trunc_str(series, title_col_w.saturating_sub(2));
                let dash_w = title_col_w.saturating_sub(name.chars().count() + 1);
                let line_style = Style::default().fg(palette::IRIS);
                let title_cell = Cell::from(Line::from(vec![
                    Span::styled(name, Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                    Span::styled(" ", Style::default()),
                    Span::styled("─".repeat(dash_w), line_style),
                ]));
                let len_cell = Cell::from(
                    Line::from("─".repeat(9)).alignment(Alignment::Right)
                ).style(line_style);
                let trail_cell = Cell::from(Span::styled("─", line_style));
                Row::new([title_cell, len_cell, trail_cell])
            }
            PRow::Item { label, pos_ticks, rt_ticks, is_audio, is_active, is_cursor, in_group, length } => {
                let row_style = if *is_active {
                    Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(palette::WHITE)
                };
                let pct_str = if *pos_ticks > 0 && *rt_ticks > 0 && !*is_audio {
                    let pct = (*pos_ticks * 100 / (*rt_ticks).max(1)) as u64;
                    format!(" {pct}%")
                } else {
                    String::new()
                };
                let indent_w = if *in_group { 2usize } else { 0 };
                let max_title = title_col_w.saturating_sub(1 + indent_w + pct_str.chars().count());
                let title = trunc_str(label, max_title);
                let marker = if *is_cursor {
                    Span::styled("▌", Style::default().fg(palette::IRIS))
                } else {
                    Span::raw(" ")
                };
                let mut spans = vec![];
                if *in_group { spans.push(Span::raw("  ")); }
                spans.push(marker);
                spans.push(Span::raw(title));
                if !pct_str.is_empty() {
                    spans.push(Span::styled(pct_str, Style::default().fg(palette::YELLOW)));
                }
                Row::new([
                    Cell::from(Line::from(spans)),
                    Cell::from(Line::from(length.as_str()).alignment(Alignment::Right))
                        .style(Style::default().fg(if *is_active { palette::FOAM } else { palette::SUBTLE })),
                ]).style(row_style)
            }
        }).collect();

        let mut state = TableState::default();
        state.select(Some(visual_cursor));
        let table = Table::new(rows, [
            Constraint::Min(10),
            Constraint::Length(if show_length { 9 } else { 0 }),
            Constraint::Length(1),
        ])
        .column_spacing(0)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, right_area, &mut state);
        self.layout_presentation_scroll = state.offset();
        self.layout_presentation_visual_cursor = visual_cursor;

        let total_rows = prows.len();
        let visible_rows = right_area.height as usize;
        if total_rows > visible_rows {
            let max_offset = total_rows.saturating_sub(visible_rows);
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(state.offset());
            self.layout_presentation_sb = Rect {
                x: right_area.x + right_area.width.saturating_sub(1),
                y: right_area.y,
                width: 1,
                height: right_area.height,
            };
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                right_area,
                &mut sb_state,
            );
        } else {
            self.layout_presentation_sb = Rect::default();
        }
    }
}
