use super::super::super::ui_util::*;
use super::{effective_sort_str, letter_bucket, parse_album_folder_name};
use crate::api::TICKS_PER_SECOND;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar by `render_power_view`.
    pub(super) fn render_power_list(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height == 0 {
            return;
        }

        // Ensure the library is loaded when a library tab is selected.
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
        }

        let mut content_area = area;

        // Store for click / page-size calculations.
        self.power_left_area = content_area;

        // Gather items, cursor, stored scroll offset, and the *true* library total
        // (not just how many pages have been fetched so far) from the appropriate
        // source.
        let (items, cursor, stored_scroll, total_count) = if self.power_left_tab == 0 {
            let items = self.home.continue_items.clone();
            let cursor = self.home.continue_cursor.min(items.len().saturating_sub(1));
            let total = items.len();
            (items, cursor, 0usize, total)
        } else {
            let lib_idx = self.power_left_tab - 1;
            let lib = &self.libs[lib_idx];
            let (items, cur, scroll, total) = if let Some(s) = &lib.search {
                let items: Vec<crate::api::MediaItem> = s
                    .results
                    .iter()
                    .filter_map(|&i| s.items.get(i).cloned())
                    .collect();
                // Search results are already the full locally-filtered match set,
                // not paginated, so their length is already the true total.
                let total = items.len();
                (items, s.cursor, s.scroll, total)
            } else {
                match lib.nav_stack.last() {
                    // `total_count` comes from Emby's TotalRecordCount, not
                    // `items.len()` -- with lazy pagination `items` may only hold
                    // a subset of the library until the user scrolls further.
                    Some(lvl) => (lvl.items.clone(), lvl.cursor, lvl.scroll, lvl.total_count),
                    None => (vec![], 0, 0, 0),
                }
            };
            (items, cur, scroll, total)
        };

        // When at the album level of a music library, group albums under artist headers.
        let show_grouped = if self.power_left_tab > 0 {
            self.is_viewing_album_folders(self.power_left_tab - 1)
        } else {
            false
        };

        let n = items.len();

        // Letter grouping: applies to non-music library lists with 50+ items (not during search).
        // Gated on the true library total, not the fetched-so-far count, so the
        // grouping style (ranges vs. individual letters) doesn't change out from
        // under the user as more pages lazily load in.
        let use_letter_groups = !show_grouped && self.power_left_tab > 0 && total_count >= 50 && {
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
                let search_area = Rect {
                    height: 3,
                    ..content_area
                };
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
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_type(BorderType::Rounded)
                                .border_style(Style::default().fg(palette::IRIS))
                                .title(Span::styled(
                                    " Search ",
                                    Style::default().fg(palette::YELLOW),
                                )),
                        ),
                    search_area,
                );
            } else if !has_search {
                let count_label = format!(" {} items", total_count);
                f.render_widget(
                    Paragraph::new(Span::styled(
                        count_label,
                        Style::default().fg(palette::SUBTLE),
                    )),
                    Rect {
                        height: 1,
                        ..content_area
                    },
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
                if self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.loading)
                    .unwrap_or(false)
                {
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
        let final_offset: usize;

        if show_grouped {
            self.power_left_sorted_indices.clear();
            // Build a display row list that interleaves artist headers with album rows.
            enum DisplayRow {
                ArtistHeader(String),
                Album(usize),
            }

            // Precompute (artist, row_prefix, album_name) for each item.
            // When proper Emby metadata is available (item.artist non-empty), use it directly.
            // Otherwise fall back to parsing "Artist (YYYY) Album" from the folder name.
            // (artist, year_str, album_name) — year_str is empty if no year.
            let album_info: Vec<(String, String, String)> = items
                .iter()
                .map(|item| {
                    if !item.artist.is_empty() {
                        let year_str = if item.production_year > 0 {
                            item.production_year.to_string()
                        } else {
                            String::new()
                        };
                        (item.artist.clone(), year_str, item.display_name())
                    } else if let Some((artist, year, album)) = parse_album_folder_name(&item.name)
                    {
                        let year_str = if year > 0 {
                            year.to_string()
                        } else {
                            String::new()
                        };
                        (artist, year_str, album)
                    } else {
                        (
                            "Unknown Artist".to_string(),
                            String::new(),
                            item.display_name(),
                        )
                    }
                })
                .collect();

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
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Album(i) if *i == cursor))
                .unwrap_or(0);
            let offset = stored_scroll.clamp(
                display_cursor.saturating_sub(visible.saturating_sub(1)),
                display_cursor,
            );
            final_offset = offset;

            let avail = (area.width as usize).saturating_sub(2);

            let list_items: Vec<ListItem> = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|row| {
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
                            let prefix_w = if year_str.is_empty() {
                                3
                            } else {
                                year_str.len() + 6
                            };
                            let name_w = avail.saturating_sub(prefix_w);
                            let trunc_name = trunc_str(album_name, name_w);
                            let fg = if focused {
                                palette::WHITE
                            } else {
                                palette::SUBTLE
                            };
                            let name_color = if selected && focused {
                                palette::IRIS
                            } else {
                                fg
                            };
                            let mut spans: Vec<Span> = Vec::new();
                            if selected && focused {
                                spans.push(Span::styled(
                                    "\u{258c}",
                                    Style::default().fg(palette::PINE),
                                ));
                            } else {
                                spans.push(Span::raw(" "));
                            }
                            if year_str.is_empty() {
                                spans.push(Span::raw("   "));
                            } else {
                                spans.push(Span::styled(
                                    "   (",
                                    Style::default().fg(palette::SUBTLE),
                                ));
                                spans.push(Span::styled(
                                    year_str.clone(),
                                    Style::default().fg(palette::PINE),
                                ));
                                spans
                                    .push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
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
                })
                .collect();

            let mut state = ListState::default();
            state.select(Some(display_cursor.saturating_sub(offset)));
            self.power_cursor_screen_y = Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            self.power_left_row_map.clear();
            let display_n = display_rows.len();
            if focused && display_n > visible {
                let max_off = display_n.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None)
                        .end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area,
                    &mut sb,
                );
            }
        } else if use_letter_groups {
            // Build display rows: inject a Spacer+LetterHeader at each bucket boundary.
            // The spacer is omitted before the very first header.
            enum DisplayRow {
                Spacer,
                LetterHeader(String),
                Item(usize),
            }

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
                let bucket = letter_bucket(item, total_count);
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
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
                .unwrap_or(0);
            let offset = stored_scroll.clamp(
                display_cursor.saturating_sub(visible.saturating_sub(1)),
                display_cursor,
            );
            final_offset = offset;

            // Build row map so mouse clicks can map visual row → item index.
            self.power_left_row_map.clear();
            for row in display_rows.iter().skip(offset).take(visible) {
                self.power_left_row_map.push(match row {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) => None,
                    DisplayRow::Item(idx) => Some(*idx),
                });
            }

            let avail = (area.width as usize).saturating_sub(2);
            let list_items: Vec<ListItem> = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|row| match row {
                    DisplayRow::Spacer => ListItem::new(Line::default()),
                    DisplayRow::LetterHeader(label) => ListItem::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(
                            label.clone(),
                            Style::default()
                                .fg(palette::YELLOW)
                                .add_modifier(Modifier::BOLD),
                        ),
                    ])),
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
                                format!(
                                    " {}",
                                    fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                                )
                            } else {
                                String::new()
                            };
                            (item.display_name(), dur)
                        };
                        let name_w = avail.saturating_sub(dur_str.width());
                        let title = trunc_str(&item_name, name_w);
                        let fg = if focused {
                            palette::WHITE
                        } else {
                            palette::SUBTLE
                        };
                        let mut spans: Vec<Span> = if selected && focused {
                            vec![
                                Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                                Span::styled(
                                    title,
                                    Style::default()
                                        .fg(palette::IRIS)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ]
                        } else {
                            vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                        };
                        if !dur_str.is_empty() {
                            spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                        }
                        ListItem::new(Line::from(spans))
                    }
                })
                .collect();

            let mut state = ListState::default();
            state.select(Some(display_cursor.saturating_sub(offset)));
            self.power_cursor_screen_y = Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            if focused && total_display > visible {
                let max_off = total_display.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None)
                        .end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area,
                    &mut sb,
                );
            }
        } else {
            self.power_left_row_map.clear();
            self.power_left_sorted_indices.clear();
            let offset =
                stored_scroll.clamp(cursor.saturating_sub(visible.saturating_sub(1)), cursor);
            final_offset = offset;

            let list_items: Vec<ListItem> = items
                .iter()
                .skip(offset)
                .take(visible)
                .enumerate()
                .map(|(i, item)| {
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
                            format!(
                                " {}",
                                fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                            )
                        } else {
                            String::new()
                        };
                        (item.display_name(), dur)
                    };

                    let avail = (area.width as usize).saturating_sub(2);
                    let name_w = avail.saturating_sub(dur_str.width());
                    let title = trunc_str(&item_name, name_w);
                    let fg = if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };

                    let mut spans: Vec<Span> = if selected && focused {
                        vec![
                            Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                            Span::styled(
                                title,
                                Style::default()
                                    .fg(palette::IRIS)
                                    .add_modifier(Modifier::BOLD),
                            ),
                        ]
                    } else {
                        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                    };
                    if !dur_str.is_empty() {
                        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
                    }
                    ListItem::new(Line::from(spans))
                })
                .collect();

            let mut state = ListState::default();
            state.select(Some(cursor.saturating_sub(offset)));
            self.power_cursor_screen_y = Some(content_area.y + (cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            if focused && n > visible {
                let max_off = n.saturating_sub(visible);
                let mut sb = ScrollbarState::new(max_off + 1).position(offset);
                f.render_stateful_widget(
                    Scrollbar::new(ScrollbarOrientation::VerticalRight)
                        .thumb_symbol("\u{2590}")
                        .track_symbol(Some(" "))
                        .begin_symbol(None)
                        .end_symbol(None)
                        .style(Style::default().fg(palette::SUBTLE)),
                    content_area,
                    &mut sb,
                );
            }
        }

        // Persist the scroll offset so the viewport is remembered across frames.
        // power_left_tab is always > 0 here (tab == 0 uses render_power_home_list).
        if self.power_left_tab > 0 {
            let lib_idx = self.power_left_tab - 1;
            if let Some(s) = &mut self.libs[lib_idx].search {
                s.scroll = final_offset;
            } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                lvl.scroll = final_offset;
            }
        }
    }
}
