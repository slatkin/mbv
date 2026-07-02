use super::super::super::palette;
use super::super::super::App;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem};
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_context_menu(&mut self, f: &mut Frame) {
        let Some(ref menu) = self.context_menu else {
            self.context_menu_rect = None;
            return;
        };
        let width = (menu.items.iter().map(|s| s.len()).max().unwrap_or(4) + 4) as u16;
        let height = menu.items.len() as u16 + 2;
        let full = f.area();
        let x = menu.x.min(full.width.saturating_sub(width));
        let y = menu.y.min(full.height.saturating_sub(height));
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        self.context_menu_rect = Some(rect);
        f.render_widget(Clear, rect);
        f.render_widget(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(palette::IRIS)),
            rect,
        );
        let list_items: Vec<ListItem> = menu
            .items
            .iter()
            .enumerate()
            .map(|(i, &label)| {
                let style = if i == menu.cursor {
                    Style::default().fg(palette::BASE).bg(palette::IRIS)
                } else {
                    Style::default().fg(palette::TEXT)
                };
                ListItem::new(format!(" {label} ")).style(style)
            })
            .collect();
        let inner = Rect {
            x: x + 1,
            y: y + 1,
            width: width - 2,
            height: height - 2,
        };
        f.render_widget(List::new(list_items), inner);
    }
}
