use super::super::super::ui_util::*;
use super::{effective_sort_str, letter_bucket, parse_album_folder_name, strip_article};
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Rows the compact movie banner occupies inline in the library list: an
/// opening horizontal rule directly above the selected item's own row (so the
/// selected title reads as visually set off from the row above it), the
/// banner's own content (meta/overview/poster, rendered by
/// `render_power_compact_detail`) directly below the selected row, and a
/// closing horizontal rule that also acts as the 1-row separator before the
/// next list row — matching the spacing the banner already used when it was
/// pinned to the top of the panel. The rules make the selected row + banner
/// read as a distinct, boxed-off region instead of blending into the
/// surrounding list.
const COMPACT_BANNER_RULE_ROWS: usize = 1;
const COMPACT_BANNER_CONTENT_ROWS: usize = 13;
const COMPACT_BANNER_GAP_ROWS: usize = 1;
const COMPACT_BANNER_TOTAL_ROWS: usize =
    COMPACT_BANNER_RULE_ROWS + COMPACT_BANNER_CONTENT_ROWS + COMPACT_BANNER_GAP_ROWS;

impl App {
    /// Filler-row count to reserve around the selected movie's row in
    /// `lib_idx`'s display-row sequence: `COMPACT_BANNER_TOTAL_ROWS` when a
    /// leaf movie is selected and expanded detail is not open, else 0 (no
    /// banner — ordinary list rendering, unchanged from before this feature).
    /// One of those rows is the opening rule placed immediately *before* the
    /// selected item's row; the rest (content + closing rule) follow it.
    fn compact_banner_rows(&self, lib_idx: usize) -> usize {
        if self.libs[lib_idx].power_detail_item.is_some() {
            return 0;
        }
        if self.power_selected_movie_item(lib_idx).is_some() {
            COMPACT_BANNER_TOTAL_ROWS
        } else {
            0
        }
    }

    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar by `render_power_view`.
    pub(super) fn render_power_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }

        // Ensure the library is loaded when a library tab is selected.
        if self.power_left_tab > 0 {
            self.ensure_lib_loaded_for(self.power_left_tab - 1);
        }

        let mut content_area = area;

        // Store for click / page-size calculations.
        layout.left_area = content_area;

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
                let items: Vec<mbv_core::api::MediaItem> = s
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

        // Reserved filler-row count for the compact movie banner, 0 for every
        // library type/state except "leaf movie selected, detail not pinned".
        let banner_rows: usize = if self.power_left_tab > 0 {
            self.compact_banner_rows(self.power_left_tab - 1)
        } else {
            0
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
            // Build a display row list that interleaves artist headers with album rows.
            enum DisplayRow {
                ArtistHeader(String),
                Album(usize),
            }

            // Precompute (artist, row_prefix, album_name) for each item.
            // Artist resolution (Emby tag → fetched-from-first-tracks cache →
            // folder-name guess → "Unknown Artist") is centralized in
            // `resolve_group_album_artist` since it may need to kick off an
            // async fetch, hence the explicit loop instead of a pure `.map()`.
            // (artist, year_str, album_name) — year_str is empty if no year.
            let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(items.len());
            for item in &items {
                let artist = self.resolve_group_album_artist(item);
                let (year_str, album_name) = if !item.artist.is_empty() {
                    let year_str = if item.production_year > 0 {
                        item.production_year.to_string()
                    } else {
                        String::new()
                    };
                    (year_str, item.display_name())
                } else if let Some((_, year, album)) = parse_album_folder_name(&item.name) {
                    let year_str = if year > 0 {
                        year.to_string()
                    } else {
                        String::new()
                    };
                    (year_str, album)
                } else {
                    (String::new(), item.display_name())
                };
                album_info.push((artist, year_str, album_name));
            }

            // Albums arrive sorted by album name (SortName), not by artist, so a
            // stable sort by artist is needed first — otherwise the same artist
            // (e.g. "Unknown Artist" for tag-less compilations) resurfaces as a
            // new header every time it's interrupted by a different album name.
            let mut order: Vec<usize> = (0..album_info.len()).collect();
            order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));

            // Publish the sorted order so keyboard cursor movement (PageUp/Down,
            // Home/End — see `move_lib_cursor`/`jump_lib_cursor` in actions.rs)
            // follows display order rather than the raw (SortName-by-album-title)
            // order `items` arrived in — otherwise arrow keys jump the cursor to
            // an unrelated album whenever the artist-sort permutes the list.
            layout.left_sorted_indices = order.clone();

            let mut display_rows: Vec<DisplayRow> = Vec::new();
            let mut last_artist = String::new();
            for &idx in &order {
                let artist = &album_info[idx].0;
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
            layout.cursor_screen_y =
                Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            // Row map so mouse clicks translate a visual row back to the correct
            // (sorted-order) album index; header rows map to None.
            layout.left_row_map = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|dr| match dr {
                    DisplayRow::ArtistHeader(_) => None,
                    DisplayRow::Album(idx) => Some(*idx),
                })
                .collect();
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
                BannerFiller,
            }

            // Sort item indices by the same effective key used for bucketing so that
            // items within each group appear in article-stripped alphabetical order.
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by_key(|&i| natural_sort_key(effective_sort_str(&items[i])));
            // Publish the sorted order so cursor navigation can follow display order.
            layout.left_sorted_indices = sorted_indices.clone();

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
                // The opening rule sits directly above the selected item's own
                // row (not after it), so the selected title reads as set off
                // from the row above it.
                if banner_rows > 0 && idx == cursor {
                    display_rows.push(DisplayRow::BannerFiller);
                }
                display_rows.push(DisplayRow::Item(idx));
                if banner_rows > 0 && idx == cursor {
                    for _ in 0..banner_rows.saturating_sub(1) {
                        display_rows.push(DisplayRow::BannerFiller);
                    }
                }
            }
            let total_display = display_rows.len();

            // Find the visual row of the current cursor item for scrolling.
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
                .unwrap_or(0);
            // Only `banner_rows - 1` rows sit *below* the cursor now (the
            // opening rule sits above it), hence the `- 1`.
            let lower_bound = (display_cursor + banner_rows.saturating_sub(1))
                .saturating_sub(visible.saturating_sub(1))
                .min(display_cursor);
            let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
            // If stale scroll state would put the first item of a bucket at the
            // top of the viewport, back up so its letter header remains visible.
            // When that item is also the selected/bannered one, the banner's
            // opening rule sits between the header and the item, so back up an
            // extra row to clear the rule too.
            if visible > 1 && offset > 0 && matches!(display_rows.get(offset), Some(DisplayRow::Item(_)))
            {
                if matches!(
                    display_rows.get(offset - 1),
                    Some(DisplayRow::LetterHeader(_))
                ) {
                    offset -= 1;
                } else if offset >= 2
                    && matches!(display_rows.get(offset - 1), Some(DisplayRow::BannerFiller))
                    && matches!(
                        display_rows.get(offset - 2),
                        Some(DisplayRow::LetterHeader(_))
                    )
                {
                    offset -= 2;
                }
            }
            final_offset = offset;

            // Build row map so mouse clicks can map visual row → item index.
            for row in display_rows.iter().skip(offset).take(visible) {
                layout.left_row_map.push(match row {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) | DisplayRow::BannerFiller => {
                        None
                    }
                    DisplayRow::Item(idx) => Some(*idx),
                });
            }

            // Absolute display-row indices of the banner's opening and closing
            // horizontal rules (only meaningful when banner_rows > 0). The
            // opening rule sits directly above the selected item's row; the
            // closing rule sits after the banner content, before the next
            // list row. Together they bracket the selected row + banner as a
            // distinct region rather than blending into the surrounding list.
            let banner_rule_top = display_cursor.saturating_sub(1);
            let content_start = display_cursor + 1;
            let banner_rule_bottom = content_start + banner_rows.saturating_sub(2);
            let show_scrollbar = focused && total_display > visible;

            let avail = (area.width as usize).saturating_sub(2);
            let list_items: Vec<ListItem> = display_rows
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible)
                .map(|(abs_idx, row)| match row {
                    DisplayRow::Spacer => ListItem::new(Line::default()),
                    DisplayRow::BannerFiller => {
                        if banner_rows > 0
                            && (abs_idx == banner_rule_top || abs_idx == banner_rule_bottom)
                        {
                            ListItem::new(Line::from(vec![
                                Span::raw(" "),
                                Span::styled(
                                    "\u{2500}".repeat(avail),
                                    Style::default().fg(palette::OVERLAY),
                                ),
                            ]))
                        } else {
                            ListItem::new(Line::default())
                        }
                    }
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
            layout.cursor_screen_y =
                Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            if banner_rows > 0 {
                if content_start >= offset && content_start < offset + visible {
                    let banner_y = content_area.y + (content_start - offset) as u16;
                    let bottom = content_area.y + content_area.height;
                    // Reserve the rightmost column for the scrollbar (drawn over
                    // content_area's last column below) so the poster image,
                    // which is right-anchored, doesn't render underneath it.
                    let banner_w = if show_scrollbar {
                        content_area.width.saturating_sub(1)
                    } else {
                        content_area.width
                    };
                    let banner_h =
                        (COMPACT_BANNER_CONTENT_ROWS as u16).min(bottom.saturating_sub(banner_y));
                    if banner_h > 0 {
                        let banner_rect = Rect {
                            x: content_area.x,
                            y: banner_y,
                            width: banner_w,
                            height: banner_h,
                        };
                        let want_cursor_y = layout.cursor_screen_y;
                        self.render_power_compact_detail(
                            f,
                            banner_rect,
                            self.power_left_tab - 1,
                            focused,
                            layout,
                        );
                        layout.cursor_screen_y = want_cursor_y;
                    }
                }
            }

            if show_scrollbar {
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
            enum DisplayRow {
                Item(usize),
                BannerFiller,
            }

            let mut display_rows: Vec<DisplayRow> = Vec::with_capacity(n + banner_rows);
            for i in 0..n {
                // The opening rule sits directly above the selected item's own
                // row (not after it), so the selected title reads as set off
                // from the row above it.
                if banner_rows > 0 && i == cursor {
                    display_rows.push(DisplayRow::BannerFiller);
                }
                display_rows.push(DisplayRow::Item(i));
                if banner_rows > 0 && i == cursor {
                    for _ in 0..banner_rows.saturating_sub(1) {
                        display_rows.push(DisplayRow::BannerFiller);
                    }
                }
            }
            let total_display = display_rows.len();
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
                .unwrap_or(0);

            // Lower bound normally just keeps the cursor row visible; when a
            // banner follows it, extend the lower bound so scrolling keeps
            // pulling up until the whole banner is visible too (clamped to
            // display_cursor itself if the viewport could never fit both).
            // Only `banner_rows - 1` rows sit *below* the cursor now (the
            // opening rule sits above it), hence the `- 1`.
            let lower_bound = (display_cursor + banner_rows.saturating_sub(1))
                .saturating_sub(visible.saturating_sub(1))
                .min(display_cursor);
            let offset = stored_scroll.clamp(lower_bound, display_cursor);
            final_offset = offset;

            // Absolute display-row indices of the banner's opening and closing
            // horizontal rules (only meaningful when banner_rows > 0). The
            // opening rule sits directly above the selected item's row; the
            // closing rule sits after the banner content, before the next
            // list row. Together they bracket the selected row + banner as a
            // distinct region rather than blending into the surrounding list.
            let banner_rule_top = display_cursor.saturating_sub(1);
            let content_start = display_cursor + 1;
            let banner_rule_bottom = content_start + banner_rows.saturating_sub(2);
            let show_scrollbar = focused && total_display > visible;

            let list_items: Vec<ListItem> = display_rows
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible)
                .map(|(abs_idx, row)| match row {
                    DisplayRow::BannerFiller => {
                        if banner_rows > 0
                            && (abs_idx == banner_rule_top || abs_idx == banner_rule_bottom)
                        {
                            let avail = (area.width as usize).saturating_sub(2);
                            ListItem::new(Line::from(vec![
                                Span::raw(" "),
                                Span::styled(
                                    "\u{2500}".repeat(avail),
                                    Style::default().fg(palette::OVERLAY),
                                ),
                            ]))
                        } else {
                            ListItem::new(Line::default())
                        }
                    }
                    DisplayRow::Item(idx) => {
                        let item = &items[*idx];
                        let selected = *idx == cursor;

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
                    }
                })
                .collect();

            layout.left_row_map = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|row| match row {
                    DisplayRow::BannerFiller => None,
                    DisplayRow::Item(idx) => Some(*idx),
                })
                .collect();

            let mut state = ListState::default();
            state.select(Some(display_cursor.saturating_sub(offset)));
            layout.cursor_screen_y =
                Some(content_area.y + (display_cursor.saturating_sub(offset)) as u16);
            f.render_stateful_widget(
                List::new(list_items).highlight_style(Style::default()),
                content_area,
                &mut state,
            );

            if banner_rows > 0 {
                if content_start >= offset && content_start < offset + visible {
                    let banner_y = content_area.y + (content_start - offset) as u16;
                    let bottom = content_area.y + content_area.height;
                    // Reserve the rightmost column for the scrollbar (drawn over
                    // content_area's last column below) so the poster image,
                    // which is right-anchored, doesn't render underneath it.
                    let banner_w = if show_scrollbar {
                        content_area.width.saturating_sub(1)
                    } else {
                        content_area.width
                    };
                    let banner_h =
                        (COMPACT_BANNER_CONTENT_ROWS as u16).min(bottom.saturating_sub(banner_y));
                    if banner_h > 0 {
                        let banner_rect = Rect {
                            x: content_area.x,
                            y: banner_y,
                            width: banner_w,
                            height: banner_h,
                        };
                        // render_power_compact_detail overwrites layout.cursor_screen_y with
                        // the banner's own top row; restore the selected list row's y after,
                        // since that row (not the banner) is what should host the blinking
                        // cursor / mouse hit target.
                        let want_cursor_y = layout.cursor_screen_y;
                        self.render_power_compact_detail(
                            f,
                            banner_rect,
                            self.power_left_tab - 1,
                            focused,
                            layout,
                        );
                        layout.cursor_screen_y = want_cursor_y;
                    }
                }
            }

            if show_scrollbar {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::layout::LayoutPower;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{BrowseLevel, LibraryTab};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;
    use ratatui::Terminal;

    fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    fn render_power_list_to_string(app: &mut App, layout: &mut LayoutPower) -> String {
        let backend = TestBackend::new(60, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_list(f, Rect::new(0, 0, 60, 8), true, layout);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    fn render_power_list_to_string_sized(
        app: &mut App,
        layout: &mut LayoutPower,
        width: u16,
        height: u16,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_list(f, Rect::new(0, 0, width, height), true, layout);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    fn make_power_movie_list_app(titles: Vec<&str>) -> App {
        let mut app = make_app_stub();
        app.power_left_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let items: Vec<_> = titles
            .into_iter()
            .enumerate()
            .map(|(i, title)| {
                let mut m = make_item(title, "Movie");
                m.id = format!("movie-{i}");
                if title.contains("Selected") {
                    m.overview = "This is the compact movie banner overview text.".into();
                }
                m
            })
            .collect();
        let total = items.len();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items,
                total_count: total,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,
        });

        app
    }

    #[test]
    fn letter_group_keeps_top_bucket_header_after_scrolling_back_to_top() {
        let mut app = make_app_stub();
        app.power_left_tab = 1;

        let mut library = make_item("Power View Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.collection_type = "movies".into();
        library.is_folder = true;

        let mut items = Vec::new();
        let mut first = make_item("The 8 Diagram Pole Fighter", "Movie");
        first.id = "movie-first".into();
        first.sort_name = "8 Diagram Pole Fighter".into();
        items.push(first);
        for i in 0..670 {
            let mut item = make_item(&format!("Movie {i:03}"), "Movie");
            item.id = format!("movie-{i:03}");
            items.push(item);
        }

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Power View Movies".into(),
                items,
                total_count: 671,
                cursor: 0,
                scroll: 1,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
            }],
            search: None,
            feed_home_video: None,
            power_detail_item: None,
            power_detail_scroll: 0,
        });

        let mut layout = LayoutPower::default();
        app.libs[0].nav_stack[0].cursor = 20;
        app.libs[0].nav_stack[0].scroll = 20;
        let _ = render_power_list_to_string(&mut app, &mut layout);

        app.libs[0].nav_stack[0].cursor = 0;
        app.libs[0].nav_stack[0].scroll = 1;
        let out = render_power_list_to_string(&mut app, &mut layout);

        assert!(out.contains("#"), "expected first bucket header:\n{out}");
        assert!(
            out.find('#').unwrap() < out.find("The 8 Diagram Pole Fighter").unwrap(),
            "expected bucket header above first item:\n{out}"
        );
        assert_eq!(app.libs[0].nav_stack[0].scroll, 0);
    }

    #[test]
    fn compact_banner_appears_inline_in_letter_grouped_movie_list() {
        // 60 movies triggers use_letter_groups (total_count >= 50, collection_type
        // != "music"). Titles are spread across many starting letters (cycling
        // A..Z) so the selected item's letter bucket is followed by several more
        // buckets -- this is what exercises the riskiest part of the interleaving
        // logic: filler rows must land between the selected item and the NEXT
        // bucket's header, not get scattered or appended after the whole list.
        let titles: Vec<String> = (0..60)
            .map(|i| {
                let letter = (b'A' + (i % 26) as u8) as char;
                format!("{letter} Movie {i:02}")
            })
            .collect();
        let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
        let mut app = make_power_movie_list_app(title_refs);

        // Select an early-alphabet item (letter 'K') so later letter buckets --
        // e.g. the 'Z' item -- must sort, and therefore render, after it.
        let selected_idx = 10; // letter (b'A' + 10) as char == 'K'
        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.items[selected_idx].overview =
                "This is the compact movie banner overview text.".into();
            lvl.cursor = selected_idx;
        }
        let selected_title = titles[selected_idx].clone();
        let later_title = titles[25].clone(); // letter (b'A' + 25) as char == 'Z'

        let mut layout = LayoutPower::default();
        let out = render_power_list_to_string_sized(&mut app, &mut layout, 60, 60);

        let selected_pos = out
            .find(selected_title.as_str())
            .expect("selected item's row should render");
        let banner_pos = out
            .find("compact movie banner")
            .expect("expected banner overview text to appear in letter-grouped list render");
        assert!(
            selected_pos < banner_pos,
            "banner should render after the selected row, not before it:\n{out}"
        );
        if let Some(later_pos) = out.find(later_title.as_str()) {
            assert!(
                banner_pos < later_pos,
                "banner must land inline between the selected item and later alphabet \
                 buckets, not scattered after the whole list:\n{out}"
            );
        }
    }

    #[test]
    fn compact_banner_appears_under_selected_row_not_pinned_to_top() {
        let mut app =
            make_power_movie_list_app(vec!["First", "Second Selected", "Third"]);
        // Select the second item (index 1) — banner must render after ITS row, not row 0.
        app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;

        let mut layout = LayoutPower::default();
        let out = render_power_list_to_string_sized(&mut app, &mut layout, 40, 20);
        let lines: Vec<&str> = out.lines().collect();

        let first_pos = out.find("First").expect("row above cursor unaffected");
        let selected_row_line = lines
            .iter()
            .position(|l| l.contains("Second Selected"))
            .expect("selected row itself, unaffected by banner");
        let selected_row_pos = out.find("Second Selected").unwrap();
        assert!(
            first_pos < selected_row_pos,
            "row for First expected before the selected row:\n{out}"
        );
        let banner_pos = out
            .find("compact movie banner")
            .expect("banner content expected somewhere after the selected row");
        assert!(
            banner_pos > selected_row_pos,
            "banner content expected after the selected row:\n{out}"
        );
        // Third item pushed down by the 14 reserved banner rows (13 content + 1 gap),
        // so it should not appear on the row immediately after the selected row.
        assert!(
            !lines[selected_row_line + 1].contains("Third"),
            "Third should not appear immediately after Second Selected:\n{out}"
        );
        let third_line = lines
            .iter()
            .position(|l| l.contains("Third"))
            .expect("Third item expected further down, pushed below the banner");
        assert!(
            third_line >= selected_row_line + COMPACT_BANNER_TOTAL_ROWS,
            "Third expected pushed below the reserved banner rows (row {third_line}, selected row {selected_row_line}):\n{out}"
        );
    }
}
