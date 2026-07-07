use super::super::layout::{LayoutHome, LayoutQueue};
use super::super::ui_util::{build_queue_rows, fmt_duration, trunc_str, QueueRow};
use super::super::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::{Alignment, Constraint, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Cell, Paragraph, Row, Table};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    pub(super) fn render_combined(&mut self, f: &mut Frame, area: Rect, layout: &mut LayoutHome) {
        // Carousel arrow hitboxes and card strips are only ever populated by
        // the card-view branch below; they no longer need an unconditional
        // reset here because `layout` is a fresh `AppLayout::default()` built
        // at the top of this frame's render pass (see `App::render`), so they
        // already start at their default (unset) values on every frame.
        layout.home_rect = area;
        if self.home_search.is_some() {
            self.render_home_search(f, area);
        } else if self.home_card_view {
            self.render_home_cards(f, area, layout);
        } else {
            self.render_home_panel(f, area, layout);
        }
    }

    pub(super) fn render_queue_panel(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutQueue,
    ) {
        if self.home_search.is_some() {
            self.render_home_search(f, area);
            return;
        }
        let (active, current_idx, live_pos, live_runtime, live_paused) =
            self.effective_playback_state();
        let (items, cursor) = {
            let queue = self.displayed_queue();
            (queue.items.clone(), queue.queue_cursor)
        };

        layout.rect = area;

        let inner = area;
        layout.inner = inner;

        if items.is_empty() {
            f.render_widget(
                Paragraph::new("Add items with p from Home or library tabs")
                    .style(Style::default().fg(palette::MUTED)),
                inner,
            );
            return;
        }

        // List view occupies 90% of available width, centered.
        let list_w = (area.width as u32 * 9 / 10) as u16;
        let list_x = area.x + (area.width.saturating_sub(list_w)) / 2;
        let table_area = Rect {
            x: list_x,
            width: list_w,
            ..inner
        };

        let show_ep_cols = items.iter().any(|it| it.item_type == "Episode");

        // Fixed column widths + inter-column gaps of 1.
        let title_col_width =
            (table_area.width as i32 - if show_ep_cols { 21 } else { 13 }).max(0) as usize;

        const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        // Drive frame index from playback position (10M ticks/sec; 1.5M ticks = 150ms per frame).
        // live_pos is frozen when paused, so the spinner naturally freezes at the right frame.
        let spinner_char: &str =
            SPINNER_FRAMES[(live_pos.max(0) / 1_500_000) as usize % SPINNER_FRAMES.len()];
        let spinner_color = if live_paused {
            palette::YELLOW
        } else {
            palette::IRIS
        };

        // Build display rows (grouped or flat) and window them to the visible height.
        let (display, group_for_header) = build_queue_rows(&items, self.queue_group);
        let visible = table_area.height as usize;
        let cursor_row = display
            .iter()
            .position(|r| {
                if let QueueRow::Track { idx, .. } = r {
                    *idx == cursor
                } else {
                    false
                }
            })
            .unwrap_or(0);
        let offset = if cursor_row >= visible {
            cursor_row - visible + 1
        } else {
            0
        };
        let mut header_idx = display[..offset]
            .iter()
            .filter(|r| matches!(r, QueueRow::Header))
            .count();

        let items = &items;
        let mut rows: Vec<Row> = Vec::new();
        // Header rows are rendered as full-width overlays after the table; collect their
        // positions and labels here.
        let mut header_overlays: Vec<(u16, String)> = Vec::new();

        for (row_idx, entry) in display.iter().skip(offset).take(visible).enumerate() {
            match entry {
                QueueRow::Header => {
                    let group = group_for_header
                        .get(header_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    header_idx += 1;
                    header_overlays.push((table_area.y + row_idx as u16, group.to_string()));
                    // Placeholder row keeps table row indices aligned with the display list.
                    rows.push(Row::new([
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                    ]));
                    layout.row_map.push(None);
                }
                QueueRow::Spacer => {
                    rows.push(Row::new([
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                    ]));
                    layout.row_map.push(None);
                }
                QueueRow::Track { idx, in_group } => {
                    let i = *idx;
                    let item = &items[i];
                    let now_playing = i == current_idx && active;
                    let row_style = if i == cursor {
                        Style::default().fg(palette::YELLOW)
                    } else {
                        Style::default().fg(palette::WHITE)
                    };
                    let indicator = if i == cursor {
                        Cell::from("▐").style(Style::default().fg(palette::PINE))
                    } else {
                        Cell::from(" ")
                    };

                    // Under a group header, show bare names (mirror the power view).
                    let title = if *in_group && item.is_audio() {
                        if item.index_number > 0 {
                            format!("{:02}. {}", item.index_number, item.name)
                        } else {
                            item.name.clone()
                        }
                    } else if *in_group && item.item_type == "Episode" {
                        item.name.clone()
                    } else {
                        item.playback_label()
                    };
                    let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                    let length = if len_secs > 0 {
                        fmt_duration(len_secs)
                    } else {
                        "—".to_string()
                    };
                    let (pos_ticks, rt_ticks) = if now_playing {
                        let pos = if live_pos > 0 {
                            live_pos
                        } else {
                            item.playback_position_ticks
                        };
                        (pos, live_runtime)
                    } else {
                        (item.playback_position_ticks, item.runtime_ticks)
                    };
                    // Spinner prefix "⠋ " costs 2 chars when now-playing.
                    let spin_w: usize = if now_playing { 2 } else { 0 };
                    let indent: usize = if *in_group { 1 } else { 0 };
                    let avail = title_col_width.saturating_sub(indent);
                    // Now-playing title text is emby blue (not bold); others inherit row_style.
                    let title_span_style = if now_playing {
                        Style::default().fg(palette::FOAM)
                    } else {
                        Style::default()
                    };
                    let title_cell = if pos_ticks > 0 && rt_ticks > 0 && !item.is_audio() {
                        let pct = (pos_ticks * 100 / rt_ticks.max(1)) as u64;
                        // Now-playing progress is green; other in-progress rows are dim.
                        let pct_style = if now_playing {
                            palette::IRIS
                        } else {
                            palette::MUTED
                        };
                        let pct_str = format!(" {pct}%");
                        let max_title = avail.saturating_sub(pct_str.chars().count() + spin_w);
                        let mut spans: Vec<Span> = Vec::new();
                        if indent > 0 {
                            spans.push(Span::raw(" "));
                        }
                        if now_playing {
                            spans.push(Span::styled(
                                spinner_char.to_string(),
                                Style::default().fg(spinner_color),
                            ));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(trunc_str(&title, max_title), title_span_style));
                        spans.push(Span::styled(pct_str, Style::default().fg(pct_style)));
                        Cell::from(Line::from(spans))
                    } else {
                        let max_title = avail.saturating_sub(spin_w);
                        let mut spans: Vec<Span> = Vec::new();
                        if indent > 0 {
                            spans.push(Span::raw(" "));
                        }
                        if now_playing {
                            spans.push(Span::styled(
                                spinner_char.to_string(),
                                Style::default().fg(spinner_color),
                            ));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(trunc_str(&title, max_title), title_span_style));
                        Cell::from(Line::from(spans))
                    };

                    let row = if show_ep_cols {
                        let ep_tag = if item.item_type == "Episode" && item.parent_index_number > 0
                        {
                            format!("S{:02}/E{:02}", item.parent_index_number, item.index_number)
                        } else {
                            String::new()
                        };
                        Row::new([
                            indicator,
                            title_cell,
                            Cell::from(Line::from(ep_tag).alignment(Alignment::Right))
                                .style(Style::default().fg(palette::SUBTLE)),
                            Cell::from(Line::from(length).alignment(Alignment::Right)),
                            Cell::from(""),
                        ])
                        .style(row_style)
                    } else {
                        Row::new([
                            indicator,
                            title_cell,
                            Cell::from(""),
                            Cell::from(Line::from(length).alignment(Alignment::Right)),
                            Cell::from(""),
                        ])
                        .style(row_style)
                    };
                    rows.push(row);
                    layout.row_map.push(Some(i));
                }
            }
        }

        let table = Table::new(
            rows,
            [
                Constraint::Length(1),
                Constraint::Min(10),
                Constraint::Length(if show_ep_cols { 8 } else { 0 }),
                Constraint::Length(7),
                Constraint::Length(1),
            ],
        )
        .column_spacing(1);
        f.render_widget(table, table_area);

        // Render group headers as full-width overlays so the FOAM line spans the
        // entire list area rather than just the title column.
        let full_w = table_area.width as usize;
        for (y, group) in &header_overlays {
            let max_label = full_w.saturating_sub(5);
            let label = trunc_str(group, max_label);
            let pill = format!(" {} ", label.to_uppercase());
            let pill_w = pill.width();
            let right = 2usize.min(full_w.saturating_sub(pill_w));
            let left = full_w.saturating_sub(pill_w + right);
            f.render_widget(
                Paragraph::new(Line::from(vec![
                    Span::styled("\u{2500}".repeat(left), Style::default().fg(palette::FOAM)),
                    Span::styled(pill, Style::default().fg(palette::BASE).bg(palette::FOAM)),
                    Span::styled("\u{2500}".repeat(right), Style::default().fg(palette::FOAM)),
                ])),
                Rect {
                    x: table_area.x,
                    y: *y,
                    width: table_area.width,
                    height: 1,
                },
            );
        }
    }
}
