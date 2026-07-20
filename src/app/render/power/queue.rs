use super::super::super::ui_util::*;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App, QueueScope};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

const QUEUE_TITLE_QUIET_COLUMNS: usize = 8;

fn queue_group_start_row(display: &[QueueRow], row: usize) -> usize {
    let mut start = row;
    while start > 0 && !matches!(display[start - 1], QueueRow::Track { .. }) {
        start -= 1;
    }
    start
}

impl App {
    pub(super) fn render_power_queue(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutPower,
    ) -> Vec<u16> {
        if area.height < 1 {
            return vec![];
        }

        layout.queue_cursor_screen_y = None;

        let show_remote_scope = self.has_direct_remote_queue();
        layout.queue_scope_local_area = Rect::default();
        layout.queue_scope_remote_area = Rect::default();

        // Queue header row: default FOAM rule when there is only one scope,
        // or scope pills plus Queue pill when both Local/Remote scopes exist.
        if area.height > 0 {
            if show_remote_scope {
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
                layout.queue_scope_local_area = Rect {
                    x: area.x,
                    y: area.y,
                    width: local_w,
                    height: 1,
                };
                layout.queue_scope_remote_area = Rect {
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
                        Span::styled("━".repeat(left), Style::default().fg(palette::FOAM)),
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
        layout.queue_area = area;

        let (items, cursor) = {
            let queue = self.displayed_queue();
            (queue.items.clone(), queue.queue_cursor)
        };
        let n = items.len();
        if n == 0 {
            self.power_queue_scroll = 0;
            f.render_widget(
                Paragraph::new(if self.visible_queue_scope() == QueueScope::Local {
                    "  Add items with p from Home or library tabs"
                } else {
                    "  Remote queue is empty"
                })
                .style(Style::default().fg(palette::MUTED)),
                area,
            );
            return vec![];
        }

        let playback = self.displayed_queue_playback_state();

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
            self.power_queue_scroll = queue_group_start_row(&display, cursor_row);
        } else if cursor_row >= self.power_queue_scroll + visible {
            self.power_queue_scroll = cursor_row.saturating_sub(visible.saturating_sub(1));
        }
        let offset = self.power_queue_scroll;
        layout.queue_cursor_screen_y =
            Some(area.y + (cursor_row.saturating_sub(self.power_queue_scroll)) as u16);

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

        // Build visible ListItems and the row map simultaneously.
        let mut list_items: Vec<ListItem> = Vec::new();
        let mut header_ys: Vec<u16> = Vec::new();

        // Line-based cursor (not item-based): group headers can wrap into more
        // than one screen line, so the on-screen row a given `display` entry
        // lands on isn't always the same as its index in `display`.
        let mut line_offset: u16 = 0;

        for entry in display.iter().skip(offset) {
            if line_offset as usize >= visible {
                break;
            }
            match entry {
                QueueRow::Header => {
                    let group = group_for_header
                        .get(header_idx)
                        .map(|s| s.as_str())
                        .unwrap_or("");
                    header_idx += 1;
                    header_ys.push(area.y + line_offset);
                    // Long group headers wrap onto additional lines instead of
                    // being truncated, so the full artist/series name is
                    // always visible.
                    let wrap_w = render_w.saturating_sub(1).max(1);
                    let wrapped = wrap(group, wrap_w);
                    let lines: Vec<Line> = if wrapped.is_empty() {
                        vec![Line::from(Span::styled(
                            " ",
                            Style::default()
                                .fg(palette::YELLOW)
                                .add_modifier(Modifier::BOLD),
                        ))]
                    } else {
                        wrapped
                            .iter()
                            .map(|seg| {
                                Line::from(Span::styled(
                                    format!(" {seg}"),
                                    Style::default()
                                        .fg(palette::YELLOW)
                                        .add_modifier(Modifier::BOLD),
                                ))
                            })
                            .collect()
                    };
                    let n_lines = lines.len() as u16;
                    list_items.push(ListItem::new(Text::from(lines)));
                    for _ in 0..n_lines {
                        layout.queue_row_map.push(None);
                    }
                    line_offset += n_lines;
                }
                QueueRow::Spacer => {
                    list_items.push(ListItem::new(Line::raw("")));
                    layout.queue_row_map.push(None);
                    line_offset += 1;
                }
                QueueRow::Track { idx, in_group: _ } => {
                    let i = *idx;
                    let indent: usize = 0;
                    let item = &items[i];
                    let is_active = i == playback.active_idx && playback.active;
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
                        let pos = if playback.position_ticks > 0 {
                            playback.position_ticks
                        } else {
                            item.playback_position_ticks
                        };
                        (pos, playback.runtime_ticks)
                    } else {
                        (item.playback_position_ticks, item.runtime_ticks)
                    };
                    let pct_str = if pt > 0 && rt > 0 && !item.is_audio() {
                        format!("{}%", pt * 100 / rt.max(1))
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

                    // Title truncated to leave room for indent + marker + right-aligned metadata.
                    let dur_visible = show_length && !dur.is_empty();
                    let pct_visible = !pct_str.is_empty();
                    let metadata_gap = if dur_visible && pct_visible { 1 } else { 0 };
                    let metadata_w = (if dur_visible { dur.width() } else { 0 })
                        + (if pct_visible { pct_str.width() } else { 0 })
                        + metadata_gap;
                    let extra = metadata_w;
                    let title_w =
                        render_w.saturating_sub(indent + 1 + extra + QUEUE_TITLE_QUIET_COLUMNS);
                    let title = trunc_str(&label, title_w);

                    // Now-playing title text is always emby blue, regardless of focus state.
                    let title_color = if is_active { palette::FOAM } else { fg };

                    let mut spans: Vec<Span> = Vec::new();
                    if indent > 0 {
                        spans.push(Span::raw(" "));
                    }
                    spans.push(marker);
                    // Prefix is "{n:>w}. " — render it dim.
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
                        spans.push(Span::styled(
                            title[split..].to_string(),
                            Style::default().fg(title_color),
                        ));
                    } else {
                        spans.push(Span::styled(title, Style::default().fg(title_color)));
                    }
                    if pct_visible || dur_visible {
                        let used: usize = spans.iter().map(|s| s.content.as_ref().width()).sum();
                        let pad = render_w.saturating_sub(used + metadata_w);
                        spans.push(Span::raw(" ".repeat(pad)));
                    }
                    if pct_visible {
                        let pct_color = if is_active {
                            palette::IRIS
                        } else {
                            palette::MUTED
                        };
                        spans.push(Span::styled(pct_str, Style::default().fg(pct_color)));
                    }
                    if pct_visible && dur_visible {
                        spans.push(Span::raw(" "));
                    }
                    if dur_visible {
                        let dur_color = dim_color;
                        spans.push(Span::styled(dur, Style::default().fg(dur_color)));
                    }

                    list_items.push(ListItem::new(Line::from(spans)).style(row_style));
                    layout.queue_row_map.push(Some(i));
                    line_offset += 1;
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
            super::render_power_scrollbar(f, area, max_off, offset);
        }
        header_ys
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn queue_group_start_row_includes_spacer_and_header() {
        let mut items = Vec::new();
        for i in 0..4 {
            let mut item = make_item(&format!("A{i}"), "Audio");
            item.id = format!("a-{i}");
            item.album_id = "album-a".into();
            item.album = "Album A".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        for i in 0..4 {
            let mut item = make_item(&format!("B{i}"), "Audio");
            item.id = format!("b-{i}");
            item.album_id = "album-b".into();
            item.album = "Album B".into();
            item.artist = "Artist".into();
            items.push(item);
        }

        let (display, _) = build_queue_rows(&items, true);

        assert_eq!(queue_group_start_row(&display, 7), 5);
    }

    #[test]
    fn render_power_queue_snaps_upward_scroll_to_group_start() {
        let mut app = make_app_stub();
        app.power_focus = crate::app::PowerFocus::Queue;

        let mut items = Vec::new();
        for i in 0..4 {
            let mut item = make_item(&format!("A{i}"), "Audio");
            item.id = format!("a-{i}");
            item.album_id = "album-a".into();
            item.album = "Album A".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        for i in 0..4 {
            let mut item = make_item(&format!("B{i}"), "Audio");
            item.id = format!("b-{i}");
            item.album_id = "album-b".into();
            item.album = "Album B".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        app.player_tab.set_items(items, 4);
        app.power_queue_scroll = 9;

        let backend = TestBackend::new(40, 3);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_queue(f, Rect::new(0, 0, 40, 3), true, &mut layout);
        })
        .unwrap();

        assert_eq!(app.power_queue_scroll, 5);
    }
}
