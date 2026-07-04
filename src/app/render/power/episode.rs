use super::super::super::ui_util::*;
use crate::api::TICKS_PER_SECOND;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Renders the combined TV series view: episode metadata at the top, season
    /// tabs + green divider above the episode list at the bottom.
    pub(super) fn render_power_episode_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
    ) {
        if area.height == 0 {
            return;
        }

        // ── Collect nav state ───────────────────────────────────────────────
        let stack_len = self.libs[lib_idx].nav_stack.len();
        let (seasons, season_cursor, items, ep_cursor) = {
            let lib = &self.libs[lib_idx];
            let last = match lib.nav_stack.last() {
                Some(l) => l,
                None => return,
            };
            let at_episodes = last
                .items
                .first()
                .map(|i| i.item_type == "Episode")
                .unwrap_or(false);
            if at_episodes && stack_len >= 2 {
                let season_lvl = &lib.nav_stack[stack_len - 2];
                let have_seasons = season_lvl
                    .items
                    .first()
                    .map(|i| i.item_type == "Season")
                    .unwrap_or(false);
                if have_seasons {
                    (
                        season_lvl.items.clone(),
                        season_lvl.cursor,
                        last.items.clone(),
                        last.cursor,
                    )
                } else {
                    (vec![], 0, last.items.clone(), last.cursor)
                }
            } else if last
                .items
                .first()
                .map(|i| i.item_type == "Season")
                .unwrap_or(false)
            {
                // Loading state: at season level, episodes still arriving.
                (last.items.clone(), last.cursor, vec![], 0)
            } else {
                (vec![], 0, last.items.clone(), last.cursor)
            }
        };

        let inner_x = area.x + 1;
        let inner_w = (area.width as usize).saturating_sub(2);
        let inner_w16 = area.width.saturating_sub(2);
        let max_y = area.y + area.height;
        let mut row = area.y;

        // ── Selected episode metadata (top of panel) ─────────────────────────
        let text_color = if focused {
            palette::WHITE
        } else {
            palette::SUBTLE
        };

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
                    let avail = ratatui::layout::Size {
                        width: IMG_COLS,
                        height: IMG_MAX_ROWS,
                    };
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
            self.power_inline_image_rect = if img_height > 0 {
                Some(Rect {
                    x: img_x,
                    y: img_start_row,
                    width: img_actual_w,
                    height: img_height + 1,
                })
            } else {
                None
            };
            let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
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
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
                row += 1;
            }

            // ── Series metadata (year range + genre) ─────────────────────────
            if row < max_y {
                // Try to get the series item from the nav level two above episode level.
                let series_item = if stack_len >= 3 {
                    let series_lvl = &self.libs[lib_idx].nav_stack[stack_len - 3];
                    series_lvl.items.get(series_lvl.cursor).cloned()
                } else {
                    None
                };
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
                    .iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join("  ");
                if !ser_meta.is_empty() {
                    let (tw, tw16) = text_dims(row);
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            trunc_str(&ser_meta, tw),
                            Style::default().fg(palette::SUBTLE),
                        ))),
                        Rect {
                            x: inner_x,
                            y: row,
                            width: tw16,
                            height: 1,
                        },
                    );
                    row += 1;
                }
            }

            // Blank spacer after series block
            if row < max_y {
                row += 1;
            }

            // ── Episode title (PINE/green) ────────────────────────────────────
            if row < max_y {
                let ep_title_color = if focused {
                    palette::PINE
                } else {
                    palette::SUBTLE
                };
                let (tw, tw16) = text_dims(row);
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(&item.name, tw),
                        Style::default().fg(ep_title_color),
                    ))),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
                row += 1;
            }

            // ── Episode metadata (year + duration only) ───────────────────────
            if row < max_y {
                let dur_str = if item.runtime_ticks > 0 {
                    fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                } else {
                    String::new()
                };
                let year_str = if item.production_year > 0 {
                    item.production_year.to_string()
                } else {
                    String::new()
                };
                let ep_meta = [year_str.as_str(), dur_str.as_str()]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join("  ");
                if !ep_meta.is_empty() {
                    let (tw, tw16) = text_dims(row);
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            trunc_str(&ep_meta, tw),
                            Style::default().fg(palette::SUBTLE),
                        ))),
                        Rect {
                            x: inner_x,
                            y: row,
                            width: tw16,
                            height: 1,
                        },
                    );
                    row += 1;
                }
            }

            // Blank spacer
            if row < max_y {
                row += 1;
            }

            // Overview (word-wrapped, respects image shadow width).
            // Fill the space alongside/below the series image, reserving room for
            // the play-status line, divider, and a usable episode list below.
            let used = row.saturating_sub(area.y);
            const OV_RESERVED: u16 = 8; // play status + spacer + divider + a few episode rows
            let max_ov_rows = (area.height.saturating_sub(used).saturating_sub(OV_RESERVED)
                as usize)
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
                if !cur_line.is_empty() {
                    ov_lines.push(cur_line);
                }
                for line_text in ov_lines.iter().take(max_ov_rows) {
                    if row >= max_y {
                        break;
                    }
                    let (_, tw16) = text_dims(row);
                    f.render_widget(
                        Paragraph::new(Line::from(Span::styled(
                            line_text.clone(),
                            Style::default().fg(text_color),
                        ))),
                        Rect {
                            x: inner_x,
                            y: row,
                            width: tw16,
                            height: 1,
                        },
                    );
                    row += 1;
                }
                if row < max_y {
                    row += 1;
                }
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
                        Block::default().style(Style::default().bg(palette::OVERLAY)),
                        img_rect,
                    );
                } else if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key)
                {
                    type SImg =
                        ratatui_image::StatefulImage<ratatui_image::protocol::StatefulProtocol>;
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Scale(Some(
                            ratatui_image::FilterType::Lanczos3,
                        ))),
                        img_rect,
                        state,
                    );
                }
            }
        }

        // Push below the image if it extends past the text.
        if row < metadata_img_end_row {
            row = metadata_img_end_row;
        }

        // ── Grey divider with season tabs overlaid ───────────────────────────
        if row < max_y {
            row += 1;
        } // blank spacer above divider
        if row < max_y {
            // Draw the full-width blue rule first.
            let line_str = "\u{2500}".repeat(area.width as usize);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    line_str,
                    Style::default().fg(palette::FOAM),
                ))),
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
            );

            // Overlay season tabs on the same row (drawn on top of the rule).
            if !seasons.is_empty() {
                // Labels are bare numbers: "01", "02", …
                let tab_labels: Vec<String> = seasons
                    .iter()
                    .enumerate()
                    .map(|(i, s)| {
                        let n = if s.index_number > 0 {
                            s.index_number as usize
                        } else {
                            i + 1
                        };
                        format!("{:02}", n)
                    })
                    .collect();
                // pill width = 1 space + 2 digits + 1 space = 4; gap between pills = 1
                let per_tab = 5usize;
                let n_tabs = tab_labels.len();

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
                spans.push(Span::styled(
                    "Series: ",
                    Style::default().fg(ratatui::style::Color::White),
                ));
                if scroll_start > 0 {
                    spans.push(Span::styled(
                        "\u{2039} ",
                        Style::default().fg(palette::FOAM),
                    ));
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
                        Style::default()
                            .fg(fg)
                            .bg(palette::FOAM)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(fg).bg(palette::FOAM)
                    };
                    spans.push(Span::styled(format!(" {} ", label), style));
                }
                if scroll_end < n_tabs {
                    spans.push(Span::styled(
                        " \u{203a}",
                        Style::default().fg(palette::FOAM),
                    ));
                }
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

        // ── Loading state: episodes not yet arrived ─────────────────────────
        if items.is_empty() {
            if row < max_y {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        " Loading\u{2026}",
                        Style::default().fg(palette::MUTED),
                    ))),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
            }
            return;
        }

        // ── Episode list ─────────────────────────────────────────────────────
        let table_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        if table_area.height == 0 {
            return;
        }

        let (active2, active_idx2, _, _, _) = self.effective_playback_state();
        let now_playing_id2: Option<String> = if active2 {
            self.playback_queue().items.get(active_idx2).map(|i| i.id.clone())
        } else {
            None
        };

        // Prefetch card images for episodes near the cursor.
        const EP_AHEAD: usize = 4;
        const EP_BEHIND: usize = 2;
        let n = items.len();
        {
            let pf_start = ep_cursor.saturating_sub(EP_BEHIND);
            let pf_end = (ep_cursor + EP_AHEAD + 1).min(n);
            let prefetch: Vec<(String, String, String)> = items[pf_start..pf_end]
                .iter()
                .map(|ep| {
                    (
                        format!("{}:pwr_ep", ep.id),
                        ep.id.clone(),
                        ep.series_id.clone(),
                    )
                })
                .collect();
            for (cache_key, ep_id, series_id) in prefetch {
                self.fetch_card_image(cache_key, ep_id, series_id, &["Primary", "Backdrop"]);
            }
            // Prefetch adjacent seasons' posters under the same key format so the
            // card shows something immediately when the user switches seasons.
            let series_id_adj = items
                .first()
                .map(|ep| ep.series_id.clone())
                .unwrap_or_default();
            for delta in [-1i64, 1i64] {
                let adj = season_cursor as i64 + delta;
                if adj >= 0 && (adj as usize) < seasons.len() {
                    let s = &seasons[adj as usize];
                    let ck = format!("{}:pwr_ep", s.id);
                    self.fetch_card_image(
                        ck,
                        s.id.clone(),
                        series_id_adj.clone(),
                        &["Primary", "Backdrop"],
                    );
                }
            }
        }

        let show_length = table_area.width > 40;
        let dur_col_w: usize = if show_length { 7 } else { 0 };
        let title_col_w = (table_area.width as usize)
            .saturating_sub(1 + if show_length { dur_col_w + 1 } else { 0 });

        let rows: Vec<Row> = items
            .iter()
            .enumerate()
            .map(|(i, ep)| {
                let is_cursor = i == ep_cursor;
                let is_playing = now_playing_id2.as_deref() == Some(ep.id.as_str());
                let row_style = if is_playing {
                    Style::default()
                        .fg(palette::FOAM)
                        .add_modifier(Modifier::BOLD)
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
                let length = if len_secs > 0 {
                    fmt_duration_approx(len_secs)
                } else {
                    "\u{2014}".to_string()
                };
                if show_length {
                    Row::new([
                        title_cell,
                        Cell::from(Line::from(length).alignment(Alignment::Right))
                            .style(Style::default().fg(palette::SUBTLE)),
                        Cell::from(""),
                    ])
                    .style(row_style)
                } else {
                    Row::new([title_cell, Cell::from(""), Cell::from("")]).style(row_style)
                }
            })
            .collect();

        let mut state = TableState::default();
        state.select(Some(ep_cursor));
        let table = Table::new(
            rows,
            [
                Constraint::Min(10),
                Constraint::Length(if show_length { dur_col_w as u16 } else { 0 }),
                Constraint::Length(1),
            ],
        )
        .column_spacing(1)
        .row_highlight_style(Style::default());
        f.render_stateful_widget(table, table_area, &mut state);
        self.power_cursor_screen_y =
            Some(table_area.y + (ep_cursor.saturating_sub(state.offset())) as u16);

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
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                sb_area,
                &mut sb_state,
            );
        }
    }
}
