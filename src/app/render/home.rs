use super::super::layout::AppLayout;
use super::super::palette;
use super::super::ui_util::{
    fmt_item_continue, fmt_item_wrapped, highlight_style, highlight_style_continue,
};
use super::super::App;
use super::super::HOME_MIN_SECTION_H;
use crate::api::MediaItem;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    pub(super) fn render_home_cards(&mut self, f: &mut Frame, area: Rect, layout: &mut AppLayout) {
        let n_sections = 1 + self.home.latest.len();
        if n_sections == 0 {
            return;
        }

        if self.home.section >= n_sections {
            self.home.section = 0;
        }

        let compact = self.terminal_height < 28;
        let max_h_full = if area.height < 12 {
            area.height
        } else {
            ((area.height as u32 * 24 / 25) as u16).min(24)
        }
        .max(4);
        let side_h_full = ((max_h_full as u32 * 4 / 5) as u16).max(3);
        let center_h_full = if compact {
            side_h_full
        } else {
            side_h_full + 2
        };
        let visible = (area.height / center_h_full).max(1).min(n_sections as u16) as usize;

        let sec = self.home.section;
        if sec < self.home_cards_section_offset {
            self.home_cards_section_offset = sec;
        } else if sec >= self.home_cards_section_offset + visible {
            self.home_cards_section_offset = sec + 1 - visible;
        }
        let max_offset = n_sections.saturating_sub(visible);
        if self.home_cards_section_offset > max_offset {
            self.home_cards_section_offset = max_offset;
        }
        let offset = self.home_cards_section_offset;

        let arrow_top = offset > 0;
        let arrow_bot = offset + visible < n_sections;

        let constraints: Vec<ratatui::layout::Constraint> = (0..visible)
            .map(|_| ratatui::layout::Constraint::Ratio(1, visible as u32))
            .collect();
        let content_rect = area;
        let strips = ratatui::layout::Layout::vertical(constraints).split(content_rect);

        let mut section_data: Vec<(String, Vec<crate::api::MediaItem>, usize)> =
            Vec::with_capacity(visible);
        for i in 0..visible {
            let s = offset + i;
            let (title, items, cursor) = if s == 0 {
                (
                    "Continue".to_string(),
                    self.home.continue_items.clone(),
                    self.home.continue_cursor,
                )
            } else {
                let (t, _, items, c) = &self.home.latest[s - 1];
                (t.clone(), items.clone(), *c)
            };
            section_data.push((title, items, cursor));
        }

        for i in 0..visible {
            let s = offset + i;
            let is_active = s == self.home.section;
            let (ref title, ref items, cursor) = section_data[i];
            let strip = strips[i];
            if items.is_empty() {
                f.render_widget(
                    Paragraph::new("(empty)")
                        .style(Style::default().fg(palette::MUTED))
                        .alignment(Alignment::Center),
                    strip,
                );
                layout.home.home_card_strips.push((s, strip));
                continue;
            }
            let slots =
                self.render_home_cards_section(f, strip, title, items, cursor, is_active, layout);
            if is_active {
                layout.home.carousel_slots = slots;
            }
            layout.home.home_card_strips.push((s, strip));
        }

        let ud_arrow_style = Style::default().fg(palette::IRIS);
        if arrow_top {
            let r = Rect {
                x: area.x,
                y: area.y,
                width: area.width,
                height: 1,
            };
            layout.home.carousel_up_arrow = Some(r);
            f.render_widget(
                Paragraph::new("▲")
                    .style(ud_arrow_style)
                    .alignment(Alignment::Center),
                r,
            );
        }
        if arrow_bot {
            let r = Rect {
                x: area.x,
                y: area.bottom().saturating_sub(1),
                width: area.width,
                height: 1,
            };
            layout.home.carousel_down_arrow = Some(r);
            f.render_widget(
                Paragraph::new("▼")
                    .style(ud_arrow_style)
                    .alignment(Alignment::Center),
                r,
            );
        }
    }

    pub(super) fn render_home_cards_section(
        &mut self,
        f: &mut Frame,
        area: Rect,
        sec_title: &str,
        items: &[crate::api::MediaItem],
        cursor: usize,
        is_active: bool,
        layout: &mut AppLayout,
    ) -> [(Option<usize>, Rect); 3] {
        let n = items.len();
        if n == 0 {
            return [(None, Rect::default()); 3];
        }
        let cursor = cursor.min(n - 1);

        let cards_area = area;
        let cards_h = cards_area.height;

        let compact = self.terminal_height < 28;
        let max_h = if cards_h < 12 {
            cards_h
        } else {
            ((cards_h as u32 * 24 / 25) as u16).min(24)
        }
        .max(4);
        let side_h = ((max_h as u32 * 4 / 5) as u16).max(3);
        let center_h = if compact { side_h } else { side_h + 2 };
        let center_v_pad = (cards_h.saturating_sub(center_h)) / 2;
        let side_v_pad = center_v_pad + (center_h.saturating_sub(side_h)) / 2;

        const SIDE_HIDE_W: u16 = 60;
        let show_sides = cards_area.width >= SIDE_HIDE_W;

        const GAP: u16 = 1;
        let (center_w, side_w, x_left, x_center, x_right) = if show_sides {
            let avail_w = cards_area.width.saturating_sub(GAP * 4 + 4);
            let cw = (avail_w as u32 * 2 / 5) as u16;
            let sw = avail_w.saturating_sub(cw) / 2;
            let xl = cards_area.x + GAP + 2;
            let xc = xl + sw + GAP;
            let xr = xc + cw + GAP;
            (cw, sw, xl, xc, xr)
        } else {
            let avail_w = cards_area.width.saturating_sub(GAP * 2);
            (avail_w, 0, cards_area.x, cards_area.x + GAP, cards_area.x)
        };

        let slots: [(Option<usize>, Rect, bool); 3] = [
            (
                if show_sides && cursor > 0 {
                    Some(cursor - 1)
                } else {
                    None
                },
                Rect {
                    x: x_left + 2,
                    y: cards_area.y + side_v_pad,
                    width: side_w.saturating_sub(3),
                    height: side_h,
                },
                false,
            ),
            (
                Some(cursor),
                Rect {
                    x: x_center,
                    y: cards_area.y + center_v_pad,
                    width: center_w,
                    height: center_h,
                },
                true,
            ),
            (
                if show_sides && cursor + 1 < n {
                    Some(cursor + 1)
                } else {
                    None
                },
                Rect {
                    x: x_right + 1,
                    y: cards_area.y + side_v_pad,
                    width: side_w.saturating_sub(3),
                    height: side_h,
                },
                false,
            ),
        ];

        if self.images_enabled() {
            let prefetch_start = cursor.saturating_sub(3);
            let prefetch_end = (cursor + 3).min(n.saturating_sub(1));
            for (pi, item) in items
                .iter()
                .enumerate()
                .take(prefetch_end + 1)
                .skip(prefetch_start)
            {
                let (item_id, series_id) = (item.id.clone(), item.series_id.clone());
                let types_a: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio" => &["Primary"],
                    "Movie" => &["Backdrop", "Primary", "Logo"],
                    _ => &["Primary", "Backdrop", "Logo"],
                };
                self.fetch_card_image(
                    format!("{}:A", item_id.clone()),
                    item_id.clone(),
                    series_id.clone(),
                    types_a,
                );
                if pi != cursor {
                    let types_s: &[&str] = match item.item_type.as_str() {
                        "MusicAlbum" => &["AudioChild"],
                        "Audio" => &["Primary"],
                        _ => &["Logo", "Primary", "Backdrop"],
                    };
                    self.fetch_card_image(format!("{}:S", item_id), item_id, series_id, types_s);
                }
            }
        }

        for (maybe_idx, card_rect, is_center) in &slots {
            let i = match maybe_idx {
                None => continue,
                Some(i) => *i,
            };
            if card_rect.width < 3 {
                continue;
            }

            let item = &items[i];
            let is_ep = item.item_type == "Episode" && item.parent_index_number > 0;
            let ep_tag = if is_ep {
                format!("S{:02}E{:02}", item.parent_index_number, item.index_number)
            } else {
                String::new()
            };
            let name = item.name.clone();
            let series = item.series_name.clone();
            let runtime = item.runtime_ticks;
            let pos_ticks = item.playback_position_ticks;
            let rt_ticks = item.runtime_ticks;
            let played = item.played;
            let item_id = item.id.clone();
            let series_id = item.series_id.clone();
            let selected = i == cursor && is_active;

            let (cache_key, img_types): (String, &[&str]) = if *is_center {
                let types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio" => &["Primary"],
                    "Movie" => &["Backdrop", "Primary", "Logo"],
                    _ => &["Primary", "Backdrop", "Logo"],
                };
                (format!("{}:A", item_id), types)
            } else {
                let types: &[&str] = match item.item_type.as_str() {
                    "MusicAlbum" => &["AudioChild"],
                    "Audio" => &["Primary"],
                    _ => &["Logo", "Primary", "Backdrop"],
                };
                (format!("{}:S", item_id), types)
            };
            if self.images_enabled() {
                self.fetch_card_image(cache_key.clone(), item_id, series_id, img_types);
            }

            let count_label = if *is_center {
                Some(format!("{}/{}", cursor + 1, n))
            } else {
                None
            };
            let sec_title_label = if *is_center { Some(sec_title) } else { None };
            self.render_card_slot(
                f,
                *card_rect,
                *is_center,
                selected,
                false,
                false,
                false,
                false,
                &cache_key,
                &name,
                &series,
                &ep_tag,
                runtime,
                pos_ticks,
                rt_ticks,
                played,
                count_label.as_deref(),
                sec_title_label,
                false,
            );
        }

        if is_active {
            let lr_arrow_style = Style::default().fg(palette::WHITE);
            let y_mid = cards_area.y + center_v_pad + center_h / 2;
            if show_sides && cursor > 0 {
                let r = Rect {
                    x: x_left,
                    y: y_mid,
                    width: 1,
                    height: 1,
                };
                layout.home.carousel_left_arrow = Some(r);
                f.render_widget(Paragraph::new("◀").style(lr_arrow_style), r);
            }
            if show_sides && cursor + 1 < n {
                let r = Rect {
                    x: x_right + side_w - 1,
                    y: y_mid,
                    width: 1,
                    height: 1,
                };
                layout.home.carousel_right_arrow = Some(r);
                f.render_widget(Paragraph::new("▶").style(lr_arrow_style), r);
            }
        }

        [
            (slots[0].0, slots[0].1),
            (slots[1].0, slots[1].1),
            (slots[2].0, slots[2].1),
        ]
    }

    pub(super) fn render_home_panel(&mut self, f: &mut Frame, area: Rect, layout: &mut AppLayout) {
        let home_focused = true;
        let n_latest = self.home.latest.len();
        let n_sections = 1 + n_latest;
        let n_rows = 1 + n_latest.div_ceil(2);

        let visible_rows = if (n_rows as u16) * HOME_MIN_SECTION_H <= area.height {
            n_rows
        } else {
            ((area.height / HOME_MIN_SECTION_H) as usize).max(1)
        };

        let max_offset = n_rows.saturating_sub(visible_rows);
        if self.home_panel_section_offset > max_offset {
            self.home_panel_section_offset = max_offset;
        }
        let row_offset = self.home_panel_section_offset;
        let render_row_count = visible_rows.min(n_rows - row_offset);

        let scrollable = n_rows > visible_rows;
        let layout_area = if scrollable && area.width > 2 {
            Rect {
                width: area.width - 2,
                ..area
            }
        } else {
            area
        };

        let continue_items = self.home.continue_items.len();
        let constraints: Vec<Constraint> = (0..render_row_count)
            .map(|row_pos| {
                let logical_row = row_offset + row_pos;
                if logical_row == 0 {
                    // Size Continue Watching to its content: header + 2 rows per item, capped at half height
                    let content_h = (1 + continue_items as u16)
                        .min(area.height / 2)
                        .max(HOME_MIN_SECTION_H);
                    Constraint::Length(content_h)
                } else {
                    Constraint::Min(HOME_MIN_SECTION_H)
                }
            })
            .collect();
        let row_areas = Layout::vertical(constraints).spacing(1).split(layout_area);

        let mut areas: Vec<Rect> = vec![Rect::default(); n_sections];

        let latest_data: Vec<(String, Vec<MediaItem>, usize)> = self
            .home
            .latest
            .iter()
            .map(|(t, _, items, c)| (t.clone(), items.clone(), *c))
            .collect();

        let mut scrolls = vec![0usize; n_sections];

        for row_pos in 0..render_row_count {
            let logical_row = row_offset + row_pos;
            let row_area = row_areas[row_pos];

            if logical_row == 0 {
                areas[0] = row_area;
                let cont_focused = home_focused && self.home.section == 0;
                scrolls[0] = self.render_home_section(
                    f,
                    row_area,
                    "Continue",
                    &self.home.continue_items,
                    self.home.continue_cursor,
                    cont_focused,
                    true,
                );
            } else {
                let latest_row_idx = logical_row - 1;
                let left_sec = 1 + latest_row_idx * 2;
                let right_sec = left_sec + 1;

                let [left_area, right_area] =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .spacing(2)
                        .areas(row_area);

                let is_last_odd = right_sec >= n_sections;
                let render_left_area = if is_last_odd {
                    Layout::horizontal([
                        Constraint::Percentage(25),
                        Constraint::Percentage(50),
                        Constraint::Percentage(25),
                    ])
                    .areas::<3>(row_area)[1]
                } else {
                    left_area
                };

                if let Some((title, items, cursor)) = latest_data.get(left_sec - 1) {
                    areas[left_sec] = render_left_area;
                    let focused = home_focused && self.home.section == left_sec;
                    scrolls[left_sec] = self.render_home_section(
                        f,
                        render_left_area,
                        title,
                        items,
                        *cursor,
                        focused,
                        false,
                    );
                }
                if right_sec < n_sections {
                    if let Some((title, items, cursor)) = latest_data.get(right_sec - 1) {
                        areas[right_sec] = right_area;
                        let focused = home_focused && self.home.section == right_sec;
                        scrolls[right_sec] = self.render_home_section(
                            f, right_area, title, items, *cursor, focused, false,
                        );
                    }
                }
            }
        }

        layout.home.section_areas = areas;
        layout.home.home_scrolls = scrolls;

        if scrollable {
            let sb_rect = Rect {
                x: area.x + area.width.saturating_sub(1),
                y: area.y,
                width: 1,
                height: area.height,
            };
            layout.home.home_scrollbar = sb_rect;
            let mut sb_state = ScrollbarState::new(max_offset + 1).position(row_offset);
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
        } else {
            layout.home.home_scrollbar = Rect::default();
        }
    }

    pub(super) fn render_home_section(
        &self,
        f: &mut Frame,
        area: Rect,
        title: &str,
        items: &[MediaItem],
        cursor: usize,
        focused: bool,
        continue_style: bool,
    ) -> usize {
        if area.height < 2 {
            return 0;
        }

        let title_style = if focused {
            Style::default()
                .fg(palette::IRIS)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
                .fg(palette::YELLOW)
                .add_modifier(Modifier::BOLD)
        };
        let rule_style = Style::default().fg(palette::MUTED);

        // Header: bold title + right-padded rule on the same row
        let title_chars = title.chars().count() as u16;
        let rule_len = area.width.saturating_sub(title_chars + 1);
        let rule = "\u{2500}".repeat(rule_len as usize);
        let header = Line::from(vec![
            Span::styled(title.to_string(), title_style),
            Span::styled(format!(" {rule}"), rule_style),
        ]);
        let header_rect = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        f.render_widget(Paragraph::new(header), header_rect);

        let list_rect = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        if items.is_empty() {
            f.render_widget(
                Paragraph::new("(empty)").style(Style::default().fg(palette::MUTED)),
                list_rect,
            );
            return 0;
        }

        let list_items: Vec<ListItem> = items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let sel = focused && i == cursor;
                let li = if continue_style {
                    ListItem::new(fmt_item_continue(item, list_rect.width as usize, sel))
                } else {
                    ListItem::new(fmt_item_wrapped(item, list_rect.width as usize, sel))
                };
                if sel {
                    li.style(if continue_style {
                        highlight_style_continue(item)
                    } else {
                        highlight_style(item)
                    })
                } else {
                    li
                }
            })
            .collect();

        let mut state = ListState::default();
        if focused {
            state.select(Some(cursor));
        }
        f.render_stateful_widget(List::new(list_items), list_rect, &mut state);
        state.offset()
    }

    pub(super) fn render_home_search(&mut self, f: &mut Frame, area: Rect) {
        use super::super::palette;
        use ratatui::layout::{Constraint, Layout};
        use ratatui::style::{Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{
            Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        };

        if self.home_search.is_none() {
            return;
        }

        // Compute layout parameters from an immutable borrow, then drop it
        let (show_filter, filter_height, filtered_total) = {
            let hs = self.home_search.as_ref().unwrap();
            let types = hs.available_types();
            let show = !hs.results.is_empty() && types.len() > 1;
            let total = hs.filtered_count();
            (show, if show { 1 } else { 0 }, total)
        };

        let chunks = Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(filter_height),
            Constraint::Min(0),
        ])
        .split(area);
        let input_area = chunks[0];
        let filter_area = chunks[1];
        let results_area = chunks[2];
        let visible = results_area.height as usize;

        // Mutable scroll sync
        if let Some(ref mut hs) = self.home_search {
            if hs.cursor < hs.scroll {
                hs.scroll = hs.cursor;
            } else if visible > 0 && hs.cursor >= hs.scroll + visible {
                hs.scroll = hs.cursor + 1 - visible;
            }
        }

        let hs = self.home_search.as_ref().unwrap();

        // Cursor position (computed before the borrow ends)
        let input_focused = hs.input_focused;
        let cursor_x = (input_area.x + 1 + hs.query.width() as u16)
            .min(input_area.x + input_area.width.saturating_sub(2));
        let cursor_y = input_area.y + 1;

        // Search input bar
        let loading_suffix = if hs.loading { " [searching...]" } else { "" };
        let input_text = format!("{}{}", hs.query, loading_suffix);
        let border_color = if input_focused {
            palette::IRIS
        } else {
            palette::MUTED
        };
        let hint_style = Style::default().fg(palette::MUTED);
        f.render_widget(
            Paragraph::new(input_text)
                .style(Style::default().fg(palette::FOAM))
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(border_color))
                        .title(" Search ")
                        .title_style(Style::default().fg(palette::YELLOW))
                        .title_bottom(Line::from(vec![
                            Span::styled(" ESC: back", hint_style),
                            Span::styled("  Tab: toggle input ", hint_style),
                        ])),
                ),
            input_area,
        );
        if input_focused {
            f.set_cursor_position((cursor_x, cursor_y));
        }

        // Type filter bar
        if show_filter {
            let types = hs.available_types();
            let mut spans: Vec<Span> = Vec::new();
            let active = hs.type_filter;
            let all_style = if active == 0 {
                Style::default()
                    .fg(palette::FOAM)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::default().fg(palette::MUTED)
            };
            spans.push(Span::styled(" All ", all_style));
            for (i, t) in types.iter().enumerate() {
                let label = match *t {
                    "Movie" => " Movie ",
                    "Series" => " Series ",
                    "Episode" => " Ep ",
                    "Audio" => " Audio ",
                    "MusicAlbum" => " Album ",
                    "MusicArtist" => " Artist ",
                    _ => " Item ",
                };
                let style = if active == i + 1 {
                    Style::default()
                        .fg(palette::FOAM)
                        .add_modifier(Modifier::REVERSED)
                } else {
                    Style::default().fg(palette::MUTED)
                };
                spans.push(Span::styled(label, style));
            }
            f.render_widget(Paragraph::new(Line::from(spans)), filter_area);
        }

        if hs.results.is_empty() && !hs.loading {
            let hint = if hs.last_query.is_empty() {
                "Type a search term and press Enter"
            } else {
                "No results"
            };
            f.render_widget(
                Paragraph::new(hint).style(Style::default().fg(palette::MUTED)),
                Rect {
                    x: results_area.x + 1,
                    y: results_area.y + 1,
                    width: results_area.width.saturating_sub(2),
                    height: 1,
                },
            );
            return;
        }

        let cursor = hs.cursor;
        let scroll = hs.scroll;
        let filtered = hs.filtered_results();

        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .skip(scroll)
            .take(visible)
            .map(|(i, item)| {
                let type_label = match item.item_type.as_str() {
                    "Movie" => "[Movie]  ",
                    "Series" => "[Series] ",
                    "Episode" => "[Ep]     ",
                    "Audio" => "[Audio]  ",
                    "MusicAlbum" => "[Album]  ",
                    "MusicArtist" => "[Artist] ",
                    _ => "[Item]   ",
                };
                let year = if item.production_year > 0 {
                    format!("  ({})", item.production_year)
                } else {
                    String::new()
                };
                let name = format!("{}{}{}", type_label, item.display_name(), year);
                let style = if i == cursor {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(vec![Span::styled(name, style)]))
            })
            .collect();

        use ratatui::widgets::{List, ListState};
        let mut state = ListState::default();
        state.select(Some(cursor.saturating_sub(scroll)));
        f.render_stateful_widget(List::new(items), results_area, &mut state);

        if filtered_total > visible {
            let mut sb_state = ScrollbarState::new(filtered_total).position(scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                results_area,
                &mut sb_state,
            );
        }
    }
}
