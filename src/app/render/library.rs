use std::time::Duration;
use textwrap::wrap;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Cell, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::App;
use super::super::palette;
use super::super::ui_util::{fmt_duration, trunc_overview, trunc_str};

impl App {
    pub(super) fn render_library(&mut self, f: &mut Frame, area: Rect, lib_idx: usize) {
        let is_loading = self.libs[lib_idx].nav_stack.last().map(|l| l.loading).unwrap_or(true);
        if is_loading && self.libs[lib_idx].search.is_none() {
            let block = Block::default()
                .borders(Borders::TOP).border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS));
            let inner = block.inner(area);
            f.render_widget(block, area);
            let mid = inner.y + inner.height / 2;
            let label_area = Rect { y: mid, height: 1, ..inner };
            f.render_widget(
                Paragraph::new("Loading...")
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(palette::MUTED)),
                label_area,
            );
            return;
        }

        let lib = &self.libs[lib_idx];
        let skip = if lib.nav_stack.first().map(|l| l.title == lib.library.name).unwrap_or(false) { 1 } else { 0 };
        let mut crumb_names: Vec<(String, usize)> = vec![(lib.library.name.clone(), 1)];
        for (i, lvl) in lib.nav_stack.iter().enumerate().skip(skip) {
            crumb_names.push((lvl.title.clone(), i + 1));
        }

        let sep = " \u{bb} ";
        let is_deep = crumb_names.len() > 1;

        let crumb_row = area.y;
        let mut x = area.x + 2;

        let crumb_style = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
        let mut crumb_spans: Vec<Span<'static>> = Vec::new();
        let mut new_breadcrumbs: Vec<(u16, u16, u16, usize)> = Vec::new();
        for (ci, (name, target_depth)) in crumb_names.iter().enumerate() {
            let is_last = ci + 1 == crumb_names.len();
            let w = name.chars().count() as u16;
            new_breadcrumbs.push((x, x + w, crumb_row, *target_depth));
            crumb_spans.push(Span::styled(name.clone(), crumb_style));
            x += w;
            if !is_last {
                crumb_spans.push(Span::styled(sep, crumb_style));
                x += sep.len() as u16;
            }
        }
        self.layout_breadcrumbs = if is_deep { new_breadcrumbs } else { Vec::new() };

        let mut block = Block::default()
            .borders(Borders::TOP).border_type(BorderType::Rounded)
            .border_style(Style::default().fg(palette::IRIS));
        let search_ref = self.libs[lib_idx].search.as_ref();
        if let Some(s) = search_ref {
            let label = if s.loading {
                format!("Search {} (loading…): {}█", self.libs[lib_idx].library.name, s.query)
            } else {
                format!("Search {}: {}█", self.libs[lib_idx].library.name, s.query)
            };
            let border_style = Style::default().fg(palette::IRIS);
            let text_style   = Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD);
            let title_spans = Line::from(vec![
                Span::styled("─", border_style),
                Span::raw(" "),
                Span::styled(label, text_style),
                Span::raw(" "),
            ]);
            block = block.title(title_spans);
        } else if is_deep {
            crumb_spans.insert(0, Span::raw(" "));
            crumb_spans.push(Span::raw(" "));
            block = block.title(Line::from(crumb_spans));
        }
        let inner = block.inner(area);
        f.render_widget(block, area);
        if self.is_album_level(lib_idx) && self.libs[lib_idx].search.is_none() {
            self.render_album_view(f, inner, lib_idx);
        } else {
            self.render_library_table(f, inner, lib_idx);
        }
    }

    pub(super) fn render_album_view(&mut self, f: &mut Frame, area: Rect, lib_idx: usize) {
        let (items, cursor, album_id) = {
            let lvl = match self.libs[lib_idx].nav_stack.last() { Some(l) => l, None => return };
            (lvl.items.clone(), lvl.cursor, lvl.parent_id.clone())
        };
        let n = items.len();
        if n == 0 {
            f.render_widget(Paragraph::new("  (empty)").style(Style::default().fg(palette::MUTED)), area);
            return;
        }

        let first = &items[0];
        let album_name = self.libs[lib_idx].nav_stack.last()
            .map(|l| l.title.clone()).unwrap_or_else(|| first.album.clone());
        let artist = first.artist.clone();
        let year = first.production_year;

        let left_w = ((area.width as u32 * 2 / 5) as u16).clamp(20, 60);
        let right_x = area.x + left_w + 1;
        let right_w = area.width.saturating_sub(left_w + 1);
        let left_area  = Rect { x: area.x,  y: area.y, width: left_w,  height: area.height };
        let right_area = Rect { x: right_x, y: area.y, width: right_w, height: area.height };

        let cache_key = format!("{}:lib", album_id);
        self.fetch_card_image(cache_key.clone(), album_id, String::new(), &["AudioChild", "Primary"]);
        let mut meta_parts: Vec<String> = Vec::new();
        if year > 0 { meta_parts.push(format!("{}", year)); }
        meta_parts.push(format!("{} tracks", n));
        let ep_tag = meta_parts.join("  ");
        self.render_card_slot(f, left_area, true, true, false, true, true, false,
            &cache_key, &album_name, &artist, &ep_tag, 0, 0, 0, false, None, None, true);

        let (active, active_idx, _, _, _) = self.effective_playback_state();
        let now_playing_id: Option<String> = if active {
            self.player_tab.items.get(active_idx).map(|i| i.id.clone())
        } else {
            None
        };

        let show_length = right_w > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (right_w as usize).saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items.iter().enumerate().map(|(i, item)| {
            let is_cursor = i == cursor;
            let is_playing = now_playing_id.as_deref() == Some(item.id.as_str());
            let row_style = if is_playing {
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::WHITE)
            };
            let marker = if is_cursor {
                Span::styled("▌", Style::default().fg(palette::IRIS))
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
            let length = if len_secs > 0 { fmt_duration(len_secs) } else { "—".to_string() };
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
        f.render_stateful_widget(table, right_area, &mut state);

        let total_rows = n;
        let visible_rows = right_area.height as usize;
        if total_rows > visible_rows {
            let max_offset = total_rows.saturating_sub(visible_rows);
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(state.offset());
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
        }
    }

    pub(super) fn render_season_grid(&mut self, f: &mut Frame, area: Rect, lib_idx: usize) {
        const COLS: usize = 4;
        const TEXT_ROWS: u16 = 3;
        const H_GAP: u16 = 2;
        const V_GAP: u16 = 1;

        let (items, cursor) = {
            let lvl = match self.libs[lib_idx].nav_stack.last() { Some(l) => l, None => return };
            (lvl.items.clone(), lvl.cursor)
        };
        let n = items.len();

        if n == 0 {
            f.render_widget(Paragraph::new("  (empty)").style(Style::default().fg(palette::MUTED)), area);
            return;
        }

        let total_rows = (n + COLS - 1) / COLS;
        let scrollbar_w: u16 = 1;

        let cell_w = area.width.saturating_sub(scrollbar_w + H_GAP * (COLS as u16 - 1)) / COLS as u16;

        let img_h: u16 = self.image_picker.as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((cell_w as f32 * fs.width as f32 * 1.5) / fs.height as f32).floor() as u16
            })
            .unwrap_or(8)
            .min(8);

        let cell_h = img_h + TEXT_ROWS;
        let cell_step_h = cell_h + V_GAP;
        let n_visible_rows = 2;

        let cursor_row = cursor / COLS;
        let scroll_row = {
            let prev = self.layout_lib_scroll;
            let s = prev.min(cursor_row).max(cursor_row.saturating_sub(n_visible_rows - 1));
            self.layout_lib_scroll = s;
            s
        };

        let images_enabled = self.images_enabled();

        if images_enabled {
            let first = scroll_row * COLS;
            let last = ((scroll_row + n_visible_rows) * COLS).min(n);
            let ids: Vec<String> = items[first..last].iter().map(|i| i.id.clone()).collect();
            for id in ids {
                let key = format!("{}:lib", id);
                self.fetch_card_image(key, id, String::new(), &["Primary"]);
            }
        }

        let total_grid_w = COLS as u16 * cell_w + (COLS as u16 - 1) * H_GAP;
        let x_off = area.x + area.width.saturating_sub(scrollbar_w + total_grid_w) / 2;

        let total_grid_h = n_visible_rows as u16 * cell_step_h;
        let y_off = area.y + area.height.saturating_sub(total_grid_h) / 2;

        for row in 0..n_visible_rows {
            let abs_row = scroll_row + row;
            if abs_row >= total_rows { break; }
            let row_y = y_off + row as u16 * cell_step_h;
            if row_y >= area.y + area.height { break; }

            for col in 0..COLS {
                let idx = abs_row * COLS + col;
                if idx >= n { break; }
                let item = &items[idx];
                let selected = idx == cursor;
                let cell_x = x_off + col as u16 * (cell_w + H_GAP);

                if images_enabled {
                    let key = format!("{}:lib", item.id);
                    let avail = ratatui::layout::Size { width: cell_w, height: img_h };
                    let actual = self.card_image_states.get_mut(&key)
                        .and_then(|s| s.as_mut())
                        .map(|s| s.size_for(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)), avail));
                    if let Some(actual) = actual {
                        let ix = cell_x + (cell_w.saturating_sub(actual.width)) / 2;
                        let iy = row_y + (img_h.saturating_sub(actual.height)) / 2;
                        let img_rect = Rect { x: ix, y: iy, width: actual.width, height: actual.height.min((area.y + area.height).saturating_sub(iy)) };
                        if let Some(Some(state)) = self.card_image_states.get_mut(&key) {
                            type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                            f.render_stateful_widget(SImg::default().resize(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3))), img_rect, state);
                        }
                    }
                }

                let name_y = row_y + img_h + 1;
                if name_y < area.y + area.height {
                    let style = if selected {
                        Style::default().fg(palette::IRIS).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(palette::TEXT)
                    };
                    f.render_widget(
                        Paragraph::new(Span::styled(trunc_str(&item.name, cell_w as usize), style))
                            .alignment(Alignment::Center),
                        Rect { x: cell_x, y: name_y, width: cell_w, height: 1 },
                    );
                }

                let meta_y = row_y + img_h + 2;
                if meta_y < area.y + area.height {
                    let mut parts: Vec<String> = Vec::new();
                    if item.total_count > 0 { parts.push(format!("{} eps", item.total_count)); }
                    if item.production_year > 0 { parts.push(format!("{}", item.production_year)); }
                    let meta = trunc_str(&parts.join("  "), cell_w as usize);
                    f.render_widget(
                        Paragraph::new(Span::styled(meta, Style::default().fg(palette::SUBTLE)))
                            .alignment(Alignment::Center),
                        Rect { x: cell_x, y: meta_y, width: cell_w, height: 1 },
                    );
                }
            }
        }

        if total_rows > n_visible_rows {
            let mut state = ScrollbarState::new(total_rows).position(scroll_row);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area,
                &mut state,
            );
        }
    }

    pub(super) fn render_library_table(&mut self, f: &mut Frame, area: Rect, lib_idx: usize) {
        if self.is_viewing_season_grid(lib_idx) {
            self.render_season_grid(f, area, lib_idx);
            return;
        }
        self.layout_lib_table_area = area;
        const LIB_SELECTED_IMG_W: u16 = 32;
        const LIB_AUDIO_IMG_W: u16 = 12;
        let lib_audio_img_h: u16 = self.image_picker.as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_AUDIO_IMG_W as f32 * fs.width as f32) / fs.height as f32).ceil() as u16
            })
            .unwrap_or(12);
        #[allow(non_snake_case)]
        let LIB_AUDIO_IMG_H = lib_audio_img_h;

        let (display_items, cursor): (Vec<(usize, crate::api::MediaItem)>, usize) = {
            let lib = &self.libs[lib_idx];
            if let Some(s) = &lib.search {
                let items: Vec<(usize, crate::api::MediaItem)> = s.results.iter()
                    .filter_map(|&i| s.items.get(i).map(|item| (i, item.clone())))
                    .collect();
                (items, s.cursor)
            } else {
                let lvl = lib.nav_stack.last();
                let items: Vec<(usize, crate::api::MediaItem)> = lvl.map(|l| {
                    l.items.iter().enumerate().map(|(i, item)| (i, item.clone())).collect()
                }).unwrap_or_default();
                let cur = lvl.map(|l| l.cursor).unwrap_or(0);
                (items, cur)
            }
        };

        let items_len = display_items.len();
        if items_len == 0 {
            let loading = self.libs[lib_idx].nav_stack.last().map(|l| l.loading).unwrap_or(false);
            let msg = if loading { "  Loading..." }
                else if self.libs[lib_idx].search.is_some() { "  (no results)" }
                else { "  (empty)" };
            f.render_widget(Paragraph::new(msg).style(Style::default().fg(palette::MUTED)), area);
            return;
        }

        let images_enabled = self.images_enabled();
        let (lib_active, lib_active_idx, _, _, _) = self.effective_playback_state();
        let lib_now_playing_id: Option<String> = if lib_active {
            self.player_tab.items.get(lib_active_idx).map(|i| i.id.clone())
        } else {
            None
        };
        let lib_movie_img_h: u16 = self.image_picker.as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_SELECTED_IMG_W as f32 * fs.width as f32 * 1.5) / fs.height as f32).ceil() as u16
            })
            .unwrap_or(12)
            .min(12);
        #[allow(non_snake_case)]
        let LIB_SELECTED_IMG_H = lib_movie_img_h;
        const LIB_EPISODE_IMG_W: u16 = 40;
        let lib_episode_img_h: u16 = self.image_picker.as_ref()
            .map(|p| {
                let fs = p.font_size();
                ((LIB_EPISODE_IMG_W as f32 * fs.width as f32 * (9.0 / 16.0)) / fs.height as f32).ceil() as u16
            })
            .unwrap_or(9);
        #[allow(non_snake_case)]
        let LIB_EPISODE_IMG_H = lib_episode_img_h;
        let at_album_folders = self.is_viewing_album_folders(lib_idx) && self.libs[lib_idx].search.is_none();
        let is_homevideo_lib = matches!(self.libs[lib_idx].library.collection_type.as_str(), "homevideos" | "channels");

        let actual_sel_img_h: u16 = if images_enabled {
            if let Some((_, item)) = display_items.get(cursor) {
                let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
                let is_album_folder = at_album_folders && item.is_folder;
                if !is_audio && !is_album_folder {
                    let is_episode_like = item.item_type == "Episode" || (is_homevideo_lib && item.item_type == "Video");
                    let (img_w, img_h) = if is_episode_like {
                        (LIB_EPISODE_IMG_W, LIB_EPISODE_IMG_H)
                    } else {
                        (LIB_SELECTED_IMG_W, LIB_SELECTED_IMG_H)
                    };
                    let cache_key = format!("{}:lib", item.id);
                    if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                        let avail = ratatui::layout::Size { width: img_w, height: img_h };
                        state.size_for(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)), avail).height
                    } else {
                        img_h
                    }
                } else { 0 }
            } else { 0 }
        } else { 0 };

        let all_heights: Vec<u16> = display_items.iter().enumerate().map(|(i, (_, item))| {
            let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
            let base: u16 = if item.is_folder && item.item_type != "Series" && item.item_type != "Season" {
                if at_album_folders && i == cursor {
                    if images_enabled { LIB_AUDIO_IMG_H.max(3) } else { 3 }
                } else {
                    1
                }
            } else if is_audio {
                if i == cursor { LIB_AUDIO_IMG_H.max(3) } else { 3 }
            } else if images_enabled && i == cursor {
                let is_episode_like = item.item_type == "Episode" || (is_homevideo_lib && item.item_type == "Video");
                let (sel_img_w, sel_img_h) = if is_episode_like {
                    (LIB_EPISODE_IMG_W, LIB_EPISODE_IMG_H)
                } else {
                    (LIB_SELECTED_IMG_W, LIB_SELECTED_IMG_H)
                };
                let ew = area.width.saturating_sub(2 + sel_img_w) as usize;
                let overview = trunc_overview(&item.overview);
                let ov_lines = if overview.is_empty() { 0 }
                    else { wrap(&overview, ew.max(1)).len() as u16 };
                let dir_lines: u16 = if item.item_type == "Movie" && !item.director.is_empty() { 2 } else { 0 };
                let tech: u16 = match (item.video_info.is_empty(), item.audio_info.is_empty()) {
                    (true, true) => 0,
                    _ => 1,
                };
                let img_h_for_layout = if actual_sel_img_h > 0 { actual_sel_img_h } else { sel_img_h };
                (2 + tech + ov_lines + dir_lines).max(img_h_for_layout)
            } else { 2 };
            base + 1
        }).collect();

        let scroll = if self.libs[lib_idx].search.is_some() {
            let mut s = self.layout_lib_scroll.min(cursor);
            loop {
                let visible_h: u16 = all_heights[s..=cursor].iter().sum();
                if visible_h <= area.height { break; }
                s += 1;
            }
            s
        } else {
            cursor
        };
        self.layout_lib_scroll = scroll;

        if self.last_nav_at.elapsed() >= Duration::from_millis(150) {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end = (cursor + 3).min(display_items.len().saturating_sub(1));
            for pi in prefetch_start..=prefetch_end {
                if let Some((_, item)) = display_items.get(pi) {
                    let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
                    let is_album_folder = at_album_folders && item.is_folder;
                    if images_enabled || is_audio || is_album_folder {
                        let cache_key = format!("{}:lib", item.id);
                        let img_types: &[&str] = if is_album_folder { &["AudioChild", "Primary"] } else { &["Primary"] };
                        self.fetch_card_image(cache_key, item.id.clone(), String::new(), img_types);
                    }
                    if is_album_folder && item.production_year == 0 {
                        self.fetch_album_year(item.id.clone());
                    }
                }
            }
        }

        let total_h: u16 = all_heights.iter().sum();
        let needs_scrollbar = total_h > area.height;
        let sep_w = if needs_scrollbar { area.width.saturating_sub(1) } else { area.width };

        let mut row_y = area.y;
        let mut rendered_heights: Vec<u16> = Vec::new();
        for (vi, (_, item)) in display_items[scroll..].iter().enumerate() {
            if row_y >= area.y + area.height { break; }
            let abs_idx = scroll + vi;
            let row_h = all_heights[abs_idx].min(area.y + area.height - row_y);
            let selected = abs_idx == cursor;
            let is_audio = item.media_type == "Audio" || item.item_type == "Audio";
            let is_album_folder = at_album_folders && item.is_folder;
            let show_img = selected && (images_enabled || is_audio || is_album_folder);
            let row_w = if needs_scrollbar { area.width.saturating_sub(1) } else { area.width };
            let row_rect = Rect { x: area.x, y: row_y, width: row_w, height: row_h };

            let content_area = Rect { height: row_h.saturating_sub(1), ..row_rect };
            let padded_area = content_area;

            let cache_key = format!("{}:lib", item.id);
            let img_actual = if show_img {
                if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                    let (img_w, img_h) = if is_audio || is_album_folder {
                        (LIB_AUDIO_IMG_W, LIB_AUDIO_IMG_H)
                    } else if item.item_type == "Episode" || (is_homevideo_lib && item.item_type == "Video") {
                        (LIB_EPISODE_IMG_W, LIB_EPISODE_IMG_H)
                    } else {
                        (LIB_SELECTED_IMG_W, LIB_SELECTED_IMG_H)
                    };
                    let avail = ratatui::layout::Size { width: img_w, height: img_h.min(padded_area.height) };
                    Some(state.size_for(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3)), avail))
                } else { None }
            } else { None };

            let (ind_rect, text_rect, img_rect_opt) = if is_audio || is_album_folder {
                if let Some(actual) = img_actual {
                    let [a, b, _, c] = Layout::horizontal([
                        Constraint::Length(1),
                        Constraint::Length(actual.width),
                        Constraint::Length(1),
                        Constraint::Min(0),
                    ]).areas(padded_area);
                    let img_h = actual.height.min(b.height);
                    let v_off = b.height.saturating_sub(img_h) / 2;
                    let img_rect = Rect { y: b.y + v_off, height: img_h, ..b };
                    (a, c, Some(img_rect))
                } else {
                    let [a, c] = Layout::horizontal([
                        Constraint::Length(1),
                        Constraint::Min(0),
                    ]).areas(padded_area);
                    (a, c, None)
                }
            } else if let Some(actual) = img_actual {
                let [a, b, _, c] = Layout::horizontal([
                    Constraint::Length(1),
                    Constraint::Length(actual.width),
                    Constraint::Length(1),
                    Constraint::Min(0),
                ]).areas(padded_area);
                let img_rect = Rect { height: actual.height.min(b.height), ..b };
                (a, c, Some(img_rect))
            } else {
                let [a, c] = Layout::horizontal([
                    Constraint::Length(1),
                    Constraint::Min(0),
                ]).areas(padded_area);
                (a, c, None)
            };
            let content_w = text_rect.width as usize;

            let is_episode_like = item.item_type == "Episode" || (is_homevideo_lib && item.item_type == "Video");
            if selected && !is_album_folder && !matches!(item.item_type.as_str(), "Movie" | "Series" | "Season" | "Episode") && !is_episode_like {
                let bar: Vec<Line> = (0..ind_rect.height)
                    .map(|_| Line::from(Span::styled("▌", Style::default().fg(palette::IRIS))))
                    .collect();
                f.render_widget(Paragraph::new(bar), ind_rect);
            }

            if let Some(img_rect) = img_rect_opt {
                type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
                if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
                    f.render_stateful_widget(SImg::default().resize(ratatui_image::Resize::Fit(Some(ratatui_image::FilterType::Lanczos3))), img_rect, state);
                }
            }

            let is_now_playing = lib_now_playing_id.as_deref() == Some(item.id.as_str());
            let text_color = if is_now_playing { palette::FOAM }
                else if selected && (matches!(item.item_type.as_str(), "Movie" | "Series" | "Season" | "Episode") || is_episode_like) { palette::IRIS }
                else if selected { palette::WHITE }
                else { palette::TEXT };

            let title_line = match item.item_type.as_str() {
                "Episode" => {
                    let n = item.index_number;
                    if n > 0 { format!("{}. {}", n, item.name) } else { item.name.clone() }
                }
                "Series" => {
                    if item.total_count > 0 {
                        format!("{} ({}/{})", item.name, item.unplayed_item_count, item.total_count)
                    } else if item.unplayed_item_count > 0 {
                        format!("{} ({})", item.name, item.unplayed_item_count)
                    } else {
                        item.name.clone()
                    }
                }
                _ if item.is_folder && item.item_type != "Series" && item.item_type != "Season" => {
                    if is_album_folder {
                        item.name.clone()
                    } else if item.total_count > 0 {
                        format!("{} ({})", item.name, item.total_count)
                    } else {
                        item.name.clone()
                    }
                }
                _ => item.name.clone(),
            };
            let title_display = wrap(&title_line, content_w.max(1))
                .into_iter().next().map(|c| c.into_owned()).unwrap_or_default();

            let artist_line: Option<String> = if is_audio && !item.artist.is_empty() {
                Some(item.artist.clone())
            } else { None };

            let episode_meta_line = |item: &crate::api::MediaItem| -> Line<'static> {
                let mut spans: Vec<Span> = Vec::new();
                if item.played {
                    spans.push(Span::styled("\u{2713} ", Style::default().fg(palette::PINE)));
                }
                let mut parts: Vec<String> = Vec::new();
                if !item.premiere_date.is_empty() { parts.push(item.premiere_date.clone()); }
                if is_homevideo_lib && !item.date_added.is_empty() {
                    const MONTHS: [&str; 12] = ["January","February","March","April","May","June","July","August","September","October","November","December"];
                    let formatted = item.date_added.splitn(3, '-')
                        .collect::<Vec<_>>()
                        .as_slice()
                        .windows(3)
                        .next()
                        .and_then(|p| {
                            let y = p[0]; let d: u32 = p[2].parse().ok()?;
                            let m: usize = p[1].parse::<usize>().ok()?.checked_sub(1)?;
                            Some(format!("Added {} {}, {}", d, MONTHS.get(m)?, y))
                        })
                        .unwrap_or_else(|| item.date_added.clone());
                    parts.push(formatted);
                }
                let dur_s = item.runtime_ticks / TICKS_PER_SECOND;
                if dur_s > 0 {
                    let h = dur_s / 3600; let m = (dur_s % 3600) / 60;
                    parts.push(if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") });
                }
                if !parts.is_empty() {
                    spans.push(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)));
                }
                if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                    let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    spans.push(Span::styled(format!("  {pct}%"), Style::default().fg(palette::YELLOW)));
                }
                Line::from(spans)
            };
            let meta_line: Line = if is_episode_like && item.item_type != "Episode" {
                episode_meta_line(item)
            } else { match item.item_type.as_str() {
                "Series" => {
                    let year_str = if item.production_year > 0 && item.end_year > 0 && item.end_year != item.production_year {
                        format!("{} \u{2013} {}", item.production_year, item.end_year)
                    } else if item.production_year > 0 && item.end_year == 0 {
                        format!("{} \u{2013}", item.production_year)
                    } else if item.production_year > 0 {
                        format!("{}", item.production_year)
                    } else {
                        String::new()
                    };
                    Line::from(Span::styled(year_str, Style::default().fg(palette::SUBTLE)))
                }
                "Season" => {
                    let mut parts: Vec<String> = Vec::new();
                    if item.total_count > 0 { parts.push(format!("{} eps", item.total_count)); }
                    if item.production_year > 0 { parts.push(format!("{}", item.production_year)); }
                    Line::from(Span::styled(parts.join(" \u{b7} "), Style::default().fg(palette::SUBTLE)))
                }
                "Episode" => episode_meta_line(item),
                _ if item.is_folder && item.item_type != "Series" && item.item_type != "Season" => {
                    if is_album_folder {
                        let mut parts: Vec<String> = Vec::new();
                        let year = if item.production_year > 0 {
                            item.production_year
                        } else {
                            self.album_year_cache.get(&item.id).copied().unwrap_or(0)
                        };
                        if year > 0 { parts.push(format!("{}", year)); }
                        if item.total_count > 0 { parts.push(format!("{} tracks", item.total_count)); }
                        Line::from(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)))
                    } else if item.total_count > 0 {
                        Line::from(Span::styled(
                            format!("{} items", item.total_count),
                            Style::default().fg(palette::SUBTLE),
                        ))
                    } else {
                        Line::from(vec![])
                    }
                }
                _ => {
                    let mut spans: Vec<Span> = Vec::new();
                    if !is_audio && item.played {
                        spans.push(Span::styled("\u{2713} ", Style::default().fg(palette::PINE)));
                    }
                    let mut parts: Vec<String> = Vec::new();
                    if item.production_year > 0 { parts.push(format!("{}", item.production_year)); }
                    let dur_s = item.runtime_ticks / TICKS_PER_SECOND;
                    if dur_s > 0 {
                        let h = dur_s / 3600; let m = (dur_s % 3600) / 60;
                        parts.push(if h > 0 { format!("{h}h{m:02}m") } else { format!("{m}m") });
                    }
                    if is_audio && !item.container.is_empty() {
                        parts.push(item.container.to_uppercase());
                    }
                    if !parts.is_empty() {
                        spans.push(Span::styled(parts.join("  "), Style::default().fg(palette::SUBTLE)));
                    }
                    if !is_audio && item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                        let pct = (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                        spans.push(Span::styled(format!("  {pct}%"), Style::default().fg(palette::YELLOW)));
                    }
                    Line::from(spans)
                }
            }};

            let meta_line = if item.is_folder && item.item_type != "Series" && item.item_type != "Season" {
                meta_line
            } else {
                let type_str = if matches!(item.item_type.as_str(), "Movie" | "Series") {
                    if !item.genre.is_empty() { item.genre.clone() } else { String::new() }
                } else if item.item_type == "Episode" || is_episode_like {
                    String::new()
                } else if !item.item_type.is_empty() {
                    item.item_type.clone()
                } else {
                    "—".to_string()
                };
                if type_str.is_empty() {
                    meta_line
                } else {
                    let mut spans = vec![Span::styled(format!("{}  ", type_str), Style::default().fg(palette::SUBTLE))];
                    spans.extend(meta_line.spans);
                    Line::from(spans)
                }
            };

            let overview_lines: Vec<String> = if !is_audio && selected && images_enabled && !item.overview.is_empty() {
                let w = content_w.max(1);
                wrap(&trunc_overview(&item.overview), w).into_iter().map(|s| s.into_owned()).collect()
            } else { Vec::new() };
            let artist_extra: usize = if artist_line.is_some() { 1 } else { 0 };
            let is_generic_folder = item.is_folder && item.item_type != "Series" && item.item_type != "Season";
            let tech_line: String = if selected && !is_audio && !is_generic_folder {
                match (item.video_info.is_empty(), item.audio_info.is_empty()) {
                    (false, false) => format!("{}  {}", item.video_info, item.audio_info),
                    (false, true)  => item.video_info.clone(),
                    (true, false)  => item.audio_info.clone(),
                    (true, true)   => String::new(),
                }
            } else { String::new() };
            let tech_lines: usize = if tech_line.is_empty() { 0 } else { 1 };
            let base_lines = if is_album_folder { 2 } else if is_generic_folder { 1 } else { 2 + artist_extra + tech_lines };
            let dir_lines: usize = if selected && item.item_type == "Movie" && !item.director.is_empty() { 2 } else { 0 };
            let line_count = (base_lines + overview_lines.len() + dir_lines).min(text_rect.height as usize);
            if line_count == 0 { continue; }
            let v_offset = if is_audio && selected {
                (text_rect.height as usize).saturating_sub(line_count) / 2
            } else { 0 };
            let centered_text_rect = Rect {
                y: text_rect.y + v_offset as u16,
                height: text_rect.height.saturating_sub(v_offset as u16),
                ..text_rect
            };
            let constraints: Vec<Constraint> = (0..line_count).map(|_| Constraint::Length(1)).collect();
            let line_rects = Layout::vertical(constraints).split(centered_text_rect);

            f.render_widget(
                Paragraph::new(Line::from(Span::styled(title_display, {
                    let s = Style::default().fg(text_color);
                    if selected && (matches!(item.item_type.as_str(), "Movie" | "Series" | "Season" | "Episode") || is_episode_like) { s.add_modifier(Modifier::BOLD) } else { s }
                }))),
                line_rects[0],
            );
            if let Some(ref a) = artist_line {
                if line_count >= 2 {
                    f.render_widget(
                        Paragraph::new(Span::styled(a.as_str(), Style::default().fg(palette::SUBTLE))),
                        line_rects[1],
                    );
                }
                if line_count >= 3 {
                    f.render_widget(Paragraph::new(meta_line), line_rects[2]);
                }
            } else if line_count >= 2 {
                f.render_widget(Paragraph::new(meta_line), line_rects[1]);
            }
            if tech_lines > 0 {
                let ti = 2 + artist_extra;
                if ti < line_count {
                    f.render_widget(
                        Paragraph::new(Span::styled(tech_line.as_str(), Style::default().fg(palette::SUBTLE))),
                        line_rects[ti],
                    );
                }
            }
            for (j, ov_line) in overview_lines.iter().enumerate() {
                let idx = base_lines + j;
                if idx >= line_count { break; }
                f.render_widget(
                    Paragraph::new(Span::styled(ov_line.as_str(), Style::default().fg(palette::WHITE))),
                    line_rects[idx],
                );
            }
            if dir_lines > 0 {
                let dir_idx = base_lines + overview_lines.len() + 1;
                if dir_idx < line_count {
                    f.render_widget(
                        Paragraph::new(Line::from(vec![
                            Span::styled("Director: ", Style::default().fg(palette::SUBTLE)),
                            Span::styled(item.director.clone(), Style::default().fg(palette::TEXT)),
                        ])),
                        line_rects[dir_idx],
                    );
                }
            }
            let sep_y = row_y + row_h - 1;
            if sep_y < area.y + area.height {
                let sep_rect = Rect { x: area.x, y: sep_y, width: sep_w, height: 1 };
                let sep_str: String = "\u{2500}".repeat(sep_w as usize);
                f.render_widget(
                    Paragraph::new(Span::styled(sep_str, Style::default().fg(palette::MUTED))),
                    sep_rect,
                );
            }

            rendered_heights.push(row_h);
            row_y += row_h;
        }
        self.layout_lib_row_heights = rendered_heights;

        if needs_scrollbar {
            let mut sb_state = ScrollbarState::new(items_len).position(scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area,
                &mut sb_state,
            );
        }
    }
}
