use super::super::super::palette;
use super::super::super::App;
use super::super::super::MultiSelectKind;
use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_multiselect_popup(&mut self, f: &mut Frame) {
        let Some(ref popup) = self.multiselect_popup else {
            return;
        };
        let title = match popup.kind {
            MultiSelectKind::HiddenLibraries => " Hidden Libraries ",
            MultiSelectKind::HiddenLatest => " Hidden Latest ",
            MultiSelectKind::FeedViewLibraries => " Feed View ",
            MultiSelectKind::MyLanguages => " My Languages ",
        };
        let max_name = popup
            .items
            .iter()
            .map(|(_, n, _)| n.len())
            .max()
            .unwrap_or(0);
        let inner_w = ((max_name + 6) as u16).clamp(36, 60);
        let width = inner_w + 2;
        let content_h = popup.items.len() as u16 + 1;
        let height = content_h + 2;

        let inner = render_modal_frame(
            f,
            &mut self.dim_backdrop_active,
            title,
            width,
            height,
            palette::SURFACE_FOCUSED,
        );

        let hint = "Space toggle  ·  Esc / Enter close";
        f.render_widget(
            Paragraph::new(Span::styled(hint, Style::default().fg(palette::TEXT_MUTED))),
            Rect {
                x: inner.x,
                y: inner.y,
                width: inner.width,
                height: 1,
            },
        );

        let list_area = Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: inner.height.saturating_sub(1),
        };
        let list_h = list_area.height as usize;
        let cursor = popup.cursor;
        let scroll = if cursor >= list_h {
            cursor + 1 - list_h
        } else {
            0
        };

        let lines: Vec<Line> = popup
            .items
            .iter()
            .enumerate()
            .skip(scroll)
            .take(list_h)
            .map(|(i, (_, name, is_hidden))| {
                let focused = i == cursor;
                let arrow = if focused { "▸ " } else { "  " };
                let check = if *is_hidden { "[x]" } else { "[ ]" };
                let check_style = if focused {
                    Style::default().fg(palette::TEXT_ACCENT_MUTED)
                } else {
                    Style::default().fg(palette::TEXT_MUTED)
                };
                let name_style = if focused {
                    Style::default().fg(palette::TEXT_PRIMARY)
                } else {
                    Style::default().fg(palette::TEXT_SECONDARY)
                };
                Line::from(vec![
                    Span::raw(arrow),
                    Span::styled(check, check_style),
                    Span::raw(" "),
                    Span::styled(name.clone(), name_style),
                ])
            })
            .collect();
        f.render_widget(Paragraph::new(lines), list_area);
    }
}
