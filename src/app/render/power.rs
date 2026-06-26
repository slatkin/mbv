use unicode_width::UnicodeWidthStr;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, List, ListItem, ListState, Paragraph, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, Table, TableState};
use crate::api::TICKS_PER_SECOND;
use super::super::{App, PowerFocus, palette};
use super::super::ui_util::{fmt_duration, item_text_and_style, trunc_str};

const MIN_COL_W: u16 = 35;

impl App {
    pub(super) fn render_power_view(&mut self, f: &mut Frame, area: Rect) {
        if area.height < 6 { return; }

        let min_queue_h: u16 = 8;
        let min_lib_h: u16 = 4;
        let top_max = area.height.saturating_sub(min_lib_h);
        let top_h = if top_max >= min_queue_h {
            let preferred = (area.height as u32 * 50 / 100) as u16;
            preferred.clamp(min_queue_h, top_max)
        } else {
            top_max
        };
        let bot_h = area.height.saturating_sub(top_h);
        // Top section is inset by 1 column on each side; bottom library panels use full width.
        let top_area = Rect { x: area.x, y: area.y, width: area.width, height: top_h };
        let bot_area = Rect { x: area.x, y: area.y + top_h, width: area.width, height: bot_h };

        // ── top section ──────────────────────────────────────────────────────
        let left_w = ((top_area.width as u32 * 2 / 5) as u16).clamp(20, 60);
        let right_w = top_area.width.saturating_sub(left_w + 1);
        let divider_x  = top_area.x + left_w;
        let right_area = Rect { x: divider_x + 1, y: top_area.y, width: right_w, height: top_h };
        // Indicator bar: render in the gap row above main_area, aligned to the queue panel only.
        let indicator_y = area.y.saturating_sub(1);
        let ind_area = Rect { x: right_area.x, y: indicator_y, width: right_area.width, height: 1 };
        self.render_indicator_bar(f, ind_area, true);
        // Card image: extend upward into the now-free indicator row for extra height.
        let left_area = Rect { x: top_area.x, y: indicator_y, width: left_w, height: top_h + 1 };

        let queue_focused = matches!(self.power_focus, PowerFocus::Queue);

        self.render_power_card(f, left_area);
        self.render_power_queue(f, right_area, queue_focused);

        // ── horizontal divider ───────────────────────────────────────────────
        let hdiv_fg = palette::IRIS;
        let hdiv_str = "\u{2500}".repeat(area.width as usize);
        let hdiv_y = area.y + top_h;
        f.render_widget(
            Paragraph::new(Span::styled(hdiv_str, Style::default().fg(hdiv_fg))),
            Rect { x: area.x, y: hdiv_y, width: area.width, height: 1 },
        );

        let bot_area = Rect { y: bot_area.y + 1, height: bot_h.saturating_sub(1), ..bot_area };

        // ── bottom library columns ───────────────────────────────────────────
        self.render_power_libraries(f, bot_area);
    }

    fn render_power_card(&mut self, f: &mut Frame, area: Rect) {
        let cursor = self.player_tab.playlist_cursor;
        let n = self.player_tab.items.len();
        if n == 0 {
            f.render_widget(
                Paragraph::new("Queue is empty").style(Style::default().fg(palette::MUTED)),
                area,
            );
            return;
        }
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

        // Render image only — no text, no seekbar.
        if let Some(Some(state)) = self.card_image_states.get_mut(&cache_key) {
            type SImg = ratatui_image::StatefulImage::<ratatui_image::protocol::StatefulProtocol>;
            let avail = ratatui::layout::Size { width: area.width, height: area.height };
            let actual = state.size_for(
                ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3)), avail,
            );
            let img_x = area.x + (area.width.saturating_sub(actual.width)) / 2;
            let img_y = area.y;
            let img_rect = Rect { x: img_x, y: img_y, width: actual.width, height: actual.height };
            f.render_stateful_widget(
                SImg::default().resize(ratatui_image::Resize::Scale(Some(ratatui_image::FilterType::Lanczos3))),
                img_rect, state,
            );
        }
    }

    fn render_power_playback_controls(&mut self, f: &mut Frame, area: Rect) {
        if area.height == 0 { return; }
        let (position_ticks, runtime_ticks, paused) = if let Some(ref remote) = self.connected_session_state {
            let elapsed_s = self.remote_pos_at.elapsed().as_secs_f64();
            let pos_s = (self.remote_pos_s as f64 + elapsed_s).min(remote.runtime_s as f64);
            let pos_ticks = (pos_s * crate::api::TICKS_PER_SECOND as f64) as i64;
            (pos_ticks, remote.runtime_s * crate::api::TICKS_PER_SECOND, remote.is_paused)
        } else {
            let s = self.player.status.lock().unwrap();
            (s.position_ticks, s.runtime_ticks, s.paused)
        };
        let pos_s = position_ticks / TICKS_PER_SECOND;
        let dur_s = runtime_ticks / TICKS_PER_SECOND;
        let pos_str = fmt_duration(pos_s);
        let dur_str = fmt_duration(dur_s);
        let time_style  = Style::default().fg(palette::SUBTLE);
        let delim_style = Style::default().fg(palette::OVERLAY);
        let elapsed_w = pos_str.chars().count() as u16;
        let total_w   = dur_str.chars().count() as u16;
        self.layout_seekbar_area = Rect::default();
        self.layout_tracks_area  = Rect::default();
        self.layout_vol_area     = Rect::default();
        self.layout_sub_area     = Rect::default();
        self.layout_audio_area   = Rect::default();
        if self.use_nerd_fonts {
            const BTNS_W: u16 = 30;
            let btn_style  = Style::default().fg(Color::Rgb(203, 212, 241));
            let stop_style = Style::default().fg(palette::SUBTLE);
            let pp_icon = if !paused { "\u{F03E4}" } else { "\u{F040A}" };
            let btn_icons: &[(&str, Style)] = &[
                ("\u{F04AE}", btn_style),
                ("\u{F04A}",  btn_style),
                (pp_icon,     btn_style),
                ("\u{F04DB}", stop_style),
                ("\u{F04E}",  btn_style),
                ("\u{F04AD}", btn_style),
            ];
            let mut spans: Vec<Span> = vec![
                Span::styled(pos_str.clone(), time_style),
                Span::styled(" \u{2502} ", delim_style),
            ];
            for (icon, style) in btn_icons.iter() {
                spans.push(Span::styled(format!("  {icon}  "), *style));
            }
            spans.push(Span::styled(" \u{2502} ", delim_style));
            spans.push(Span::styled(dur_str.clone(), time_style));
            const DELIM_W: u16 = 3;
            let row_w = elapsed_w + DELIM_W + BTNS_W + DELIM_W + total_w;
            let btn_x = area.x + area.width.saturating_sub(row_w) / 2 + elapsed_w + DELIM_W;
            self.layout_button_area = Rect { x: btn_x, y: area.y, width: BTNS_W, height: 1 };
            f.render_widget(
                Paragraph::new(Line::from(spans)).alignment(Alignment::Center),
                Rect { x: area.x, y: area.y, width: area.width, height: 1 },
            );
        } else {
            f.render_widget(
                Paragraph::new(Span::styled(pos_str, time_style)),
                Rect { x: area.x, y: area.y, width: elapsed_w.min(area.width), height: 1 },
            );
            let total_x = area.x + area.width.saturating_sub(total_w);
            f.render_widget(
                Paragraph::new(Span::styled(dur_str, time_style)),
                Rect { x: total_x, y: area.y, width: total_w.min(area.width), height: 1 },
            );
            self.layout_button_area = Rect::default();
        }
    }

    fn render_power_queue(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height < 3 { return; }

        let active = self.player.status.lock().unwrap().active
            || self.connected_session_state.is_some();

        // Title row: now-playing name, centered, at the very top of the queue area.
        let area = if active && self.show_playback_panel {
            let title: Option<String> = {
                let pst = self.player.status.lock().unwrap();
                if pst.active {
                    let idx = pst.current_idx;
                    drop(pst);
                    self.player_tab.items.get(idx).map(|i| i.playback_label())
                } else {
                    drop(pst);
                    self.connected_session_state.as_ref().and_then(|s| s.now_playing.clone())
                }
            };
            if let Some(t) = title {
                f.render_widget(
                    Paragraph::new(t)
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)),
                    Rect { x: area.x, y: area.y, width: area.width, height: 1 },
                );
            }
            Rect { y: area.y + 1, height: area.height.saturating_sub(1), ..area }
        } else {
            area
        };

        let list_area = if active && self.show_playback_panel {
            self.render_power_playback_controls(f, Rect { x: area.x, y: area.y, width: area.width, height: 1 });
            let div_y = area.y + 1;
            let div_w = area.width;
            // Inner dashes divider: ─── [ 1080p en CC ] ───
            let hdiv_fg = palette::IRIS;
            if let Some(inner) = self.build_status_indicator_spans() {
                let bracket = Style::default().fg(palette::WHITE).add_modifier(Modifier::BOLD);
                let dash    = Style::default().fg(hdiv_fg);
                let inner_w: u16 = inner.iter().map(|s| s.content.width() as u16).sum();
                let group_w = inner_w + 4; // "[ " + inner + " ]"
                let total_dashes = div_w.saturating_sub(group_w);
                let left_dashes  = total_dashes / 2;
                let right_dashes = total_dashes - left_dashes;
                let mut spans: Vec<Span> = Vec::new();
                spans.push(Span::styled("\u{2500}".repeat(left_dashes as usize), dash));
                spans.push(Span::styled("[", bracket));
                spans.push(Span::raw(" "));
                spans.extend(inner);
                spans.push(Span::raw(" "));
                spans.push(Span::styled("]", bracket));
                spans.push(Span::styled("\u{2500}".repeat(right_dashes as usize), dash));
                f.render_widget(
                    Paragraph::new(Line::from(spans)),
                    Rect { x: area.x, y: div_y, width: div_w, height: 1 },
                );
            } else {
                f.render_widget(
                    Paragraph::new(Span::styled("\u{2500}".repeat(div_w as usize), Style::default().fg(hdiv_fg))),
                    Rect { x: area.x, y: div_y, width: div_w, height: 1 },
                );
            }
            Rect { y: area.y + 2, height: area.height.saturating_sub(2), ..area }
        } else {
            area
        };

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
                Style::default().fg(palette::FOAM).add_modifier(Modifier::BOLD)
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
                    .style(Style::default().fg(if is_active { if focused { palette::FOAM } else { palette::MUTED } } else { palette::SUBTLE })),
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

    fn render_power_libraries(&mut self, f: &mut Frame, area: Rect) {
        let n_libs = self.libs.len();
        if n_libs == 0 || area.height == 0 || area.width == 0 { return; }

        // How many columns fit? Always show at least 2, then add more at MIN_COL_W.
        let n_cols = if area.width >= MIN_COL_W * 3 {
            (area.width / MIN_COL_W) as usize
        } else {
            2usize // squeeze: 2 share the space even below min
        }.min(n_libs);

        // Clamp scroll so we don't show empty columns on the right.
        let max_scroll = n_libs.saturating_sub(n_cols);
        self.power_lib_col_scroll = self.power_lib_col_scroll.min(max_scroll);
        let col_scroll = self.power_lib_col_scroll;

        let col_w = area.width / n_cols as u16;
        let extra = area.width - col_w * n_cols as u16; // distribute remainder to last col

        self.power_lib_col_areas.clear();

        // Ensure each visible library column has triggered its initial load.
        for ci in 0..n_cols {
            self.ensure_lib_loaded_for(col_scroll + ci);
        }

        // Scroll indicator shown in the right-most panel header when scrollable.
        let indicator = if n_libs > n_cols {
            let shown_pos = if let PowerFocus::Library(idx) = self.power_focus { idx + 1 } else { col_scroll + 1 };
            format!("[{}/{}]", shown_pos, n_libs)
        } else {
            String::new()
        };

        for ci in 0..n_cols {
            let lib_idx = col_scroll + ci;
            if lib_idx >= n_libs { break; }

            let x = area.x + ci as u16 * col_w;
            let w = if ci == n_cols - 1 { col_w + extra } else { col_w };
            let col_area = Rect { x, y: area.y, width: w, height: area.height };
            self.power_lib_col_areas.push((lib_idx, col_area));

            // Column header: library name, highlighted green if focused
            let focused = matches!(self.power_focus, PowerFocus::Library(idx) if idx == lib_idx);
            let lib_name = self.libs[lib_idx].library.name.clone();
            let header_fg = palette::IRIS;
            let is_rightmost = ci == n_cols - 1 && !indicator.is_empty();
            let ind_len = if is_rightmost { indicator.len() as u16 } else { 0 };

            if focused {
                let title_budget = (w as usize).saturating_sub(4 + ind_len as usize);
                let label = format!("  {}  ", trunc_str(&lib_name, title_budget));
                let title_w = w.saturating_sub(ind_len);
                f.render_widget(
                    Paragraph::new(Span::styled(label, Style::default().fg(palette::WHITE).bg(palette::IRIS).add_modifier(Modifier::BOLD))),
                    Rect { x, y: area.y, width: title_w, height: 1 },
                );
                if is_rightmost {
                    f.render_widget(
                        Paragraph::new(Span::styled(indicator.clone(), Style::default().fg(palette::SUBTLE))),
                        Rect { x: x + title_w, y: area.y, width: ind_len, height: 1 },
                    );
                }
            } else {
                let title_budget = (w as usize).saturating_sub(2 + ind_len as usize);
                let title_w = w.saturating_sub(ind_len);
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(trunc_str(&lib_name, title_budget), Style::default().fg(palette::YELLOW)),
                    ])),
                    Rect { x, y: area.y, width: title_w, height: 1 },
                );
                if is_rightmost {
                    f.render_widget(
                        Paragraph::new(Span::styled(indicator.clone(), Style::default().fg(palette::SUBTLE))),
                        Rect { x: x + title_w, y: area.y, width: ind_len, height: 1 },
                    );
                }
            }
            // underline beneath header — replaced with search label when active
            if area.height > 1 {
                if let Some(s) = &self.libs[lib_idx].search {
                    let query_text = if s.loading {
                        format!(" Search \u{2026}: {}\u{2588} ", s.query)
                    } else {
                        format!(" Search: {}\u{2588} ", s.query)
                    };
                    let label_w = query_text.chars().count().min(w as usize);
                    let remaining = (w as usize).saturating_sub(1 + label_w);
                    let mut spans = vec![
                        Span::styled("\u{2500}", Style::default().fg(header_fg)),
                        Span::styled(trunc_str(&query_text, label_w), Style::default().fg(palette::YELLOW).add_modifier(Modifier::BOLD)),
                    ];
                    if remaining > 0 {
                        spans.push(Span::styled("\u{2500}".repeat(remaining), Style::default().fg(header_fg)));
                    }
                    f.render_widget(
                        Paragraph::new(Line::from(spans)),
                        Rect { x, y: area.y + 1, width: w, height: 1 },
                    );
                } else {
                    let uline = "\u{2500}".repeat(w as usize);
                    f.render_widget(
                        Paragraph::new(Span::styled(uline, Style::default().fg(header_fg))),
                        Rect { x, y: area.y + 1, width: w, height: 1 },
                    );
                }
            }

            let content_area = Rect { y: area.y + 2, height: col_area.height.saturating_sub(2), ..col_area };

            self.render_power_lib_col(f, content_area, lib_idx, focused);
        }
    }

    fn render_power_lib_col(&mut self, f: &mut Frame, area: Rect, lib_idx: usize, focused: bool) {
        // Collect display items and cursor, respecting active search.
        let (items, cursor, loading) = {
            let lib = &self.libs[lib_idx];
            if let Some(s) = &lib.search {
                let items: Vec<crate::api::MediaItem> = s.results.iter()
                    .filter_map(|&i| s.items.get(i).cloned())
                    .collect();
                (items, s.cursor, s.loading)
            } else {
                match lib.nav_stack.last() {
                    Some(lvl) => (lvl.items.clone(), lvl.cursor, lvl.loading),
                    None => return,
                }
            }
        };

        if loading && self.libs[lib_idx].search.is_none() {
            f.render_widget(
                Paragraph::new(Span::styled("Loading...", Style::default().fg(palette::MUTED))),
                area,
            );
            return;
        }

        let n = items.len();

        if n == 0 {
            f.render_widget(
                Paragraph::new(Span::styled("(empty)", Style::default().fg(palette::MUTED))),
                area,
            );
            return;
        }

        let visible = area.height as usize;
        let offset = if cursor >= visible { cursor - visible + 1 } else { 0 };

        // Store the table area for mouse hit-testing
        if let Some(entry) = self.power_lib_col_areas.iter_mut().find(|(idx, _)| *idx == lib_idx) {
            entry.1 = area;
        }
        if let Some(v) = self.layout_lib_table_area.get_mut(lib_idx) { *v = area; }

        let list_items: Vec<ListItem> = items.iter().skip(offset).take(visible).enumerate().map(|(i, item)| {
            let abs = offset + i;
            let selected = abs == cursor;
            let (text, _) = item_text_and_style(item, selected);
            let title = trunc_str(&text, (area.width as usize).saturating_sub(2));
            let fg = if !focused {
                palette::SUBTLE
            } else if item.is_folder {
                palette::WHITE
            } else {
                palette::TEXT
            };
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
        f.render_stateful_widget(List::new(list_items).highlight_style(Style::default()), area, &mut state);

        // Scrollbar: only shown for the focused column.
        if focused && n > visible {
            let max_off = n.saturating_sub(visible);
            let mut sb = ScrollbarState::new(max_off + 1).position(offset);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None).end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area, &mut sb,
            );
        }
    }
}
