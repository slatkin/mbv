use super::super::layout::{LayoutHome, LayoutQueue};
use super::super::ui_util::{build_queue_rows, fmt_duration, trunc_str, QueueRow};
use super::super::{palette, App, QueueScope};
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
        if self.search.is_open() {
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
        if self.search.is_open() {
            self.render_home_search(f, area);
            return;
        }
        let playback = self.displayed_queue_playback_state();
        let (items, cursor) = {
            let queue = self.displayed_queue();
            (queue.items.clone(), queue.queue_cursor)
        };

        layout.rect = area;

        // Render scope-pill header when a direct remote queue exists.
        let show_scope_header = self.has_direct_remote_queue();
        layout.scope_local_area = Rect::default();
        layout.scope_remote_area = Rect::default();

        let header_h: u16 = if show_scope_header { 1 } else { 0 };
        if show_scope_header {
            let queue_label = " Queue ";
            let queue_w = queue_label.width() as u16;
            let queue_x = area.x + area.width.saturating_sub(queue_w);
            let mut spans = Vec::new();
            let local_selected = self.visible_queue_scope() == QueueScope::Local;
            let local_label = " Local ";
            let remote_label = " Remote ";
            let local_w = local_label.width() as u16;
            let remote_w = remote_label.width() as u16;
            let gap = 1u16;
            layout.scope_local_area = Rect {
                x: area.x,
                y: area.y,
                width: local_w,
                height: 1,
            };
            layout.scope_remote_area = Rect {
                x: area.x + local_w + gap,
                y: area.y,
                width: remote_w,
                height: 1,
            };
            let inactive = Style::default().fg(palette::MUTED).bg(palette::PILL_BG);
            let active = Style::default().fg(palette::BASE).bg(palette::FOAM);
            spans.push(Span::styled(
                local_label,
                if local_selected { active } else { inactive },
            ));
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                remote_label,
                if local_selected { inactive } else { active },
            ));
            let header_used = spans.iter().map(|span| span.content.width()).sum::<usize>() as u16;
            let gap_to_queue = queue_x.saturating_sub(area.x + header_used);
            spans.push(Span::raw(" ".repeat(gap_to_queue as usize)));
            spans.push(Span::styled(
                queue_label,
                Style::default().fg(palette::BASE).bg(palette::FOAM),
            ));
            f.render_widget(
                Paragraph::new(Line::from(spans)),
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
            );
        }

        let inner = if header_h > 0 {
            Rect {
                y: area.y + 1,
                height: area.height.saturating_sub(1),
                ..area
            }
        } else {
            area
        };
        layout.inner = inner;

        if items.is_empty() {
            let msg = if show_scope_header && self.visible_queue_scope() == QueueScope::Remote {
                "  Remote queue is empty"
            } else {
                "  Add items with p from Home or library tabs"
            };
            f.render_widget(
                Paragraph::new(msg).style(Style::default().fg(palette::MUTED)),
                inner,
            );
            return;
        }

        // List view occupies 90% of available width, centered.
        let list_w = (inner.width as u32 * 9 / 10) as u16;
        let list_x = inner.x + (inner.width.saturating_sub(list_w)) / 2;
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
        // position_ticks is frozen when paused, so the spinner naturally freezes at the right frame.
        let spinner_char: &str = SPINNER_FRAMES
            [(playback.position_ticks.max(0) / 1_500_000) as usize % SPINNER_FRAMES.len()];
        let spinner_color = if playback.paused {
            palette::YELLOW
        } else {
            palette::IRIS
        };

        // Build display rows and window them to the visible height.
        //
        // A queue sourced from a saved playlist gets a single header naming the
        // playlist, rather than the per-album/per-series headers `build_queue_rows`
        // would otherwise produce -- those make a playlist queue look like it was
        // built by "play series"/"play album" and hide the fact that it's a curated
        // playlist (see the "QI XL" show-name header bug: a playlist of episodes
        // from one show rendered with the show name as if the queue were a
        // "play series" queue).
        let playlist_name = self.queue_playlist_name();
        let (display, group_for_header) = if !playlist_name.is_empty() {
            let mut rows: Vec<QueueRow> = Vec::with_capacity(items.len() + 1);
            rows.push(QueueRow::Header);
            rows.extend((0..items.len()).map(|idx| QueueRow::Track {
                idx,
                in_group: false,
            }));
            (rows, vec![playlist_name.to_string()])
        } else {
            build_queue_rows(&items, self.queue_group)
        };
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
                    let now_playing = i == playback.active_idx && playback.active;
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
                        let pos = if playback.position_ticks > 0 {
                            playback.position_ticks
                        } else {
                            item.playback_position_ticks
                        };
                        (pos, playback.runtime_ticks)
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
