use crate::app::components::list::{ThreeLineFlatList, ThreeLineRole};
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

#[allow(dead_code)]
pub(in crate::app) fn render_three_line_flat_list<Target: Clone + Eq>(
    frame: &mut Frame,
    area: Rect,
    list: &mut ThreeLineFlatList<Target>,
) {
    list.begin_paint();
    if area.is_empty() {
        return;
    }
    let surface = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    frame.render_widget(
        Paragraph::new(" ").style(Style::default().bg(surface)),
        area,
    );
    let (offset, visible, gap) = list.painting_parts(area);
    let mut hits = Vec::with_capacity(visible);
    let mut selected_rect = None;
    for index in offset..offset + visible {
        let y = area.y + ((index - offset) * (3 + gap)) as u16;
        let rect = Rect::new(area.x, y, area.width, 3);
        let selected = list.focused() && list.selected() == Some(&list.item(index).target);
        if selected {
            selected_rect = Some(rect);
        }
        let bg = if selected {
            palette::SELECTED_ROW_BG
        } else if index % 2 == 1 {
            palette::surface_colors(palette::Surface::SidebarBand, false).fill
        } else {
            surface
        };
        for line_index in 0..3 {
            let line_area = Rect::new(area.x, y + line_index, area.width, 1);
            let line = Line::from(
                list.item(index).lines[line_index as usize]
                    .iter()
                    .map(|span| {
                        let fg = match span.role {
                            ThreeLineRole::Name => palette::TEXT_PRIMARY,
                            ThreeLineRole::Kind => palette::TEXT_EMPHASIS,
                            ThreeLineRole::Detail => palette::TEXT_MUTED,
                            ThreeLineRole::Status => palette::STATUS_AVAILABLE,
                            ThreeLineRole::Accent => palette::ACCENT_ACTIVE,
                        };
                        Span::styled(span.text.clone(), Style::default().fg(fg).bg(bg))
                    })
                    .collect::<Vec<_>>(),
            );
            frame.render_widget(
                Paragraph::new(line).style(Style::default().bg(bg)),
                line_area,
            );
        }
        hits.push((rect, list.item(index).target.clone()));
    }
    list.publish(area, hits, selected_rect);
}
