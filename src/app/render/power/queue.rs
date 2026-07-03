use super::super::super::ui_util::*;
use crate::api::TICKS_PER_SECOND;
use crate::app::{palette, App, QueueScope};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    pub(super) fn render_power_queue(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
    ) -> Vec<u16> {
        if area.height < 1 {
            return vec![];
        }

        self.power_queue_cursor_screen_y = None;

        let show_remote_scope = self.has_direct_remote_queue();
        self.power_queue_scope_local_area = Rect::default();
        self.power_queue_scope_remote_area = Rect::default();

        // Queue header row: default FOAM rule when there is only one scope,
        // or scope pills plus Queue pill when both Local/Remote scopes exist.
        if area.height > 0 {
            if show_remote_scope {
                let queue_label = " Queue ";
                let queue_w = queue_label.width() as u16;
                let queue_x = area.x + area.width.saturating_sub(queue_w);
                let mut spans = Vec::new();
                let local_selected = self.displayed_queue_scope() == QueueScope::Local;
                let local_label = " Local ";
                let remote_label = " Remote ";
                let local_w = local_label.width() as u16;
                let remote_w = remote_label.width() as u16;
                let gap = 1u16;
                self.power_queue_scope_local_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: local_w,
                    height: 1,
                };
                self.power_queue_scope_remote_area = Rect {
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
                let header_used =
                    spans.iter().map(|span| span.content.width()).sum::<usize>() as u16;
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
            } else {
                let pill = " Queue ";
                let pill_w = pill.width();
                let left = (area.width as usize).saturating_sub(pill_w);
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("─".repeat(left), Style::default().fg(palette::FOAM)),
                        Span::styled(pill, Style::default().fg(palette::BASE).bg(palette::FOAM)),
                    ])),
                    Rect {
                        x: area.x,
                        y: area.y,
                        width: area.width,
                        height: 1,
                    },
                );
            }
        }
        let area = Rect {
            y: area.y + 1,
            height: area.height.saturating_sub(1),
            ..area
        };
        // Store the content area (after header) so mouse clicks map to the right rows.
        self.power_queue_area = area;

        let (items, cursor) = {
            let queue = self.displayed_queue();
            (queue.items.clone(), queue.playlist_cursor)
        };
        let n = items.len();
        if n == 0 {
            self.power_queue_scroll = 0;
            self.power_queue_row_map.clear();
            f.render_widget(
                Paragraph::new(if self.displayed_queue_scope() == QueueScope::Local {
                    "  Add items with p from Home or library tabs"
                } else {
                    "  Remote queue is empty"
                })
                .style(Style::default().fg(palette::MUTED)),
                area,
            );
            return vec![];
        }

        let (active, active_idx, live_pos, live_runtime, live_paused) =
            if self.displayed_queue_scope() == QueueScope::Remote || !self.player.is_remote() {
                self.effective_playback_state()
            } else {
                (false, 0, 0, 0, false)
            };

        // Build display rows: audio grouped by album, episodes by series, the rest
        // flat. group_for_header[j] holds the label for the j-th Header.
        let (display, group_for_header) = build_queue_rows(&items, true);
        let total = display.len();
        let visible = area.height as usize;

        // Visual row of the cursor item.
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
        let max_offset = total.saturating_sub(visible);
        self.power_queue_scroll = self.power_queue_scroll.min(max_offset);
        if cursor_row < self.power_queue_scroll {
            self.power_queue_scroll = cursor_row;
        } else if cursor_row >= self.power_queue_scroll + visible {
            self.power_queue_scroll = cursor_row.saturating_sub(visible.saturating_sub(1));
        }
        let offset = self.power_queue_scroll;
        self.power_queue_cursor_screen_y =
            Some(area.y + 1 + (cursor_row.saturating_sub(self.power_queue_scroll)) as u16);

        // Count how many group headers appear before the scroll offset, so we
        // index group_for_header correctly for the visible window.
        let mut header_idx = display[..offset]
            .iter()
            .filter(|r| matches!(r, QueueRow::Header))
            .count();

        let has_sb = total > visible; // column always reserved when scrollbar would appear
        let need_sb = has_sb && focused; // scrollbar only drawn when focused
        let render_w = area.width.saturating_sub(if has_sb { 1 } else { 0 }) as usize;
        let show_length = render_w > 30;
        let dur_w: usize = if show_length { 6 } else { 0 }; // "mm:ss" or "h:mm:ss"

        // Spinner character for the active item — computed once per frame, not per row.
        const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
        // Drive frame index from playback position (10M ticks/sec; 1.5M ticks = 150ms per frame).
        // live_pos is frozen when paused, so the spinner naturally freezes at the right frame.
        let spinner_frame: &str =
            SPINNER_FRAMES[(live_pos.max(0) / 1_500_000) as usize % SPINNER_FRAMES.len()];
        let spinner_color = if live_paused {
            palette::YELLOW
        } else {
            palette::IRIS
        };

        // Build visible ListItems and the row map simultaneously.
        self.power_queue_row_map.clear();
        let mut list_items: Vec<ListItem> = Vec::new();
        let mut header_ys: Vec<u16> = Vec::new();

        for (row_idx, entry) in display.iter().skip(offset).take(visible).enumerate() {
            match entry {
                QueueRow::Header => {
                    let group = group_for_header
                        .get(header_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    header_idx += 1;
                    header_ys.push(area.y + row_idx as u16);
                    let label = trunc_str(group, render_w.saturating_sub(1));
                    list_items.push(ListItem::new(Line::from(Span::styled(
                        format!(" {}", label),
                        Style::default()
                            .fg(palette::YELLOW)
                            .add_modifier(Modifier::BOLD),
                    ))));
                    self.power_queue_row_map.push(None);
                }
                QueueRow::Spacer => {
                    list_items.push(ListItem::new(Line::raw("")));
                    self.power_queue_row_map.push(None);
                }
                QueueRow::Track { idx, in_group: _ } => {
                    let i = *idx;
                    let indent: usize = 0;
                    let item = &items[i];
                    let is_active = i == active_idx && active;
                    let is_cursor = i == cursor && focused;

                    let fg = if is_cursor {
                        palette::PINE
                    } else if focused {
                        palette::WHITE
                    } else {
                        palette::SUBTLE
                    };
                    let row_style = Style::default().fg(fg);

                    let (pt, rt) = if is_active {
                        let pos = if live_pos > 0 {
                            live_pos
                        } else {
                            item.playback_position_ticks
                        };
                        (pos, live_runtime)
                    } else {
                        (item.playback_position_ticks, item.runtime_ticks)
                    };
                    let pct_str = if pt > 0 && rt > 0 && !item.is_audio() {
                        format!(" {}%", pt * 100 / rt.max(1))
                    } else {
                        String::new()
                    };

                    let marker = if is_cursor {
                        Span::styled("\u{258c}", Style::default().fg(palette::PINE))
                    } else {
                        Span::raw(" ")
                    };

                    // Show queue position (1-based) for all items, right-aligned
                    // so single-digit numbers line up with double-digit ones.
                    let queue_pos = idx + 1;
                    let num_w = items.len().to_string().len();
                    let label = format!("{:>num_w$}. {}", queue_pos, item.name);

                    let len_secs = item.runtime_ticks / TICKS_PER_SECOND;
                    let dur = if len_secs > 0 {
                        if item.is_audio() {
                            fmt_duration(len_secs)
                        } else {
                            fmt_duration_approx(len_secs)
                        }
                    } else {
                        String::new()
                    };
                    let dim_color = if focused {
                        palette::SUBTLE
                    } else {
                        palette::MUTED
                    };

                    // Spinner shown right after the title while the item is playing.
                    let spinner_char: &str = if is_active { spinner_frame } else { "" };

                    // Reserve 2 extra chars for " ⠋" when active.
                    let spinner_w: usize = if is_active { 2 } else { 0 };
                    // Title truncated to leave room for indent + marker + spinner + duration + pct.
                    let extra = dur_w + pct_str.chars().count() + spinner_w;
                    let title_w = render_w.saturating_sub(indent + 1 + extra); // 1 marker
                    let title = trunc_str(&label, title_w);

                    // Now-playing title text is always emby blue, regardless of focus state.
                    let title_color = if is_active { palette::FOAM } else { fg };

                    let mut spans: Vec<Span> = Vec::new();
                    if indent > 0 {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(marker);
                    // Prefix is "{n:>w}. " — render it dim, then insert spinner between
                    // prefix and name when active so it reads " 3. ⠋ Title".
                    let prefix_chars = format!("{:>num_w$}. ", queue_pos).chars().count();
                    let tc = title.chars().count();
                    if tc > prefix_chars {
                        let split = title
                            .char_indices()
                            .nth(prefix_chars)
                            .map(|(i, _)| i)
                            .unwrap_or(title.len());
                        spans.push(Span::styled(
                            title[..split].to_string(),
                            Style::default().fg(dim_color),
                        ));
                        if is_active {
                            spans.push(Span::styled(
                                spinner_char.to_string(),
                                Style::default().fg(spinner_color),
                            ));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(
                            title[split..].to_string(),
                            Style::default().fg(title_color),
                        ));
                    } else {
                        if is_active {
                            spans.push(Span::styled(
                                spinner_char.to_string(),
                                Style::default().fg(spinner_color),
                            ));
                            spans.push(Span::raw(" "));
                        }
                        spans.push(Span::styled(title, Style::default().fg(title_color)));
                    }
                    if !pct_str.is_empty() {
                        let pct_color = if is_active {
                            palette::IRIS
                        } else {
                            palette::MUTED
                        };
                        spans.push(Span::styled(pct_str, Style::default().fg(pct_color)));
                    }
                    if show_length && !dur.is_empty() {
                        let dur_color = dim_color;
                        // Right-align duration to the right edge of the queue panel.
                        let used: usize = spans.iter().map(|s| s.content.as_ref().width()).sum();
                        let pad = render_w.saturating_sub(used + dur.width());
                        spans.push(Span::raw(" ".repeat(pad)));
                        spans.push(Span::styled(dur, Style::default().fg(dur_color)));
                    }

                    list_items.push(ListItem::new(Line::from(spans)).style(row_style));
                    self.power_queue_row_map.push(Some(i));
                }
            }
        }

        let mut state = ListState::default();
        state.select(Some(cursor_row.saturating_sub(offset)));
        let render_area = Rect {
            width: render_w as u16,
            ..area
        };
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default()),
            render_area,
            &mut state,
        );

        if need_sb {
            let max_off = total.saturating_sub(visible);
            let mut sb = ScrollbarState::new(max_off + 1).position(offset);
            let sb_area = Rect {
                x: area.x + area.width.saturating_sub(1),
                width: 1,
                ..area
            };
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("\u{2590}")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                sb_area,
                &mut sb,
            );
        }
        header_ys
    }
}
