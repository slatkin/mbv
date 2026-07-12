use super::super::super::ui_util::*;
use super::{parse_album_folder_name, strip_article};
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::MediaItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Returns the group level's items and cursor for a music-group library
    /// (the nav-stack level above the current album level), if pushed yet.
    fn music_group_state(&self, lib_idx: usize) -> (Vec<MediaItem>, usize) {
        let lib = &self.libs[lib_idx];
        if lib.nav_stack.len() >= 2 {
            let group_lvl = &lib.nav_stack[lib.nav_stack.len() - 2];
            (group_lvl.items.clone(), group_lvl.cursor)
        } else {
            (Vec::new(), 0)
        }
    }

    /// Renders the music-group selector pills (with horizontal scroll
    /// indicators) inside `row_area`. Gaps between pills, and any unused
    /// trailing width, are filled with the same dash rule used by the
    /// standard breadcrumb header row, so the row still reads as the top
    /// divider underneath/between the pills. `row_area` must already be
    /// confined to the right column and exclude the fixed `Music` marker
    /// reserved by the caller (#180).
    pub(super) fn render_power_music_group_pills_row(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        lib_idx: usize,
        layout: &mut LayoutPower,
    ) {
        let (groups, group_cursor) = self.music_group_state(lib_idx);
        if groups.is_empty() || row_area.width == 0 {
            layout.selector_tabs = Vec::new();
            if row_area.width > 0 {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "\u{2500}".repeat(row_area.width as usize),
                        Style::default().fg(palette::FOAM),
                    ))),
                    row_area,
                );
            }
            return;
        }

        const MAX_LABEL: usize = 12;
        let tab_labels: Vec<String> = groups
            .iter()
            .map(|g| trunc_str(&g.name, MAX_LABEL).to_string())
            .collect();
        let n_tabs = tab_labels.len();
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();

        // Actual display width of each pill: " label " = label_w + 2.
        // Gap between consecutive pills = 1 char.
        let pill_widths: Vec<usize> = tab_labels.iter().map(|l| l.width() + 2).collect();
        let bar_w = row_area.width as usize;

        // Greedy count: how many pills fit starting at `start` within `avail` chars.
        let count_fitting = |start: usize, avail: usize| -> usize {
            let mut used = 0usize;
            let mut count = 0usize;
            for width in pill_widths.iter().take(n_tabs).skip(start) {
                let need = if count == 0 { *width } else { 1 + *width };
                if used + need > avail {
                    break;
                }
                used += need;
                count += 1;
            }
            count
        };

        // Walk scroll_start forward until group_cursor is in the visible window.
        let mut scroll_start = 0usize;
        loop {
            let avail = bar_w
                .saturating_sub(if scroll_start > 0 { 2 } else { 0 }) // "‹ "
                .saturating_sub(2); // reserve for " ›"
            let cnt = count_fitting(scroll_start, avail);
            if cnt == 0 || scroll_start + cnt > group_cursor {
                break;
            }
            scroll_start += 1;
        }

        let has_left = scroll_start > 0;
        let avail_pills = bar_w
            .saturating_sub(if has_left { 2 } else { 0 })
            .saturating_sub(2); // reserve for " ›"
        let cnt = count_fitting(scroll_start, avail_pills);
        let scroll_end = (scroll_start + cnt).min(n_tabs);
        let has_right = scroll_end < n_tabs;

        let mut spans: Vec<Span> = Vec::new();
        let mut x_cursor = row_area.x;
        if has_left {
            let chunk = "\u{2039} ";
            spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
            x_cursor += chunk.width() as u16;
        }
        for (idx, label) in tab_labels[scroll_start..scroll_end].iter().enumerate() {
            if idx > 0 {
                // Dash rule (not a blank space) so the top divider still
                // reads as continuous underneath/between the pills.
                spans.push(Span::styled("\u{2500}", Style::default().fg(palette::FOAM)));
                x_cursor += 1;
            }
            let abs_idx = scroll_start + idx;
            let selected = abs_idx == group_cursor;
            let style = if selected {
                Style::default()
                    .fg(palette::YELLOW)
                    .bg(palette::FOAM)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::BASE).bg(palette::FOAM)
            };
            let pill = format!(" {} ", label);
            selector_tabs.push((
                Rect {
                    x: x_cursor,
                    y: row_area.y,
                    width: pill.width() as u16,
                    height: 1,
                },
                abs_idx,
            ));
            spans.push(Span::styled(pill.clone(), style));
            x_cursor += pill.width() as u16;
        }
        if has_right {
            let chunk = " \u{203a}";
            spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
            x_cursor += chunk.width() as u16;
        }

        // Fill any remaining width with the standard dash rule so the row
        // still reads as the top divider when the pills don't fill it.
        let used_w = (x_cursor - row_area.x) as usize;
        if used_w < bar_w {
            spans.push(Span::styled(
                "\u{2500}".repeat(bar_w - used_w),
                Style::default().fg(palette::FOAM),
            ));
        }

        f.render_widget(Paragraph::new(Line::from(spans)), row_area);
        layout.selector_tabs = selector_tabs;
    }

    /// Renders the grouped-by-artist album list for a music group library. The
    /// group-selector pills for this view are rendered by the caller on the
    /// power view's top rule row instead (`render_power_music_group_pills_row`,
    /// see #180) -- this method starts directly with the album list.
    pub(super) fn render_power_music_group_view(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }

        // ── Collect nav state ─────────────────────────────────────────────────
        let (albums, album_cursor) = {
            let lib = &self.libs[lib_idx];
            let last = match lib.nav_stack.last() {
                Some(l) => l,
                None => return,
            };
            (last.items.clone(), last.cursor)
        };

        let max_y = area.y + area.height;
        let row = area.y;

        // ── Loading / empty state ─────────────────────────────────────────────
        if albums.is_empty() {
            if row < max_y {
                let is_loading = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.loading)
                    .unwrap_or(false);
                let msg = if is_loading {
                    " Loading\u{2026}"
                } else {
                    " (empty)"
                };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        msg,
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

        // ── Album list (grouped by artist) ────────────────────────────────────
        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        if list_area.height == 0 {
            return;
        }

        // Store for click/page-size calculations (used by mouse handler and PageUp/Down).
        layout.left_area = list_area;

        let visible = list_area.height as usize;
        let avail_chars = (list_area.width as usize).saturating_sub(2);

        enum DisplayRow {
            ArtistHeader(String),
            Album(usize),
        }

        // Extract (artist, year, album_name) for each album item. Artist
        // resolution (Emby tag → fetched-from-first-tracks cache → folder-name
        // guess → "Unknown Artist") is centralized in
        // `resolve_group_album_artist` since it may need to kick off an async
        // fetch, hence the explicit loop instead of a pure `.map()`.
        let mut album_info: Vec<(String, String, String)> = Vec::with_capacity(albums.len());
        for item in &albums {
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

        // Build display rows: inject artist header at each artist boundary.
        // Albums arrive sorted by album name (SortName), not by artist, so a
        // stable sort by artist is needed first — otherwise the same artist
        // (e.g. "Unknown Artist" for tag-less compilations) resurfaces as a
        // new header every time it's interrupted by a different album name.
        let mut order: Vec<usize> = (0..album_info.len()).collect();
        order.sort_by_key(|&i| natural_sort_key(strip_article(&album_info[i].0)));

        // Publish the sorted order so keyboard cursor movement (PageUp/Down,
        // Home/End — see `move_lib_cursor`/`jump_lib_cursor` in actions.rs)
        // follows display order rather than the raw (SortName-by-album-title)
        // order `albums` arrived in — otherwise arrow keys jump the cursor to
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

        // Locate the cursor row and keep the saved viewport offset stable.
        // Only move the viewport when the selected album would leave it.
        let display_cursor = display_rows
            .iter()
            .position(|r| matches!(r, DisplayRow::Album(i) if *i == album_cursor))
            .unwrap_or(0);
        let max_offset = display_rows.len().saturating_sub(visible);
        let mut offset = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|lvl| lvl.scroll)
            .unwrap_or(0)
            .min(max_offset);
        if display_cursor < offset {
            offset = display_cursor;
        } else if display_cursor >= offset + visible {
            offset = display_cursor.saturating_sub(visible.saturating_sub(1));
        }
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.scroll = offset;
        }

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

        let list_items: Vec<ListItem> = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|dr| match dr {
                DisplayRow::ArtistHeader(name) => {
                    let artist_label = trunc_str(name, avail_chars);
                    ListItem::new(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(artist_label, Style::default().fg(palette::YELLOW)),
                    ]))
                }
                DisplayRow::Album(idx) => {
                    let selected = *idx == album_cursor;
                    let (_, year_str, album_name) = &album_info[*idx];
                    let prefix_w = if year_str.is_empty() {
                        3
                    } else {
                        year_str.len() + 6
                    };
                    let name_w = avail_chars.saturating_sub(prefix_w);
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
                        spans.push(Span::styled("\u{258c}", Style::default().fg(palette::PINE)));
                    } else {
                        spans.push(Span::raw(" "));
                    }
                    if year_str.is_empty() {
                        spans.push(Span::raw("   "));
                    } else {
                        spans.push(Span::styled("   (", Style::default().fg(palette::SUBTLE)));
                        spans.push(Span::styled(
                            year_str.clone(),
                            Style::default().fg(palette::PINE),
                        ));
                        spans.push(Span::styled(") ", Style::default().fg(palette::SUBTLE)));
                    }
                    let name_style = if selected && focused {
                        Style::default().fg(name_color).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(name_color)
                    };
                    spans.push(Span::styled(trunc_name.to_string(), name_style));
                    ListItem::new(Line::from(spans))
                }
            })
            .collect();

        let mut state = ListState::default();
        state.select(Some(display_cursor.saturating_sub(offset)));
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            list_area,
            &mut state,
        );
        layout.cursor_screen_y = Some(list_area.y + (display_cursor.saturating_sub(offset)) as u16);

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
                list_area,
                &mut sb,
            );
        }
    }
}
