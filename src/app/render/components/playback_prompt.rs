use super::chrome::toast_line;
use crate::app::palette;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Clear, Paragraph};
use ratatui::Frame;

pub(in crate::app) fn render_playback_prompt_content(
    f: &mut Frame,
    area: Rect,
    status: &str,
    visible: bool,
) {
    if !visible || status.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(Clear, area);
    f.render_widget(
        Paragraph::new(toast_line(status, palette::TEXT_PRIMARY))
            .alignment(Alignment::Center)
            .style(
                Style::default()
                    .fg(palette::TEXT_PRIMARY)
                    .bg(palette::SURFACE_CHROME),
            ),
        area,
    );
}
