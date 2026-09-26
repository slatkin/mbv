use super::chrome;
use crate::app::infra::palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Paint sidebar chrome and empty/loading content. Populated rows belong to
/// the embedded `ThreeLineFlatList` painter.
pub(in crate::app) fn render_sessions_overlay_content(
    f: &mut Frame,
    area: Option<Rect>,
    target_count: usize,
    sessions_loading: bool,
    can_disconnect: bool,
) -> (Rect, Rect, bool) {
    let footer = if can_disconnect {
        "[↵]conn [d]disc [r]refresh [Esc]close"
    } else {
        "[↵]conn [r]refresh [Esc]close"
    };
    let panel_area = area.unwrap_or_else(|| f.area());
    let content = chrome::render_panel_shell_at(f, panel_area, "REMOTE SESSIONS", footer);

    if sessions_loading && target_count == 0 {
        f.render_widget(
            Paragraph::new(Span::styled(
                " Loading…",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return (panel_area, content, false);
    }
    if target_count == 0 {
        f.render_widget(
            Paragraph::new(Span::styled(
                " No sessions or cast receivers found",
                Style::default().fg(palette::TEXT_SECONDARY),
            )),
            content,
        );
        return (panel_area, content, false);
    }
    (panel_area, content, true)
}

/// Keep the sidebar's established thumb-only scroll indicator in item units.
pub(in crate::app) fn render_sessions_scrollbar(
    f: &mut Frame,
    content: Rect,
    items: usize,
    offset: usize,
    gap: u16,
) {
    let stride = 3 + gap as usize;
    chrome::render_sidebar_scrollbar(f, content, items * stride, offset * stride);
}
