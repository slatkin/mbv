use super::super::super::ui_util::*;
use super::detail::compact_banner_image_cache_key;
use super::{effective_sort_str, letter_bucket};
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

/// Rows the compact movie banner occupies inline in the library list. The
/// selected movie row + the banner's own content (meta/overview/poster,
/// rendered by `render_power_compact_detail`, directly below the selected row)
/// are wrapped in a `palette::MEDIA_SELECTED_BG` colored block — a dark
/// (#282828) background visually similar to the home tab's Keep Watching
/// list — instead of horizontal rules. The two
/// constants below reserve one row above the selected item (the block's top
/// padding, replacing the previous opening `─` rule) and one row after the
/// banner content (the block's bottom padding, replacing the previous closing
/// `─` rule), and `COMPACT_BANNER_INDENT` reserves that many columns of
/// external side padding on each side of the colored block (matched one-for-
/// one by `render_power_compact_detail`'s own internal padding, so the
/// visible side padding is `INDENT + 1` columns on each side).
const COMPACT_BANNER_RULE_ROWS: usize = 1;
const COMPACT_BANNER_GAP_ROWS: usize = 1;
const COMPACT_BANNER_INDENT: u16 = 1;

impl App {
    /// Filler-row count to reserve around the selected movie's row in
    /// `lib_idx`'s display-row sequence: the colored block's top/bottom
    /// padding rows plus the banner's actual content height
    /// (meta/overview/director wrapped to `panel_width`, computed by
    /// `compact_banner_layout` — #263 replaced the old fixed content-row
    /// constant with this, so a longer overview grows the reserved space and
    /// a shorter one shrinks it) when a leaf movie is selected, else 0 (no
    /// banner — ordinary list rendering). One of the reserved rows is the
    /// top padding placed immediately *before* the selected item's row; the
    /// rest (content + bottom padding) follow it.
    ///
    /// `panel_width` matches the banner's eventual `Rect` width
    /// (`content_area.width - 2 * COMPACT_BANNER_INDENT` — see
    /// `render_power_compact_detail`'s inner padding), so the row count the
    /// layout reserves and the rows the banner actually renders stay in
    /// lockstep.
    fn compact_banner_rows(&mut self, lib_idx: usize, panel_width: u16) -> usize {
        let Some(item) = self.power_selected_movie_item(lib_idx) else {
            return 0;
        };
        let content_rows = self
            .compact_banner_layout(&item, panel_width)
            .content_rows();
        COMPACT_BANNER_RULE_ROWS + content_rows + COMPACT_BANNER_GAP_ROWS
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
                    .filter_map(|&i| {
                        s.items
                            .get(i)
                            .map(|item| self.recursive_album_display_item(lib_idx, i, item.clone()))
                    })
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
        // The width estimate matches the final banner rect's width:
        // `content_area.width.saturating_sub(2 * COMPACT_BANNER_INDENT)` (= the
        // colored block's width minus the external side padding, with the right
        // external pad covering the scrollbar column when one shows up).
        let banner_rows: usize = if self.power_left_tab > 0 {
            let banner_panel_width = content_area
                .width
                .saturating_sub(1)
                .saturating_sub(COMPACT_BANNER_INDENT);
            self.compact_banner_rows(self.power_left_tab - 1, banner_panel_width)
        } else {
            0
        };
        // Content-only row count (banner_rows minus its top/bottom colored-pad
        // filler rows), used below to size the banner rect to the same
        // content-dependent height that was reserved for it above.
        let banner_content_rows: usize =
            banner_rows.saturating_sub(COMPACT_BANNER_RULE_ROWS + COMPACT_BANNER_GAP_ROWS);

        // Pre-warm nearby movies' poster images so they're already cached by
        // the time the cursor reaches them (#287) -- mirrors the prefetch
        // window `render_power_card` already uses for the home-card
        // carousel. Only applies when a movie banner is actually showing
        // (i.e. this is a movies library with a leaf Movie selected); if
        // there's no banner, there's nothing to prefetch for.
        if self.power_left_tab > 0 {
            let lib_idx = self.power_left_tab - 1;
            if self.power_selected_movie_item(lib_idx).is_some() {
                const PREFETCH_AHEAD: usize = 3;
                const PREFETCH_BEHIND: usize = 1;
                let start = cursor.saturating_sub(PREFETCH_BEHIND);
                let end = (cursor + PREFETCH_AHEAD + 1).min(items.len());
                let prefetch: Vec<(String, String, String)> = items[start..end]
                    .iter()
                    .enumerate()
                    .filter(|(i, item)| {
                        start + i != cursor && item.item_type == "Movie" && !item.is_folder
                    })
                    .map(|(_, item)| {
                        (
                            compact_banner_image_cache_key(&item.id),
                            item.id.clone(),
                            item.series_id.clone(),
                        )
                    })
                    .collect();
                if self.images_enabled() {
                    for (cache_key, item_id, series_id) in prefetch {
                        self.fetch_list_card_image_when_idle(
                            cache_key,
                            item_id,
                            series_id,
                            &["Primary"],
                        );
                    }
                }
            }
        }

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
                if self.recursive_album_search_enabled(lib_idx)
                    && self.libs[lib_idx]
                        .search
                        .as_ref()
                        .is_some_and(|search| search.loading)
                {
                    "Indexing music library..."
                } else if self.libs[lib_idx]
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
            let lib_idx = self.power_left_tab - 1;
            final_offset = self.render_power_grouped_album_rows(
                f,
                content_area,
                lib_idx,
                &items,
                cursor,
                stored_scroll,
                focused,
                layout,
            );
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
            if visible > 1
                && offset > 0
                && matches!(
                    display_rows.get(offset),
                    Some(DisplayRow::Item(_) | DisplayRow::BannerFiller)
                )
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

            // Absolute display-row indices of the colored block's top and
            // bottom padding rows (only meaningful when banner_rows > 0).
            // `banner_rule_top` is the padding row directly above the selected
            // item's own row; `banner_rule_bottom` is the padding row after
            // the banner content, before the next list row. Together they
            // frame the selected row + banner as a single CONTINUE_BG block
            // instead of `─` rules around it.
            let banner_rule_top = display_cursor.saturating_sub(1);
            let content_start = display_cursor + 1;
            let banner_rule_bottom = content_start + banner_rows.saturating_sub(2);
            let show_scrollbar = focused && total_display > visible;

            // The selected movie + banner are wrapped in a CONTINUE_BG colored
            // block (matching the home tab's Keep Watching look). Draw the
            // block first, before the list items, so the per-row spans only
            // paint their own cells and the block's background shows through
            // on the side padding cols and on the top/bottom padding rows.
            if banner_rows > 0 {
                let vis_top = banner_rule_top.max(offset);
                let vis_bot = banner_rule_bottom.min(offset + visible - 1);
                if vis_top <= vis_bot {
                    let block_y = content_area.y + (vis_top - offset) as u16;
                    let block_h = (vis_bot - vis_top + 1) as u16;
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::MEDIA_SELECTED_BG)),
                        Rect {
                            x: content_area.x,
                            y: block_y,
                            width: content_area.width,
                            height: block_h,
                        },
                    );
                }
            }

            // Width available to title + duration on a normal list row (with a
            // 1-col leading separator before the title). For the selected row
            // with an inline banner, the colored block's 2-col side padding
            // + render_power_compact_detail's own internal 1-col pad reserve
            // `2 * COMPACT_BANNER_INDENT + 2` cols off both sides, so the
            // title aligns with the banner's `inner_x` exactly.
            let avail = (area.width as usize).saturating_sub(2 + COMPACT_BANNER_INDENT as usize);
            let list_items: Vec<ListItem> = display_rows
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible)
                .map(|(_abs_idx, row)| match row {
                    DisplayRow::Spacer => ListItem::new(Line::default()),
                    // The colored block (drawn above) frames the selected row
                    // + banner, so the banner's top/bottom padding rows are
                    // empty -- they show the block's CONTINUE_BG.
                    DisplayRow::BannerFiller => ListItem::new(Line::default()),
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
                        let selected_has_banner = selected && banner_rows > 0;
                        let avail = if selected_has_banner {
                            // 2-col left pad + 2-col right pad inside the
                            // colored block: title+dur share area.width - 4.
                            (area.width as usize)
                                .saturating_sub(2 + 2 * COMPACT_BANNER_INDENT as usize)
                        } else {
                            avail
                        };
                        let name_w = avail.saturating_sub(dur_str.width());
                        let title = trunc_str(&item_name, name_w);
                        let fg = if focused {
                            palette::WHITE
                        } else {
                            palette::SUBTLE
                        };
                        let mut spans: Vec<Span> = if selected {
                            if selected_has_banner {
                                // Colored-block look: 2-col leading pad inside
                                // the MEDIA_SELECTED_BG block (aligns the
                                // title with the banner's `inner_x`), no green
                                // `▌` gutter. Title is yellow (BOLD when
                                // focused) and the row omits the duration --
                                // it lives in the banner's metadata row below.
                                let title_style = if focused {
                                    Style::default()
                                        .fg(palette::YELLOW)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(palette::YELLOW)
                                };
                                vec![Span::raw("  "), Span::styled(title, title_style)]
                            } else {
                                // Otherwise keep the green gutter for selected
                                // list rows without an inline banner.
                                let title_style = if focused {
                                    Style::default()
                                        .fg(palette::IRIS)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(fg)
                                };
                                vec![
                                    Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                                    Span::styled(title, title_style),
                                ]
                            }
                        } else {
                            vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                        };
                        if !selected_has_banner && !dur_str.is_empty() {
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

            if banner_rows > 0 && content_start >= offset && content_start < offset + visible {
                let banner_y = content_area.y + (content_start - offset) as u16;
                let bottom = content_area.y + content_area.height;
                let banner_h = (banner_content_rows as u16).min(bottom.saturating_sub(banner_y));
                if banner_h > 0 {
                    // The banner content sits inside the colored block with
                    // `COMPACT_BANNER_INDENT` cols of external side padding on
                    // each side (and render_power_compact_detail's own
                    // internal 1-col pad), so the poster image — right-anchored
                    // inside `banner_rect` — never renders under the scrollbar
                    // (which is drawn on the rightmost col afterwards).
                    let banner_rect = Rect {
                        x: content_area.x + COMPACT_BANNER_INDENT,
                        y: banner_y,
                        width: content_area.width.saturating_sub(2 * COMPACT_BANNER_INDENT),
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

            if show_scrollbar {
                let max_off = total_display.saturating_sub(visible);
                super::render_power_scrollbar(f, content_area, max_off, offset);
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

            // Absolute display-row indices of the colored block's top and
            // bottom padding rows (only meaningful when banner_rows > 0).
            // `banner_rule_top` is the padding row directly above the selected
            // item's own row; `banner_rule_bottom` is the padding row after
            // the banner content, before the next list row.
            let banner_rule_top = display_cursor.saturating_sub(1);
            let content_start = display_cursor + 1;
            let banner_rule_bottom = content_start + banner_rows.saturating_sub(2);
            let show_scrollbar = focused && total_display > visible;

            // The selected movie + banner are wrapped in a CONTINUE_BG colored
            // block (matching the home tab's Keep Watching look). Draw the
            // block first, before the list items, so the per-row spans only
            // paint their own cells and the block's background shows through
            // on the side padding cols and on the top/bottom padding rows.
            if banner_rows > 0 {
                let vis_top = banner_rule_top.max(offset);
                let vis_bot = banner_rule_bottom.min(offset + visible - 1);
                if vis_top <= vis_bot {
                    let block_y = content_area.y + (vis_top - offset) as u16;
                    let block_h = (vis_bot - vis_top + 1) as u16;
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::MEDIA_SELECTED_BG)),
                        Rect {
                            x: content_area.x,
                            y: block_y,
                            width: content_area.width,
                            height: block_h,
                        },
                    );
                }
            }

            let list_items: Vec<ListItem> = display_rows
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible)
                .map(|(_abs_idx, row)| match row {
                    // The colored block (drawn above) frames the selected row
                    // + banner, so the banner's top/bottom padding rows are
                    // empty -- they show the block's CONTINUE_BG.
                    DisplayRow::BannerFiller => ListItem::new(Line::default()),
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

                        let selected_has_banner = selected && banner_rows > 0;
                        let avail = if selected_has_banner {
                            // 2-col left pad + 2-col right pad inside the
                            // colored block: title+dur share area.width - 4.
                            (area.width as usize)
                                .saturating_sub(2 + 2 * COMPACT_BANNER_INDENT as usize)
                        } else if selected {
                            (area.width as usize).saturating_sub(1)
                        } else {
                            (area.width as usize).saturating_sub(2)
                        };
                        let name_w = avail.saturating_sub(dur_str.width());
                        let title = trunc_str(&item_name, name_w);
                        let fg = if focused {
                            palette::WHITE
                        } else {
                            palette::SUBTLE
                        };

                        let mut spans: Vec<Span> = if selected {
                            if selected_has_banner {
                                // Colored-block look: 2-col leading pad inside
                                // the MEDIA_SELECTED_BG block (aligns the
                                // title with the banner's `inner_x`), no green
                                // `▌` gutter. Title is yellow (BOLD when
                                // focused) and the row omits the duration --
                                // it lives in the banner's metadata row below.
                                let title_style = if focused {
                                    Style::default()
                                        .fg(palette::YELLOW)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(palette::YELLOW)
                                };
                                vec![Span::raw("  "), Span::styled(title, title_style)]
                            } else {
                                // Otherwise keep the green gutter for selected
                                // list rows without an inline banner.
                                let title_style = if focused {
                                    Style::default()
                                        .fg(palette::IRIS)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default().fg(fg)
                                };
                                vec![
                                    Span::styled("\u{258c}", Style::default().fg(palette::PINE)),
                                    Span::styled(title, title_style),
                                ]
                            }
                        } else {
                            vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
                        };
                        if !selected_has_banner && !dur_str.is_empty() {
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

            if banner_rows > 0 && content_start >= offset && content_start < offset + visible {
                let banner_y = content_area.y + (content_start - offset) as u16;
                let bottom = content_area.y + content_area.height;
                let banner_h = (banner_content_rows as u16).min(bottom.saturating_sub(banner_y));
                if banner_h > 0 {
                    // The banner content sits inside the colored block with
                    // `COMPACT_BANNER_INDENT` cols of external side padding on
                    // each side (and render_power_compact_detail's own
                    // internal 1-col pad), so the poster image — right-anchored
                    // inside `banner_rect` — never renders under the scrollbar
                    // (which is drawn on the rightmost col afterwards).
                    let banner_rect = Rect {
                        x: content_area.x + COMPACT_BANNER_INDENT,
                        y: banner_y,
                        width: content_area.width.saturating_sub(2 * COMPACT_BANNER_INDENT),
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

            if show_scrollbar {
                let max_off = total_display.saturating_sub(visible);
                super::render_power_scrollbar(f, content_area, max_off, offset);
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
    use crate::app::{AlbumIndexState, BrowseLevel, LibSearch, LibraryTab};
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

            album_track_focus: None,
            artist_header_focus: None,
        });

        app
    }

    // #287: nearby movies' poster images should be prefetched before the
    // cursor reaches them, mirroring render_power_card's existing home-card
    // -carousel prefetch window (PREFETCH_AHEAD = 3 / PREFETCH_BEHIND = 1).
    #[test]
    fn compact_banner_prefetches_nearby_movies_but_not_beyond_the_window() {
        // 6 movies, cursor on item 0. PREFETCH_AHEAD = 3 / PREFETCH_BEHIND = 1
        // means the window covers indices 0..=3 (cursor has no items behind
        // it here, so PREFETCH_BEHIND has nothing to reach). Item 0 itself is
        // excluded from the prefetch loop (it's covered by its own eager
        // fetch), so movies 1-3 should be prefetched and movie 4 should not.
        let titles: Vec<&str> = vec![
            "Movie 0", "Movie 1", "Movie 2", "Movie 3", "Movie 4", "Movie 5",
        ];
        let mut app = make_power_movie_list_app(titles);
        app.image_protocol_enabled = true;

        let mut layout = LayoutPower::default();
        let _ = render_power_list_to_string(&mut app, &mut layout);

        let fetch_triggered = |app: &App, key: &str| {
            app.card_image_loading.contains(key) || app.card_image_states.contains_key(key)
        };

        // The currently-selected item's own eager fetch (unchanged existing
        // behavior, from compact_banner_layout).
        let selected_key = compact_banner_image_cache_key("movie-0");
        assert!(
            fetch_triggered(&app, &selected_key),
            "expected the selected movie's own image fetch to still be triggered"
        );

        // Prefetch window: movies 1-3 (within PREFETCH_AHEAD = 3) should be
        // prefetched.
        for i in 1..=3 {
            let key = compact_banner_image_cache_key(&format!("movie-{i}"));
            assert!(
                fetch_triggered(&app, &key),
                "expected movie-{i} to be prefetched (within the prefetch window)"
            );
        }

        // Movie 4 sits just outside the PREFETCH_AHEAD = 3 window and should
        // not be prefetched.
        let outside_key = compact_banner_image_cache_key("movie-4");
        assert!(
            !fetch_triggered(&app, &outside_key),
            "movie-4 is outside the prefetch window and should not have been fetched"
        );
    }

    #[test]
    fn recursive_album_search_loading_message_is_explicit() {
        let mut app = make_app_stub();
        app.power_left_tab = 1;
        app.music_levels = vec!["group".into(), "album".into()];
        let mut library = make_item("Music", "CollectionFolder");
        library.id = "music-lib".into();
        library.collection_type = "music".into();
        library.is_folder = true;
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: Some(LibSearch {
                query: "record".into(),
                items: Vec::new(),
                results: Vec::new(),
                cursor: 0,
                scroll: 0,
                loading: true,
            }),
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
        });
        app.album_indexes.insert(
            "music-lib".into(),
            AlbumIndexState::Loading {
                rebuild_pending: false,
            },
        );

        let out = render_power_list_to_string(&mut app, &mut LayoutPower::default());

        assert!(out.contains("Indexing music library..."), "{out}");

        app.music_levels.clear();
        let out = render_power_list_to_string(&mut app, &mut LayoutPower::default());
        assert!(!out.contains("Indexing music library..."), "{out}");
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

            album_track_focus: None,
            artist_header_focus: None,
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
    fn letter_group_backs_up_two_rows_to_reveal_header_when_banner_opening_rule_intervenes() {
        // Cursor lands on the first item of a non-initial letter bucket, with the
        // compact banner active (it's a leaf movie) reserving its top padding
        // filler row directly above the item. A stale scroll that would
        // otherwise clamp exactly onto the item's own row would strand that
        // padding row (and the bucket header above it) with no header visible
        // -- this exercises the extended nudge (back up 2 rows instead of 1)
        // added specifically for the case where a BannerFiller row sits
        // between a LetterHeader and the selected item, which the older "back
        // up one row" nudge alone doesn't cover.
        let titles: Vec<String> = (0..60)
            .map(|i| {
                let letter = (b'A' + (i % 26) as u8) as char;
                format!("{letter} Movie {i:02}")
            })
            .collect();
        let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
        let mut app = make_power_movie_list_app(title_refs);

        // "D Movie 03" is the first D-lettered item (smallest index, smallest
        // number among D items), so it's the first item of its bucket -- and
        // it's preceded by real A/B/C-bucket content, so there's room for the
        // offset to legitimately land >= 2 while still clamping onto this
        // item's own row.
        let target_idx = 3;
        let target_title = titles[target_idx].clone();

        let mut layout = LayoutPower::default();

        // Scroll deep into the list first so a large `scroll` value persists.
        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.cursor = 55;
        }
        let _ = render_power_list_to_string_sized(&mut app, &mut layout, 60, 10);

        // Jump the cursor back to the bucket-first item without resetting
        // scroll -- the stale deep scroll clamps down to exactly the item's
        // own display-row index, the scenario needing the extended nudge.
        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.cursor = target_idx;
        }
        let out = render_power_list_to_string_sized(&mut app, &mut layout, 60, 10);
        let lines: Vec<&str> = out.lines().collect();

        let title_line_idx = lines
            .iter()
            .position(|l| l.contains(target_title.as_str()))
            .expect("selected item's row should render");
        // The colored block's top padding row (previously the opening `─`
        // rule) sits directly above the selected item. It renders as an
        // empty CONTINUE_BG row, so it trimmed is empty.
        assert!(
            title_line_idx >= 2,
            "expected two rows above the selected item (its colored block's top \
             padding row + the bucket header), not the title at row \
             {title_line_idx}:\n{out}"
        );
        let pad_line = lines[title_line_idx - 1].trim();
        let header_line = lines[title_line_idx - 2].trim();
        assert!(
            pad_line.is_empty(),
            "expected the row directly above the selected item to be the empty \
             colored padding row, not visible content:\n{out}"
        );
        assert!(
            !header_line.is_empty() && !header_line.contains(target_title.as_str()),
            "expected the bucket header directly above the colored padding row, \
             not stranded off-screen by the nudge:\n{out}"
        );
    }

    #[test]
    fn letter_group_backs_up_from_banner_filler_row_to_keep_header_visible() {
        // Same reachable state as the previous regression, but with a taller
        // viewport so stale scroll clamps to the banner's top padding filler
        // row rather than the selected item row itself. The header should
        // still be nudged back into view.
        let titles: Vec<String> = (0..60)
            .map(|i| {
                let letter = (b'A' + (i % 26) as u8) as char;
                format!("{letter} Movie {i:02}")
            })
            .collect();
        let title_refs: Vec<&str> = titles.iter().map(String::as_str).collect();
        let mut app = make_power_movie_list_app(title_refs);

        let target_idx = 3;
        let target_title = titles[target_idx].clone();

        let mut layout = LayoutPower::default();

        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.cursor = 55;
        }
        let _ = render_power_list_to_string_sized(&mut app, &mut layout, 60, 16);

        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.cursor = target_idx;
        }
        let out = render_power_list_to_string_sized(&mut app, &mut layout, 60, 16);
        let lines: Vec<&str> = out.lines().collect();

        let title_line_idx = lines
            .iter()
            .position(|l| l.contains(target_title.as_str()))
            .expect("selected item's row should render");
        assert!(
            title_line_idx >= 2,
            "expected two rows above the selected item (its colored block's top \
             padding row + the bucket header), not the title at row \
             {title_line_idx}:\n{out}"
        );
        let pad_line = lines[title_line_idx - 1].trim();
        let header_line = lines[title_line_idx - 2].trim();
        assert!(
            pad_line.is_empty(),
            "expected the row directly above the selected item to be the empty \
             colored padding row, not visible content:\n{out}"
        );
        assert!(
            !header_line.is_empty() && !header_line.contains(target_title.as_str()),
            "expected the bucket header directly above the colored padding row, \
             not stranded off-screen when scroll lands on the banner filler \
             row:\n{out}"
        );
    }

    #[test]
    fn compact_banner_appears_under_selected_row_not_pinned_to_top() {
        let mut app = make_power_movie_list_app(vec!["First", "Second Selected", "Third"]);
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
        // Third item pushed down by the banner's reserved rows (content-
        // dependent since #263, not a fixed constant), so it should not
        // appear on the row immediately after the selected row.
        assert!(
            !lines[selected_row_line + 1].contains("Third"),
            "Third should not appear immediately after Second Selected:\n{out}"
        );
        let third_line = lines
            .iter()
            .position(|l| l.contains("Third"))
            .expect("Third item expected further down, pushed below the banner");
        let banner_panel_width = 40u16
            .saturating_sub(1)
            .saturating_sub(COMPACT_BANNER_INDENT);
        let expected_banner_rows = app.compact_banner_rows(0, banner_panel_width);
        assert!(
            expected_banner_rows > 0,
            "expected the selected movie to reserve banner rows"
        );
        assert!(
            third_line >= selected_row_line + expected_banner_rows,
            "Third expected pushed below the reserved banner rows (row {third_line}, selected row {selected_row_line}, reserved {expected_banner_rows}):\n{out}"
        );
    }

    // #263: the banner's reserved row budget must track the selected movie's
    // actual overview length, not a fixed constant -- a longer overview
    // should reserve more rows (and so push the rest of the list down
    // further) than a shorter one.
    #[test]
    fn compact_banner_rows_grows_with_a_longer_overview() {
        let mut app = make_power_movie_list_app(vec!["First", "Second Selected", "Third"]);
        app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
        let panel_width = 40u16
            .saturating_sub(1)
            .saturating_sub(COMPACT_BANNER_INDENT);

        app.libs[0].nav_stack.last_mut().unwrap().items[1].overview = "Short.".into();
        let short_rows = app.compact_banner_rows(0, panel_width);

        app.libs[0].nav_stack.last_mut().unwrap().items[1].overview = "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. ".repeat(6);
        let long_rows = app.compact_banner_rows(0, panel_width);

        assert!(
            long_rows > short_rows,
            "long overview ({long_rows} rows) should reserve more rows than short overview ({short_rows} rows)"
        );
    }

    // #263: the rest of the list must lay out around the banner's *actual*
    // rendered height -- a longer overview should visibly push later rows
    // further down than a shorter one, not just change an internal row
    // count that never reaches the screen.
    #[test]
    fn list_rows_lay_out_further_below_a_longer_overview_than_a_shorter_one() {
        let build = |overview: &str| {
            let mut app = make_power_movie_list_app(vec!["First", "Second Selected", "Third"]);
            app.libs[0].nav_stack.last_mut().unwrap().cursor = 1;
            app.libs[0].nav_stack.last_mut().unwrap().items[1].overview = overview.into();
            let mut layout = LayoutPower::default();
            let out = render_power_list_to_string_sized(&mut app, &mut layout, 40, 60);
            let lines: Vec<&str> = out.lines().collect();
            let selected_line = lines
                .iter()
                .position(|l| l.contains("Second Selected"))
                .expect("selected row should render");
            let third_line = lines
                .iter()
                .position(|l| l.contains("Third"))
                .expect("Third item should still be visible on screen");
            third_line - selected_line
        };

        let short_gap = build("Short.");
        let long_gap = build(
            &"Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. ".repeat(6),
        );

        assert!(
            long_gap > short_gap,
            "a longer overview should push Third further down (short gap {short_gap}, long gap {long_gap})"
        );
    }
}
