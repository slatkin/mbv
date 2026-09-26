use crate::app::components::list::{ThreeLineFlatList, ThreeLineRole};
use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// `claim_rect` is the owning panel's full-width span (so the selected row's
/// bar reaches the panel edges like every other selected-row paint);
/// `content_rect` is the inset row-flow span that owns text and hit geometry.
/// Zebra and ordinary rows stay inside `content_rect`; only the selected row
/// claims the full width (the `tree_browser` precedent).
pub(in crate::app) fn render_three_line_flat_list<Target: Clone + Eq>(
    frame: &mut Frame,
    claim_rect: Rect,
    content_rect: Rect,
    list: &mut ThreeLineFlatList<Target>,
) {
    list.begin_paint();
    if content_rect.is_empty() {
        return;
    }
    let surface = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    frame.render_widget(
        Paragraph::new(" ").style(Style::default().bg(surface)),
        content_rect,
    );
    let left_inset = content_rect.x.saturating_sub(claim_rect.x);
    let (offset, visible, gap) = list.painting_parts(content_rect);
    let mut hits = Vec::with_capacity(visible);
    let mut selected_rect = None;
    for index in offset..offset + visible {
        let y = content_rect.y + u16::try_from((index - offset) * (3 + gap)).unwrap_or(u16::MAX);
        let selected = list.focused() && list.selected() == Some(&list.item(index).target);
        let (row_x, row_width) = if selected {
            (claim_rect.x, claim_rect.width)
        } else {
            (content_rect.x, content_rect.width)
        };
        let rect = Rect::new(row_x, y, row_width, 3);
        let bg = if selected {
            palette::SELECTED_ROW_BG
        } else if index % 2 == 1 {
            palette::SESSIONS_STRIPE_BG
        } else {
            surface
        };
        for line_index in 0..3 {
            let line_area = Rect::new(row_x, y + line_index, row_width, 1);
            let mut spans = Vec::new();
            if selected && left_inset > 0 {
                spans.push(Span::raw(" ".repeat(left_inset as usize)));
            }
            spans.extend(
                list.item(index).lines[line_index as usize]
                    .iter()
                    .map(|span| {
                        let fg = if selected
                            && !matches!(span.role, ThreeLineRole::Accent | ThreeLineRole::Badge(_))
                        {
                            palette::SELECTED_ROW_FG
                        } else {
                            match span.role {
                                ThreeLineRole::Name => palette::TEXT_PRIMARY,
                                ThreeLineRole::Kind => palette::TEXT_EMPHASIS,
                                ThreeLineRole::Detail => palette::TEXT_MUTED,
                                ThreeLineRole::Status => palette::STATUS_AVAILABLE,
                                ThreeLineRole::Accent => palette::ACCENT_ACTIVE,
                                ThreeLineRole::Badge(color) => color,
                            }
                        };
                        Span::styled(span.text.clone(), Style::default().fg(fg).bg(bg))
                    })
                    .collect::<Vec<_>>(),
            );
            frame.render_widget(
                Paragraph::new(Line::from(spans)).style(Style::default().bg(bg)),
                line_area,
            );
        }
        hits.push((rect, list.item(index).target.clone()));
        if selected {
            selected_rect = Some(rect);
        }
    }
    list.publish(claim_rect, content_rect, hits, selected_rect);
}
