use super::super::super::palette;
use super::super::super::ui_util::{fmt_duration, trunc_str};
use super::super::super::App;
use super::super::super::SESSIONS_PANEL_W;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_sessions_overlay(&mut self, f: &mut Frame) {
        let content = Self::render_panel_shell(
            f,
            f.area(),
            SESSIONS_PANEL_W,
            "REMOTE SESSIONS",
            self.sessions_overlay_footer(),
        );
        let ix = content.x;
        let inner_w = content.width;
        let list_y = content.y;
        let list_h = content.height;
        let list_area = content;

        if self.sessions_loading && self.sessions.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " Loading…",
                    Style::default().fg(palette::SUBTLE),
                )),
                list_area,
            );
            return;
        }
        if self.sessions.is_empty() {
            f.render_widget(
                Paragraph::new(Span::styled(
                    " No other active sessions",
                    Style::default().fg(palette::SUBTLE),
                )),
                list_area,
            );
            return;
        }

        const CARD_H: u16 = 3;
        const DIV_H: u16 = 1;
        let entry_h = CARD_H + DIV_H;
        let visible_entries = ((list_h + DIV_H) / entry_h).max(1) as usize;
        self.sessions_cursor = self.sessions_cursor.min(self.sessions.len() - 1);
        if self.sessions_cursor < self.sessions_scroll {
            self.sessions_scroll = self.sessions_cursor;
        } else if self.sessions_cursor >= self.sessions_scroll + visible_entries {
            self.sessions_scroll = self.sessions_cursor + 1 - visible_entries;
        }
        self.sessions_scroll = self
            .sessions_scroll
            .min(self.sessions.len().saturating_sub(visible_entries));

        for (i, s) in self.sessions.iter().enumerate().skip(self.sessions_scroll) {
            let entry_y = list_y + (i - self.sessions_scroll) as u16 * entry_h;
            if entry_y + CARD_H > list_y + list_h {
                break;
            }

            let selected = i == self.sessions_cursor;
            let is_connected = self.connected_session_id.as_deref() == Some(s.id.as_str());
            let name_color = if selected {
                palette::IRIS
            } else {
                palette::TEXT
            };
            let dim = Style::default().fg(palette::MUTED);

            if selected {
                let bar: Vec<Line> = (0..CARD_H)
                    .map(|_| Line::from(Span::styled("▌", Style::default().fg(palette::PINE))))
                    .collect();
                f.render_widget(
                    Paragraph::new(bar),
                    Rect {
                        x: ix,
                        y: entry_y,
                        width: 1,
                        height: CARD_H,
                    },
                );
            }
            let text_x = ix + 2;
            let text_w = inner_w.saturating_sub(2) as usize;

            let badge = if is_connected { " ✚" } else { "" };
            let name_max = text_w.saturating_sub(badge.len());
            let name_line = Line::from(vec![
                Span::styled(
                    trunc_str(&s.device_name, name_max),
                    Style::default().fg(name_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(badge, Style::default().fg(palette::IRIS)),
            ]);
            f.render_widget(
                Paragraph::new(name_line),
                Rect {
                    x: text_x,
                    y: entry_y,
                    width: inner_w.saturating_sub(2),
                    height: 1,
                },
            );

            let meta = format!("{} · {}@{}", s.client, s.user_name, s.host);
            f.render_widget(
                Paragraph::new(Span::styled(
                    trunc_str(&meta, text_w),
                    dim.fg(palette::SUBTLE),
                )),
                Rect {
                    x: text_x,
                    y: entry_y + 1,
                    width: inner_w.saturating_sub(2),
                    height: 1,
                },
            );

            let state_icon = if s.now_playing.is_some() {
                if s.is_paused {
                    "⏸"
                } else {
                    "▶"
                }
            } else {
                "■"
            };
            let time = if s.now_playing.is_some() {
                format!(
                    " {}/{}",
                    fmt_duration(s.position_s),
                    fmt_duration(s.runtime_s)
                )
            } else {
                String::new()
            };
            let title = s.now_playing.as_deref().unwrap_or("idle");
            let playing = format!(
                "{} {}{}",
                state_icon,
                trunc_str(title, text_w.saturating_sub(11)),
                time
            );
            f.render_widget(
                Paragraph::new(Span::styled(trunc_str(&playing, text_w), dim)),
                Rect {
                    x: text_x,
                    y: entry_y + 2,
                    width: inner_w.saturating_sub(2),
                    height: 1,
                },
            );
        }
        // render_sidebar_scrollbar expects total/scroll in the same row units as
        // content.height, so convert from "entries" to rows (entry_h rows each).
        Self::render_sidebar_scrollbar(
            f,
            content,
            self.sessions.len() * entry_h as usize,
            self.sessions_scroll * entry_h as usize,
        );
    }
}
