use super::chrome;
use mbv_theme as palette;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

#[cfg(test)]
mod tests;

/// Paint sidebar chrome and empty/loading content. Populated rows belong to
/// the embedded `ThreeLineFlatList` painter.
pub fn render_sessions_overlay_content(
    f: &mut Frame,
    area: Option<Rect>,
    target_count: usize,
    sessions_loading: bool,
    can_disconnect: bool,
    offers: mbv_ui_model::panel_targets::SessionTransferOffers,
) -> (Rect, Rect, bool) {
    let footer = if can_disconnect {
        "[↵]conn [d]disc [r]refresh [Esc]close"
    } else {
        "[↵]conn [r]refresh [Esc]close"
    };
    let panel_area = area.unwrap_or_else(|| f.area());
    let content = chrome::render_panel_shell_at(f, panel_area, "REMOTE SESSIONS", footer);
    // The queue-transfer keys (design D6) list on the spare second footer
    // row, only while the selected target offers them: the base hint row
    // already fills the queue column's minimum width, and a per-selection
    // hint must never clip the `[Esc]close` hint that row ends with.
    if offers.hand_off || offers.take {
        paint_transfer_hint_row(f, panel_area, offers);
    }

    if sessions_loading && target_count == 0 {
        f.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::Role::TextSecondary.color()),
            )),
            content,
        );
        return (panel_area, content, false);
    }
    if target_count == 0 {
        f.render_widget(
            Paragraph::new(Span::styled(
                " No sessions or cast receivers found",
                Style::default().fg(palette::Role::TextSecondary.color()),
            )),
            content,
        );
        return (panel_area, content, false);
    }
    (panel_area, content, true)
}

/// Paint one transfer key hint per offered action on the blank second
/// footer row `render_panel_shell_at` leaves under the hint row. Skipped
/// entirely when the panel is too short to have painted that row at all.
fn paint_transfer_hint_row(
    f: &mut Frame,
    panel_area: Rect,
    offers: mbv_ui_model::panel_targets::SessionTransferOffers,
) {
    if panel_area.height < 4 || panel_area.width < 4 {
        return;
    }
    let mut hints = String::new();
    if offers.hand_off {
        hints.push_str("[h]hand");
    }
    if offers.take {
        if !hints.is_empty() {
            hints.push(' ');
        }
        hints.push_str("[t]take");
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            hints,
            Style::default().fg(palette::Role::TextPrimary.color()),
        ))
        .style(
            Style::default().bg(palette::surface_colors(palette::Surface::SidebarBody, false).fill),
        ),
        Rect {
            x: panel_area.x + 2,
            y: panel_area.y + panel_area.height - 1,
            width: panel_area.width - 4,
            height: 1,
        },
    );
}

/// Keep the sidebar's established thumb-only scroll indicator in item units.
pub fn render_sessions_scrollbar(
    f: &mut Frame,
    content: Rect,
    items: usize,
    offset: usize,
    gap: u16,
) {
    let stride = 3 + gap as usize;
    chrome::render_sidebar_scrollbar(f, content, items * stride, offset * stride);
}
