use super::super::super::ui_util::*;
use super::parse_album_folder_name;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Renders the combined music group view: a horizontal group-selector bar
    /// at the top (like the TV season bar) and the grouped-by-artist album
    /// list below.
    pub(super) fn render_power_music_group_view(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
    ) {
        if area.height == 0 {
            return;
        }

        // ── Collect nav state ─────────────────────────────────────────────────
        let stack_len = self.libs[lib_idx].nav_stack.len();
        let (groups, group_cursor, albums, album_cursor) = {
            let lib = &self.libs[lib_idx];
            let last = match lib.nav_stack.last() {
                Some(l) => l,
                None => return,
            };
            if stack_len >= 2 {
                let group_lvl = &lib.nav_stack[stack_len - 2];
                (
                    group_lvl.items.clone(),
                    group_lvl.cursor,
                    last.items.clone(),
                    last.cursor,
                )
            } else {
                (vec![], 0, last.items.clone(), last.cursor)
            }
        };

        let max_y = area.y + area.height;
        let mut row = area.y;

        // ── Group selector bar (blank · pills · blank) ───────────────────────
        // Blank spacer above pills.
        if row < max_y {
            row += 1;
        }

        // Pills row.
        if row < max_y && !groups.is_empty() {
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
            let bar_w = area.width as usize;

            // Greedy count: how many pills fit starting at `start` within `avail` chars.
            // Uses actual individual pill widths so short labels pack tightly.
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
            let mut x_cursor = area.x;
            if has_left {
                let chunk = "\u{2039} ";
                spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
                x_cursor += chunk.width() as u16;
            }
            for (idx, label) in tab_labels[scroll_start..scroll_end].iter().enumerate() {
                if idx > 0 {
                    spans.push(Span::raw(" "));
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
                        y: row,
                        width: pill.width() as u16,
                        height: 1,
                    },
                    abs_idx,
                ));
                spans.push(Span::styled(pill.clone(), style));
                x_cursor += pill.width() as u16;
            }
            if has_right {
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
            self.layout.power.selector_tabs = selector_tabs;
        }
        if row < max_y {
            row += 1;
        }

        // Blank spacer below pills.
        if row < max_y {
            row += 1;
        }

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
        self.layout.power.left_area = list_area;
        self.layout.power.left_sorted_indices.clear();
        self.layout.power.left_row_map.clear();

        let visible = list_area.height as usize;
        let avail_chars = (list_area.width as usize).saturating_sub(2);

        enum DisplayRow {
            ArtistHeader(String),
            Album(usize),
        }

        // Extract (artist, year, album_name) for each album item.
        let album_info: Vec<(String, String, String)> = albums
            .iter()
            .map(|item| {
                if !item.artist.is_empty() {
                    let year_str = if item.production_year > 0 {
                        item.production_year.to_string()
                    } else {
                        String::new()
                    };
                    (item.artist.clone(), year_str, item.display_name())
                } else if let Some((artist, year, album)) = parse_album_folder_name(&item.name) {
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

        // Build display rows: inject artist header at each artist boundary.
        let mut display_rows: Vec<DisplayRow> = Vec::new();
        let mut last_artist = String::new();
        for (idx, (artist, _, _)) in album_info.iter().enumerate() {
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
        self.layout.power.cursor_screen_y =
            Some(list_area.y + (display_cursor.saturating_sub(offset)) as u16);

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
