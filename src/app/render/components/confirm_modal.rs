use crate::app::render::components::modal_frame::render_modal_frame;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

/// Paint the confirm modal: centered 60×7 frame with message + hint lines.
///
/// Extracted from `impl App::render_confirm_modal` so the Interactive
/// Component (`src/app/components/confirm.rs`) can call it without an `App`
/// reference (design D9). The `dim_flag` is set by `render_modal_frame` (same
/// as the `App` path used before migration).
//
// `pub(in crate::app)` so the Interactive Component can call it.
pub(in crate::app) fn render_confirm_modal_content(
    f: &mut Frame,
    dim_flag: &mut bool,
    title: &str,
    message: &str,
    hint: &str,
) {
    let inner = render_modal_frame(
        f,
        dim_flag,
        title,
        60,
        7,
        super::super::super::palette::SURFACE_FOCUSED,
    );
    let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
    f.render_widget(
        Paragraph::new(Span::styled(
            message,
            Style::default().fg(super::super::super::palette::TEXT_STRONG),
        )),
        Rect {
            x: inner.x + 1,
            y: base_y,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            hint,
            Style::default().fg(super::super::super::palette::TEXT_SECONDARY),
        )),
        Rect {
            x: inner.x + 1,
            y: base_y + 2,
            width: inner.width.saturating_sub(2),
            height: 1,
        },
    );
}
