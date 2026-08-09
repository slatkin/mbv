use super::widgets::{
    content_width, render_pill_bar, render_placeholder, render_right_scrollbar_with_viewport,
    PillBar,
};
use crate::app::layout::LayoutMain;
use crate::app::palette;
use crate::app::App;
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Format a tick count into a human-readable duration string.
fn format_duration(ticks: Option<u64>) -> String {
    match ticks {
        Some(t) if t > 0 => {
            let total_secs = t / TICKS_PER_SECOND as u64;
            let h = total_secs / 3600;
            let m = (total_secs % 3600) / 60;
            let s = total_secs % 60;
            if h > 0 {
                format!("{h}:{m:02}:{s:02}")
            } else {
                format!("{m}:{s:02}")
            }
        }
        _ => String::new(),
    }
}

/// Format a pub_date_secs value into a short date string for display.
fn format_pub_date(secs: Option<u64>) -> String {
    match secs {
        Some(s) => {
            // Simple YYYY-MM-DD from unix seconds
            let days = (s / 86400) as i64 + 719468;
            let era = if days >= 0 { days } else { days - 146096 } / 146097;
            let doe = days - era * 146097;
            let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
            let y = yoe + era * 400;
            let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
            let mp = (5 * doy + 2) / 153;
            let d = doy - (153 * mp + 2) / 5 + 1;
            let m = if mp < 10 { mp + 3 } else { mp - 9 };
            let yr = if m <= 2 { y + 1 } else { y };
            format!("{yr:04}-{m:02}-{d:02}")
        }
        None => String::new(),
    }
}

impl App {
    pub(super) fn render_feeds(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        if area.height == 0 {
            return;
        }

        let state = &self.feed_tab;
        let subscriptions = &state.subscriptions;
        let has_subs = !subscriptions.is_empty();
        let loading = state.loading;

        let max_y = area.y + area.height;
        let mut row = area.y;
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();

        // Pill bar: "All" + one per subscription.
        if row < max_y && has_subs {
            const MAX_LABEL: usize = 12;
            let mut labels: Vec<String> = vec!["All".to_string()];
            for sub in subscriptions {
                let name = if sub.name.len() > MAX_LABEL {
                    format!("{}…", &sub.name[..MAX_LABEL])
                } else {
                    sub.name.clone()
                };
                labels.push(name);
            }
            let ids: Vec<usize> = (0..labels.len()).collect();
            selector_tabs = render_pill_bar(
                f,
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: state.selected_group,
                    prefix: Some(" ⌘ "),
                },
            );
        }
        if row < max_y {
            row += 1;
        }
        if row < max_y {
            row += 1;
        }
        layout.selector_tabs = selector_tabs;

        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        layout.left_area = list_area;
        if list_area.height == 0 {
            return;
        }

        // Empty / help state.
        if !has_subs {
            render_placeholder(
                f,
                Rect {
                    x: list_area.x,
                    y: list_area.y,
                    width: list_area.width,
                    height: 1,
                },
                " No feed subscriptions configured",
            );
            return;
        }

        let entries = self.feed_tab.visible_entries();
        if entries.is_empty() {
            let msg = if loading {
                " Loading…"
            } else {
                " Press r to load feeds"
            };
            render_placeholder(
                f,
                Rect {
                    x: list_area.x,
                    y: list_area.y,
                    width: list_area.width,
                    height: 1,
                },
                msg,
            );
            return;
        }

        // Render the entry list.
        let n = entries.len();
        let cursor = self.feed_tab.cursor.min(n.saturating_sub(1));
        let scroll = self.feed_tab.scroll.min(cursor);
        let text_w_with_sb = (list_area.width as usize).saturating_sub(1);
        let text_w = content_width(list_area.width, true);
        let mut visible_count = 0usize;
        let mut row_map: Vec<Option<usize>> = Vec::with_capacity(list_area.height as usize);

        for (i, entry) in entries.iter().enumerate().skip(scroll) {
            if row >= list_area.y + list_area.height {
                break;
            }
            visible_count += 1;
            let selected = i == cursor;

            let selected_bg = if focused {
                palette::MEDIA_SELECTED_BG
            } else {
                palette::PLAYBACK_PANEL_BG
            };

            let bg = if selected {
                selected_bg
            } else {
                palette::LIBRARY_SIDE_BG
            };
            let fg = if selected {
                if focused {
                    palette::WHITE
                } else {
                    palette::SUBTLE
                }
            } else {
                palette::TEXT
            };

            let marker = if selected { "▶ " } else { "  " };
            let title = &entry.title;
            let duration = format_duration(entry.duration_ticks);
            let date = format_pub_date(entry.pub_date_secs);
            let mime = entry.mime_type.as_deref().unwrap_or("");

            // Build the display line.
            let mut spans = vec![
                Span::styled(
                    marker,
                    Style::default()
                        .fg(palette::AQUA)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{title}  "),
                    Style::default().fg(fg).add_modifier(if selected {
                        Modifier::BOLD
                    } else {
                        Modifier::empty()
                    }),
                ),
            ];
            if !duration.is_empty() {
                spans.push(Span::styled(
                    format!("{duration} "),
                    Style::default().fg(palette::PLAYBACK_META_FG),
                ));
            }
            if !date.is_empty() {
                spans.push(Span::styled(
                    format!("{date} "),
                    Style::default().fg(palette::MUTED),
                ));
            }
            if !mime.is_empty() {
                spans.push(Span::styled(
                    mime.to_string(),
                    Style::default().fg(palette::MUTED),
                ));
            }

            // Truncate to available width.
            let line = Line::from(spans);
            let display_w = text_w.min(text_w_with_sb) as u16;
            f.render_widget(
                Paragraph::new(line).style(Style::default().bg(bg)),
                Rect {
                    x: list_area.x,
                    y: row,
                    width: display_w,
                    height: 1,
                },
            );

            row_map.push(Some(i));
            row += 1;
        }
        row_map.resize(list_area.height as usize, None);
        layout.left_row_map = row_map;

        if visible_count > 0 && visible_count < n {
            render_right_scrollbar_with_viewport(f, list_area, n, visible_count, scroll);
        }
    }
}
