use super::super::super::ui_util::*;
use crate::api::TICKS_PER_SECOND;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

impl App {
    pub(super) fn render_power_home_video_list(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
    ) {
        if area.height == 0 {
            return;
        }
        self.ensure_lib_loaded_for(lib_idx);

        let mut content_area = area;
        self.power_left_area = content_area;

        let (items, cursor) = {
            let lib = &self.libs[lib_idx];
            match lib.nav_stack.last() {
                Some(lvl) => (lvl.items.clone(), lvl.cursor),
                None => return,
            }
        };

        let n = items.len();

        // Item count label (matches render_power_list style).
        if focused && content_area.height > 0 {
            let count_label = format!(" {} items", n);
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

        if n == 0 {
            return;
        }

        let is_feed_lib = {
            let c = self.client.lock().unwrap();
            c.config
                .feed_view_libraries
                .contains(&self.libs[lib_idx].library.name.to_lowercase())
        };

        const MONTHS: [&str; 12] = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];

        // Each item: title row + meta row + separator = 3 rows; +1 if it has an overview.
        let item_heights: Vec<u16> = items
            .iter()
            .map(|item| if item.overview.is_empty() { 3 } else { 4 })
            .collect();

        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > content_area.height;
        let text_w =
            (content_area.width as usize).saturating_sub(if needs_scrollbar { 1 } else { 0 });

        // Scroll so the cursor item is always visible.
        let scroll = {
            let mut s = 0usize;
            while s < cursor {
                let visible_h: u16 = item_heights[s..=cursor].iter().sum();
                if visible_h <= content_area.height {
                    break;
                }
                s += 1;
            }
            s
        };

        let mut row_y = content_area.y;

        for (i, item) in items.iter().enumerate().skip(scroll) {
            if row_y >= content_area.y + content_area.height {
                break;
            }
            let item_h = item_heights[i];
            let selected = i == cursor;

            // Cursor marker
            let marker = if selected && focused {
                Span::styled("\u{258c}", Style::default().fg(palette::PINE))
            } else {
                Span::raw(" ")
            };
            f.render_widget(
                Paragraph::new(marker),
                Rect {
                    x: content_area.x,
                    y: row_y,
                    width: 1,
                    height: 1,
                },
            );

            let tx = content_area.x + 1;
            let tw = (text_w.saturating_sub(1)) as u16;

            // — Title —
            let title_color = if selected && focused {
                palette::IRIS
            } else {
                palette::TEXT
            };
            let title_style = if selected && focused {
                Style::default()
                    .fg(title_color)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(title_color)
            };
            let title_trunc = trunc_str(&item.display_name(), tw as usize);
            f.render_widget(
                Paragraph::new(Span::styled(title_trunc, title_style)),
                Rect {
                    x: tx,
                    y: row_y,
                    width: tw,
                    height: 1,
                },
            );

            // — Meta line: date added / duration / playback % —
            if row_y + 1 < content_area.y + content_area.height {
                let mut meta_spans: Vec<Span> = Vec::new();
                if item.played {
                    meta_spans.push(Span::styled(
                        "\u{2713} ",
                        Style::default().fg(palette::PINE),
                    ));
                }
                let mut parts: Vec<String> = Vec::new();
                if is_feed_lib && !item.date_added.is_empty() {
                    let formatted = item
                        .date_added
                        .splitn(3, '-')
                        .collect::<Vec<_>>()
                        .as_slice()
                        .windows(3)
                        .next()
                        .and_then(|p| {
                            let y = p[0];
                            let d: u32 = p[2].parse().ok()?;
                            let m: usize = p[1].parse::<usize>().ok()?.checked_sub(1)?;
                            Some(format!("Added {} {}, {}", d, MONTHS.get(m)?, y))
                        })
                        .unwrap_or_else(|| item.date_added.clone());
                    parts.push(formatted);
                }
                let dur_s = item.runtime_ticks / crate::api::TICKS_PER_SECOND;
                if dur_s > 0 {
                    parts.push(fmt_duration_approx(dur_s));
                }
                if !parts.is_empty() {
                    meta_spans.push(Span::styled(
                        parts.join("  "),
                        Style::default().fg(palette::SUBTLE),
                    ));
                }
                if item.playback_position_ticks > 0 && !item.played && item.runtime_ticks > 0 {
                    let pct =
                        (item.playback_position_ticks * 100 / item.runtime_ticks.max(1)) as u64;
                    meta_spans.push(Span::styled(
                        format!("  {}%", pct),
                        Style::default().fg(palette::YELLOW),
                    ));
                }
                f.render_widget(
                    Paragraph::new(Line::from(meta_spans)),
                    Rect {
                        x: tx,
                        y: row_y + 1,
                        width: tw,
                        height: 1,
                    },
                );
            }

            // — Overview (first wrapped line) —
            if !item.overview.is_empty()
                && item_h >= 4
                && row_y + 2 < content_area.y + content_area.height
            {
                {
                    let ov_text = trunc_overview(&item.overview);
                    let ov_first = wrap(&ov_text, (tw as usize).max(1))
                        .into_iter()
                        .next()
                        .map(|s| s.into_owned())
                        .unwrap_or_default();
                    let ov_color = if selected && focused {
                        palette::WHITE
                    } else {
                        palette::MUTED
                    };
                    f.render_widget(
                        Paragraph::new(Span::styled(ov_first, Style::default().fg(ov_color))),
                        Rect {
                            x: tx,
                            y: row_y + 2,
                            width: tw,
                            height: 1,
                        },
                    );
                }
            }

            // — Separator —
            let sep_y = row_y + item_h - 1;
            if sep_y < content_area.y + content_area.height {
                let sep_str = "\u{2500}".repeat(text_w);
                f.render_widget(
                    Paragraph::new(Span::styled(sep_str, Style::default().fg(palette::MUTED))),
                    Rect {
                        x: content_area.x,
                        y: sep_y,
                        width: text_w as u16,
                        height: 1,
                    },
                );
            }

            row_y += item_h;
        }

        // Scrollbar (hidden when unfocused, consistent with queue panel).
        if needs_scrollbar && focused {
            let max_off = n.saturating_sub(1);
            let mut sb = ScrollbarState::new(max_off + 1).position(scroll);
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

    pub(super) fn render_power_home_list(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        if area.height == 0 {
            return;
        }

        // Build flat item list and display rows.
        // DisplayRow::Header → yellow bold section label (not selectable).
        // DisplayRow::Item(flat_idx) → item row, selected when flat_idx == power_home_cursor.
        enum HomeRow {
            Spacer,
            Header(String),
            Item(usize),
        }

        let continue_items = self.home.continue_items.clone();
        let latest = self.home.latest.clone();

        let total: usize = continue_items.len()
            + latest
                .iter()
                .map(|(_, _, items, _)| items.len())
                .sum::<usize>();

        if total == 0 {
            f.render_widget(
                Paragraph::new(Span::styled(
                    "(nothing)",
                    Style::default().fg(palette::MUTED),
                )),
                area,
            );
            return;
        }

        let cursor = self.home.power_home_cursor.min(total - 1);

        let mut display_rows: Vec<HomeRow> = Vec::new();
        let mut flat_idx = 0usize;
        let mut first_group = true;

        // Continue Watching group.
        if !continue_items.is_empty() {
            display_rows.push(HomeRow::Header("Keep Watching".to_string()));
            for _ in &continue_items {
                display_rows.push(HomeRow::Item(flat_idx));
                flat_idx += 1;
            }
            first_group = false;
        }

        // Per-library latest groups — blank spacer before each header except the very first.
        for (title, _, items, _) in &latest {
            if items.is_empty() {
                continue;
            }
            if !first_group {
                display_rows.push(HomeRow::Spacer);
            }
            display_rows.push(HomeRow::Header(title.clone()));
            for _ in items {
                display_rows.push(HomeRow::Item(flat_idx));
                flat_idx += 1;
            }
            first_group = false;
        }

        // Build a combined flat items vec for rendering.
        let mut flat_items: Vec<crate::api::MediaItem> = Vec::with_capacity(total);
        flat_items.extend(continue_items.iter().cloned());
        for (_, _, items, _) in &latest {
            flat_items.extend(items.iter().cloned());
        }

        // Locate the display row for the current cursor.
        let display_cursor = display_rows
            .iter()
            .position(|r| matches!(r, HomeRow::Item(i) if *i == cursor))
            .unwrap_or(0);

        let visible = area.height as usize;
        let offset = self.home.power_home_scroll.clamp(
            display_cursor.saturating_sub(visible.saturating_sub(1)),
            display_cursor,
        );
        self.home.power_home_scroll = offset;

        // Build row map for mouse click handling.
        self.power_left_row_map.clear();
        for row in display_rows.iter().skip(offset).take(visible) {
            self.power_left_row_map.push(match row {
                HomeRow::Spacer | HomeRow::Header(_) => None,
                HomeRow::Item(idx) => Some(*idx),
            });
        }

        let avail = (area.width as usize).saturating_sub(2);
        let list_items: Vec<ListItem> = display_rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|row| match row {
                HomeRow::Spacer => ListItem::new(Line::default()),
                HomeRow::Header(label) => ListItem::new(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        trunc_str(label, avail).to_string(),
                        Style::default()
                            .fg(palette::YELLOW)
                            .add_modifier(Modifier::BOLD),
                    ),
                ])),
                HomeRow::Item(idx) => {
                    let item = &flat_items[*idx];
                    let selected = *idx == cursor;
                    let dur_str = if !item.is_folder && item.runtime_ticks > 0 {
                        format!(
                            " {}",
                            fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
                        )
                    } else {
                        String::new()
                    };
                    let name_w = avail.saturating_sub(dur_str.width());
                    let title = trunc_str(&item.display_name(), name_w);
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
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            area,
            &mut state,
        );

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
                area,
                &mut sb,
            );
        }
    }
}
