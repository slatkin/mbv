use super::super::palette;
use super::super::App;
use super::super::LogPane;
use crate::applog::Level;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

impl App {
    pub(super) fn render_log(&self, f: &mut Frame, area: Rect) {
        let [hint_area, body] =
            Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);

        let k = Style::default()
            .fg(palette::YELLOW)
            .add_modifier(Modifier::BOLD);
        let m = Style::default().fg(palette::MUTED);
        let sp = "  ";
        let hints = Line::from(vec![
            Span::raw(sp),
            Span::styled("←→", k),
            Span::styled(" pane", m),
            Span::raw(sp),
            Span::styled("↑↓", k),
            Span::styled(" scroll", m),
            Span::raw(sp),
            Span::styled("PgUp/Dn", k),
            Span::styled(" page", m),
            Span::raw(sp),
            Span::styled("Space", k),
            Span::styled(" toggle source", m),
            Span::raw(sp),
            Span::styled("c", k),
            Span::styled(" copy to clipboard", m),
        ]);
        f.render_widget(Paragraph::new(hints), hint_area);

        let sources = self.log_sources();
        let src_w = (sources.iter().map(|s| s.len()).max().unwrap_or(4) + 4) as u16;

        let [src_area, log_area] =
            Layout::horizontal([Constraint::Length(src_w), Constraint::Min(10)]).areas(body);

        let src_focused = self.log_pane == LogPane::Sources;
        let src_border = if src_focused {
            palette::IRIS
        } else {
            palette::OVERLAY
        };
        let src_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(src_border))
            .title(Span::styled(
                " Sources ",
                Style::default()
                    .fg(palette::SUBTLE)
                    .add_modifier(Modifier::BOLD),
            ));
        let src_inner = src_block.inner(src_area);
        f.render_widget(src_block, src_area);

        let src_cursor = self.log_source_cursor.min(sources.len().saturating_sub(1));
        for (i, src) in sources.iter().enumerate() {
            let y = src_inner.top() + i as u16;
            if y >= src_inner.bottom() {
                break;
            }
            let disabled = self.log_disabled_sources.contains(src);
            let selected = i == src_cursor && src_focused;
            let fg = if disabled {
                palette::OVERLAY
            } else if selected {
                palette::YELLOW
            } else {
                palette::SUBTLE
            };
            let prefix = if disabled { "○ " } else { "● " };
            let dot_color = if disabled {
                palette::OVERLAY
            } else {
                palette::IRIS
            };
            f.buffer_mut().set_stringn(
                src_inner.left(),
                y,
                prefix,
                2,
                Style::default().fg(dot_color),
            );
            f.buffer_mut().set_stringn(
                src_inner.left() + 2,
                y,
                src,
                src_inner.width as usize,
                Style::default().fg(fg),
            );
        }

        let log_focused = self.log_pane == LogPane::Log;
        let log_border = if log_focused {
            palette::IRIS
        } else {
            palette::OVERLAY
        };
        let entries = self.visible_log_entries();
        let n = entries.len();
        let log_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(log_border))
            .title(Span::styled(
                format!(" Log ({n}) "),
                Style::default()
                    .fg(palette::SUBTLE)
                    .add_modifier(Modifier::BOLD),
            ));
        let log_inner = log_block.inner(log_area);
        f.render_widget(log_block, log_area);

        let visible = log_inner.height as usize;
        let max_scroll = n.saturating_sub(visible);
        let scroll = self.log_scroll.min(max_scroll);
        let first = max_scroll.saturating_sub(scroll);

        for (row, entry) in entries.iter().skip(first).take(visible).enumerate() {
            let y = log_inner.top() + row as u16;
            let (level_color, label) = match entry.level {
                Level::Error => (Color::Red, "E"),
                Level::Warn => (Color::Yellow, "W"),
                Level::Info => (palette::WHITE, "I"),
                Level::Debug => (palette::SUBTLE, "D"),
            };
            let w = log_inner.width as usize;
            let mut x = log_inner.left();
            f.buffer_mut()
                .set_stringn(x, y, label, 1, Style::default().fg(level_color));
            x += 1;
            f.buffer_mut()
                .set_stringn(x, y, "│", 1, Style::default().fg(palette::OVERLAY));
            x += 1;
            f.buffer_mut()
                .set_stringn(x, y, &entry.ts, 8, Style::default().fg(palette::MUTED));
            x += 9;
            if x >= log_inner.right() {
                continue;
            }
            f.buffer_mut()
                .set_stringn(x, y, "│", 1, Style::default().fg(palette::OVERLAY));
            x += 1;
            let src_len = entry.source.len().min(6);
            f.buffer_mut().set_stringn(
                x,
                y,
                &entry.source,
                src_len,
                Style::default().fg(palette::MUTED),
            );
            x += 6 + 1;
            if x >= log_inner.right() {
                continue;
            }
            f.buffer_mut()
                .set_stringn(x, y, "│", 1, Style::default().fg(palette::OVERLAY));
            x += 1;
            if x < log_inner.right() {
                let remaining = (log_inner.right() - x) as usize;
                let msg_w = remaining.min(w);
                f.buffer_mut().set_stringn(
                    x,
                    y,
                    &entry.msg,
                    msg_w,
                    Style::default().fg(level_color),
                );
            }
        }
    }
}
