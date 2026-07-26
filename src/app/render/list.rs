use super::super::ui_util::*;
use super::detail::compact_banner_image_cache_key;
use super::{effective_sort_str, letter_bucket};
use crate::app::layout::LayoutMain;
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
/// are wrapped in a colored block — `palette::MEDIA_SELECTED_BG` when focused,
/// `palette::PLAYBACK_PANEL_BG` when unfocused — a dark (#282828 / #151515)
/// background visually similar to the home tab's Keep Watching
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

enum DisplayRow {
    Spacer,
    LetterHeader(String),
    Item(usize),
    BannerFiller,
    SeriesDetailFiller,
}

fn push_selected_detail_fillers_before(
    rows: &mut Vec<DisplayRow>,
    item_idx: usize,
    cursor: usize,
    banner_rows: usize,
    series_detail_rows: usize,
) {
    if banner_rows > 0 && item_idx == cursor {
        rows.push(DisplayRow::BannerFiller);
        rows.push(DisplayRow::BannerFiller);
    }
    if series_detail_rows > 0 && item_idx == cursor {
        rows.push(DisplayRow::SeriesDetailFiller);
        rows.push(DisplayRow::SeriesDetailFiller);
    }
}

fn push_selected_detail_fillers_after(
    rows: &mut Vec<DisplayRow>,
    item_idx: usize,
    cursor: usize,
    banner_rows: usize,
    series_detail_rows: usize,
) {
    if banner_rows > 0 && item_idx == cursor {
        for _ in 0..banner_rows.saturating_sub(2) {
            rows.push(DisplayRow::BannerFiller);
        }
        rows.push(DisplayRow::BannerFiller);
        rows.push(DisplayRow::BannerFiller);
    }
    if series_detail_rows > 0 && item_idx == cursor {
        for _ in 0..series_detail_rows {
            rows.push(DisplayRow::SeriesDetailFiller);
        }
    }
}

fn selected_detail_lower_bound(
    display_cursor: usize,
    banner_rows: usize,
    series_detail_rows: usize,
    visible: usize,
) -> usize {
    let rows_below_cursor = banner_rows.max(series_detail_rows);
    (display_cursor + rows_below_cursor)
        .saturating_sub(visible.saturating_sub(1))
        .min(display_cursor)
}

/// Builds the title (+ optional duration) spans for one list row, shared by
/// both the letter-grouped and plain-list rendering branches (identical
/// styling logic, only how `title`/`dur_str`/`avail` are computed differs
/// between the two call sites).
fn build_list_row_spans(
    title: String,
    dur_str: String,
    selected: bool,
    selected_has_banner: bool,
    is_series: bool,
    focused: bool,
    fg: Color,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = if selected {
        if selected_has_banner {
            // Colored-block look: 1-col leading pad inside the
            // MEDIA_SELECTED_BG block, no green `▌` gutter. Title is Emby
            // green (BOLD when focused) and the row omits the duration --
            // it lives in the banner's metadata row below.
            let title_style = if focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            vec![Span::raw(" "), Span::styled(title, title_style)]
        } else if is_series {
            // Series inline detail: title is yellow when selected.
            let title_style = if focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            vec![Span::raw(" "), Span::styled(title, title_style)]
        } else {
            // Otherwise keep the green gutter for selected list rows
            // without an inline banner.
            let title_style = if focused {
                Style::default()
                    .fg(palette::IRIS)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };
            vec![
                super::selection_marker(true),
                Span::styled(title, title_style),
            ]
        }
    } else {
        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
    };
    if !selected_has_banner && !dur_str.is_empty() {
        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
    }
    spans
}

/// Paints the series inline detail block's colored background, shared by
/// both the letter-grouped and plain-list rendering branches of
/// `render_power_list` (identical treatment, only how `display_cursor` /
/// `offset` / `visible` are computed differs between the two call sites).
/// The colored block starts at the spacer row above the selected item and runs
/// through the spacer row below the episode list; the SeriesDetailFiller top
/// border (▁) and the bottom border (▔, drawn inside `render_series_inline_detail`)
/// are left uncolored so they blend into the existing background.
fn render_series_detail_background(
    f: &mut Frame,
    content_area: Rect,
    offset: usize,
    visible: usize,
    display_cursor: usize,
    series_detail_rows: usize,
    focused: bool,
) {
    if series_detail_rows == 0 {
        return;
    }
    let series_rule_top = display_cursor.saturating_sub(1);
    let series_rule_bottom = display_cursor + series_detail_rows.saturating_sub(1);
    let bg = if focused {
        palette::MEDIA_SELECTED_BG
    } else {
        palette::PLAYBACK_PANEL_BG
    };
    super::render_selected_block_background(
        f,
        content_area,
        offset,
        visible,
        series_rule_top,
        series_rule_bottom,
        bg,
    );
}

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

    fn render_series_detail_if_visible(
        &mut self,
        f: &mut Frame,
        content_area: Rect,
        offset: usize,
        visible: usize,
        display_cursor: usize,
        series_detail_rows: usize,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if series_detail_rows == 0 {
            return;
        }
        let content_start = display_cursor + 1;
        if content_start < offset || content_start >= offset + visible {
            return;
        }

        let detail_y = content_area.y + (content_start - offset) as u16;
        let bottom = content_area.y + content_area.height;
        let detail_h = (series_detail_rows as u16).min(bottom.saturating_sub(detail_y));
        if detail_h == 0 {
            return;
        }

        self.render_series_inline_detail(
            f,
            Rect {
                x: content_area.x + COMPACT_BANNER_INDENT,
                y: detail_y,
                width: content_area.width.saturating_sub(2 * COMPACT_BANNER_INDENT),
                height: detail_h,
            },
            lib_idx,
            focused,
            layout,
        );
    }

    fn render_series_detail_top_border(
        f: &mut Frame,
        content_area: Rect,
        offset: usize,
        visible: usize,
        display_cursor: usize,
        series_detail_rows: usize,
    ) {
        if series_detail_rows == 0
            || display_cursor < 2
            || display_cursor - 2 < offset
            || display_cursor - 2 >= offset + visible
        {
            return;
        }

        let border_y = content_area.y + (display_cursor - 2 - offset) as u16;
        f.render_widget(
            Paragraph::new(Span::styled(
                "\u{2581}".repeat(content_area.width as usize),
                Style::default().fg(palette::SEEK_TRACK),
            )),
            Rect {
                x: content_area.x,
                y: border_y,
                width: content_area.width,
                height: 1,
            },
        );
    }

    /// Renders the Continue/library list items into `area`.
    /// The title header is now drawn in the top-of-screen FOAM bar by `render_power_view`.
    pub(super) fn render_power_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        // Ensure the library is loaded when a library tab is selected.
        if self.library_tab > 0 {
            self.ensure_lib_loaded_for(self.library_tab - 1);
        }

        let mut content_area = area;

        // Store for click / page-size calculations.
        layout.left_area = content_area;

        // Gather items, cursor, stored scroll offset, and the *true* library total
        // (not just how many pages have been fetched so far) from the appropriate
        // source.
        let (items, cursor, stored_scroll, total_count) = if self.library_tab == 0 {
            let items = self.home.continue_items.clone();
            let cursor = self.home.continue_cursor.min(items.len().saturating_sub(1));
            let total = items.len();
            (items, cursor, 0usize, total)
        } else {
            let lib_idx = self.library_tab - 1;
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
        let banner_rows: usize = if self.library_tab > 0 {
            let banner_panel_width = content_area
                .width
                .saturating_sub(1)
                .saturating_sub(COMPACT_BANNER_INDENT);
            self.compact_banner_rows(self.library_tab - 1, banner_panel_width)
        } else {
            0
        };
        // Content-only row count (banner_rows minus its top/bottom colored-pad
        // filler rows), used below to size the banner rect to the same
        // content-dependent height that was reserved for it above.
        let banner_content_rows: usize =
            banner_rows.saturating_sub(COMPACT_BANNER_RULE_ROWS + COMPACT_BANNER_GAP_ROWS);

        // Series inline detail rows: when a TV show Series is selected,
        // show its metadata/overview inline below the selected row.
        let series_detail_rows: usize = if self.library_tab > 0 && banner_rows == 0 {
            let lib_idx = self.library_tab - 1;
            if let Some(item) = self.power_selected_series_item(lib_idx) {
                let panel_width = content_area
                    .width
                    .saturating_sub(1)
                    .saturating_sub(COMPACT_BANNER_INDENT);
                let (in_selection, episode_count) = self.series_selection_state(lib_idx, &item.id);
                self.series_inline_detail_rows(&item, panel_width, in_selection, episode_count)
            } else {
                0
            }
        } else {
            0
        };

        // Pre-warm nearby movies' poster images so they're already cached by
        // the time the cursor reaches them (#287) -- mirrors the prefetch
        // window `render_power_card` already uses for the home-card
        // carousel. Only applies when a movie banner is actually showing
        // (i.e. this is a movies library with a leaf Movie selected); if
        // there's no banner, there's nothing to prefetch for.
        if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
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
        let show_grouped = if self.library_tab > 0 {
            self.is_viewing_album_folders(self.library_tab - 1)
        } else {
            false
        };

        let n = items.len();

        // Letter grouping: applies to non-music library lists with 50+ items (not during search).
        // Gated on the true library total (`LibraryTab.library_total` when known,
        // e.g. a letter-range pill has scoped the fetch to a smaller slice),
        // not the fetched-so-far/filtered count, so the grouping style (ranges
        // vs. individual letters) doesn't change out from under the user as
        // more pages lazily load in, and a small filtered slice (< 50 items)
        // still shows headers.
        let active_letter_filter = if self.library_tab > 0 {
            self.libs[self.library_tab - 1]
                .nav_stack
                .last()
                .and_then(|l| l.letter_filter.as_ref())
                .cloned()
        } else {
            None
        };
        let ungrouped_total = self
            .library_tab
            .checked_sub(1)
            .map_or(total_count, |lib_idx| {
                self.libs[lib_idx].library_total.unwrap_or(total_count)
            });
        let use_letter_groups = !show_grouped
            && self.library_tab > 0
            && (ungrouped_total >= 50 || active_letter_filter.is_some())
            && {
                let lib_idx = self.library_tab - 1;
                self.libs[lib_idx].library.collection_type != "music"
                    && self.libs[lib_idx].search.is_none()
            };

        // First row area: search input box (when searching).
        if focused && self.library_tab > 0 && content_area.height > 0 {
            let lib_idx = self.library_tab - 1;
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
                    Paragraph::new(Span::styled(
                        input_text,
                        Style::default().fg(palette::GREEN),
                    ))
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
            }
        }

        if n == 0 {
            let msg = if self.library_tab > 0 {
                let lib_idx = self.library_tab - 1;
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
            super::render_power_placeholder(f, content_area, msg);
            return;
        }

        let visible = content_area.height as usize;
        let final_offset: usize;

        if show_grouped {
            let lib_idx = self.library_tab - 1;
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
            // Sort item indices by the same effective key used for bucketing so that
            // items within each group appear in article-stripped alphabetical order.
            let mut sorted_indices: Vec<usize> = (0..n).collect();
            sorted_indices.sort_by_key(|&i| natural_sort_key(effective_sort_str(&items[i])));
            // Publish the sorted order so cursor navigation can follow display order.
            layout.left_sorted_indices = sorted_indices.clone();

            // With a letter-range pill active, the visible slice is already
            // narrowed to one range (e.g. `A–C`) -- bucket by the individual
            // first letter within it (`A`, `B`, `C`) rather than re-deriving
            // a range bucket from the slice's own (small) size. Forcing
            // `letter_bucket`'s `total >= 250` branch reuses its existing
            // per-letter logic without a second code path.
            let bucket_total = if active_letter_filter.is_some() {
                usize::MAX
            } else {
                ungrouped_total
            };
            let mut display_rows: Vec<DisplayRow> = Vec::new();
            let mut last_bucket = String::new();
            for &idx in &sorted_indices {
                let item = &items[idx];
                let bucket = letter_bucket(item, bucket_total);
                if bucket != last_bucket {
                    if !last_bucket.is_empty() {
                        display_rows.push(DisplayRow::Spacer);
                    }
                    display_rows.push(DisplayRow::LetterHeader(bucket.clone()));
                    last_bucket = bucket;
                }
                push_selected_detail_fillers_before(
                    &mut display_rows,
                    idx,
                    cursor,
                    banner_rows,
                    series_detail_rows,
                );
                display_rows.push(DisplayRow::Item(idx));
                push_selected_detail_fillers_after(
                    &mut display_rows,
                    idx,
                    cursor,
                    banner_rows,
                    series_detail_rows,
                );
            }
            let total_display = display_rows.len();

            // Find the visual row of the current cursor item for scrolling.
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
                .unwrap_or(0);
            // For banners, `banner_rows` rows sit below the cursor (opening rule above).
            // For series, `series_detail_rows` rows sit below the cursor (block follows it).
            let lower_bound = selected_detail_lower_bound(
                display_cursor,
                banner_rows,
                series_detail_rows,
                visible,
            );
            let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
            // If stale scroll state would put the first item of a bucket at the
            // top of the viewport, back up so its letter header remains visible.
            // When that item is also the selected/bannered one, the banner's
            // opening rule sits between the header and the item, so back up an
            // extra row to clear the rule too.
            // Also, if a colored-padding BannerFiller (from a selected block) is at
            // the top, back up one row to keep the border-space BannerFiller visible.
            if visible > 1 && offset > 0 {
                // Walk back over any run of banner/series-detail filler rows
                // touching `offset` (whether `offset` lands inside the run or
                // right after it, on the Item row the fillers belong to) so
                // `run_start` points at the run's first row. Then, if a
                // LetterHeader immediately precedes that run, include it too
                // -- otherwise the header (and/or the block's top padding)
                // can be scrolled just out of view with no way back except
                // scrolling further. This generalizes the old fixed-offset
                // special cases to any filler-run length.
                let mut run_start = offset;
                while run_start > 0
                    && matches!(
                        display_rows.get(run_start - 1),
                        Some(DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller)
                    )
                {
                    run_start -= 1;
                }
                offset = if run_start > 0
                    && matches!(
                        display_rows.get(run_start - 1),
                        Some(DisplayRow::LetterHeader(_))
                    ) {
                    run_start - 1
                } else {
                    run_start
                };
            }
            final_offset = offset;

            // Build row map so mouse clicks can map visual row → item index.
            for row in display_rows.iter().skip(offset).take(visible) {
                layout.left_row_map.push(match row {
                    DisplayRow::Spacer
                    | DisplayRow::LetterHeader(_)
                    | DisplayRow::BannerFiller
                    | DisplayRow::SeriesDetailFiller => None,
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

            // The selected movie + banner are wrapped in a colored block
            // (matching the home tab's Keep Watching look). Draw the block
            // first, before the list items, so the per-row spans only paint
            // their own cells and the block's background shows through on the
            // side padding cols and on the top/bottom padding rows.
            if banner_rows > 0 {
                let bg = if focused {
                    palette::MEDIA_SELECTED_BG
                } else {
                    palette::PLAYBACK_PANEL_BG
                };
                super::render_selected_block_background(
                    f,
                    content_area,
                    offset,
                    visible,
                    banner_rule_top,
                    banner_rule_bottom,
                    bg,
                );
            }

            render_series_detail_background(
                f,
                content_area,
                offset,
                visible,
                display_cursor,
                series_detail_rows,
                focused,
            );

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
                    // empty -- they show the block's background.
                    DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller => {
                        ListItem::new(Line::default())
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
                        let is_series = item.item_type == "Series";
                        let spans = build_list_row_spans(
                            title,
                            dur_str,
                            selected,
                            selected_has_banner,
                            is_series,
                            focused,
                            fg,
                        );
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
                        self.library_tab - 1,
                        focused,
                        layout,
                    );
                    layout.cursor_screen_y = want_cursor_y;
                }
            }

            self.render_series_detail_if_visible(
                f,
                content_area,
                offset,
                visible,
                display_cursor,
                series_detail_rows,
                self.library_tab - 1,
                focused,
                layout,
            );

            if show_scrollbar {
                let max_off = total_display.saturating_sub(visible);
                super::render_power_right_scrollbar(f, content_area, max_off, offset);
            }

            if banner_rows > 0 {
                super::render_selected_block_borders(
                    f,
                    content_area,
                    offset,
                    visible,
                    banner_rule_top,
                    banner_rule_bottom,
                );
            }

            Self::render_series_detail_top_border(
                f,
                content_area,
                offset,
                visible,
                display_cursor,
                series_detail_rows,
            );
        } else {
            let mut display_rows: Vec<DisplayRow> =
                Vec::with_capacity(n + banner_rows + series_detail_rows);
            for i in 0..n {
                push_selected_detail_fillers_before(
                    &mut display_rows,
                    i,
                    cursor,
                    banner_rows,
                    series_detail_rows,
                );
                display_rows.push(DisplayRow::Item(i));
                push_selected_detail_fillers_after(
                    &mut display_rows,
                    i,
                    cursor,
                    banner_rows,
                    series_detail_rows,
                );
            }
            let total_display = display_rows.len();
            let display_cursor = display_rows
                .iter()
                .position(|r| matches!(r, DisplayRow::Item(i) if *i == cursor))
                .unwrap_or(0);

            // Lower bound normally just keeps the cursor row visible; when a
            // banner or series detail follows it, extend the lower bound so
            // scrolling keeps pulling up until the whole block is visible too
            // (clamped to display_cursor itself if the viewport could never fit both).
            // For banners, `banner_rows` rows sit below the cursor (opening rule above).
            // For series, `series_detail_rows` rows sit below the cursor (block follows it).
            let lower_bound = selected_detail_lower_bound(
                display_cursor,
                banner_rows,
                series_detail_rows,
                visible,
            );
            let mut offset = stored_scroll.clamp(lower_bound, display_cursor);
            // Walk back over any run of banner/series-detail filler rows
            // touching `offset` (whether `offset` lands inside the run or
            // right after it, on the Item row the fillers belong to) so the
            // offset lands at the run's first row instead of stranding the
            // block's top padding just out of view. See the analogous,
            // letter-header-aware version of this scan above for
            // `use_letter_groups`; this (ungrouped) branch never has
            // LetterHeader rows, so there's nothing further to include.
            if visible > 1 && offset > 0 {
                while offset > 0
                    && matches!(
                        display_rows.get(offset - 1),
                        Some(DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller)
                    )
                {
                    offset -= 1;
                }
            }
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
                let bg = if focused {
                    palette::MEDIA_SELECTED_BG
                } else {
                    palette::PLAYBACK_PANEL_BG
                };
                super::render_selected_block_background(
                    f,
                    content_area,
                    offset,
                    visible,
                    banner_rule_top,
                    banner_rule_bottom,
                    bg,
                );
            }

            render_series_detail_background(
                f,
                content_area,
                offset,
                visible,
                display_cursor,
                series_detail_rows,
                focused,
            );

            let list_items: Vec<ListItem> = display_rows
                .iter()
                .enumerate()
                .skip(offset)
                .take(visible)
                .map(|(_abs_idx, row)| match row {
                    DisplayRow::Spacer | DisplayRow::LetterHeader(_) => {
                        ListItem::new(Line::default())
                    }
                    // The colored block (drawn above) frames the selected row
                    // + banner, so the banner's top/bottom padding rows are
                    // empty -- they show the block's background.
                    DisplayRow::BannerFiller | DisplayRow::SeriesDetailFiller => {
                        ListItem::new(Line::default())
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

                        let is_series = item.item_type == "Series";
                        let spans = build_list_row_spans(
                            title,
                            dur_str,
                            selected,
                            selected_has_banner,
                            is_series,
                            focused,
                            fg,
                        );
                        ListItem::new(Line::from(spans))
                    }
                })
                .collect();

            layout.left_row_map = display_rows
                .iter()
                .skip(offset)
                .take(visible)
                .map(|row| match row {
                    DisplayRow::Spacer
                    | DisplayRow::LetterHeader(_)
                    | DisplayRow::BannerFiller
                    | DisplayRow::SeriesDetailFiller => None,
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
                        self.library_tab - 1,
                        focused,
                        layout,
                    );
                    layout.cursor_screen_y = want_cursor_y;
                }
            }

            self.render_series_detail_if_visible(
                f,
                content_area,
                offset,
                visible,
                display_cursor,
                series_detail_rows,
                self.library_tab - 1,
                focused,
                layout,
            );

            if show_scrollbar {
                let max_off = total_display.saturating_sub(visible);
                super::render_power_right_scrollbar(f, content_area, max_off, offset);
            }

            // White unicode borders at the block's top and bottom padding
            // rows, rendering inside the coloured block.
            if banner_rows > 0 {
                super::render_selected_block_borders(
                    f,
                    content_area,
                    offset,
                    visible,
                    banner_rule_top,
                    banner_rule_bottom,
                );
            }

            Self::render_series_detail_top_border(
                f,
                content_area,
                offset,
                visible,
                display_cursor,
                series_detail_rows,
            );
        }

        // Persist the scroll offset so the viewport is remembered across frames.
        // library_tab is always > 0 here (tab == 0 uses render_power_home_list).
        if self.library_tab > 0 {
            let lib_idx = self.library_tab - 1;
            if let Some(s) = &mut self.libs[lib_idx].search {
                s.scroll = final_offset;
            } else if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
                lvl.scroll = final_offset;
            }
        }
    }
}

#[cfg(test)]
#[path = "list_tests.rs"]
mod tests;
