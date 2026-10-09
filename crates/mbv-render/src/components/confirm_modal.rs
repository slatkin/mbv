use crate::components::modal_frame::render_modal_frame_bounds;
use mbv_theme as palette;
use mbv_ui_model::confirm::{ConfirmButton, ConfirmButtonTone};
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthStr;

/// The confirm modal's pill buttons were added in issue #855.
///
/// Geometry painted by the confirm modal, reused by its mouse hit-testing.
#[derive(Debug)]
pub struct ConfirmRenderGeometry {
    /// The painted modal rect — the outside-click boundary.
    pub frame: Rect,
    /// Painted button pill rects in button order.
    pub buttons: Vec<Rect>,
}

/// Paint the confirm modal: centered frame around a 60-wide content area with
/// the message and the button row.
///
/// Extracted from `App::render_confirm_modal` so the Interactive
/// Component (`crates/mbv-components/src/confirm.rs`) can call it without an `App`
/// reference (design D9). The `dim_flag` is set by `render_modal_frame` (same
/// as the `App` path used before migration).
//
// `pub(crate)` so the Interactive Component can call it.
pub fn render_confirm_modal_content(
    f: &mut Frame,
    dim_flag: &mut bool,
    message: &str,
    buttons: &[ConfirmButton],
) -> ConfirmRenderGeometry {
    let modal = render_modal_frame_bounds(
        f,
        dim_flag,
        60,
        7,
        mbv_theme::surface_colors(mbv_theme::Surface::PopupFrame, false).fill,
    );
    let inner = modal.inner;
    let base_y = inner.y + (inner.height.saturating_sub(3)) / 2;
    f.render_widget(
        Paragraph::new(Span::styled(
            message,
            Style::default().fg(mbv_theme::Role::TextStrong.color()),
        ))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: base_y,
            width: inner.width,
            height: 1,
        },
    );
    let buttons_rect = render_button_row(f, inner, base_y + 2, buttons);
    ConfirmRenderGeometry {
        frame: modal.outer,
        buttons: buttons_rect,
    }
}

/// The pill's painted cell width: `" {keys} {label} "`, one pad space each
/// side of the text.
fn button_width(button: &ConfirmButton) -> u16 {
    let text = button.keys.width() + button.label.width() + 3;
    u16::try_from(text).unwrap_or(u16::MAX)
}

fn button_fg(tone: ConfirmButtonTone) -> Color {
    match tone {
        ConfirmButtonTone::Affirmative => palette::Role::AccentActive.color(),
        ConfirmButtonTone::Cancel => palette::Role::StatusError.color(),
    }
}

/// Paint the centered button pills and return each painted rect in button
/// order. Pills are ink blocks with one pad space each side, separated
/// by two cells, the whole group centered on `y` (issue #855).
pub(crate) fn render_button_row(
    f: &mut Frame,
    inner: Rect,
    y: u16,
    buttons: &[ConfirmButton],
) -> Vec<Rect> {
    let widths: Vec<u16> = buttons.iter().map(button_width).collect();
    let gaps = u16::try_from(buttons.len().saturating_sub(1).saturating_mul(2)).unwrap_or(u16::MAX);
    let total = widths
        .iter()
        .fold(gaps, |total, width| total.saturating_add(*width));
    let pad_left = inner.width.saturating_sub(total) / 2;
    let mut x = inner.x.saturating_add(pad_left);
    let mut geometry = Vec::with_capacity(buttons.len());
    for (button, width) in buttons.iter().zip(widths) {
        let rect = Rect {
            x,
            y,
            width,
            height: 1,
        };
        f.render_widget(
            Paragraph::new(Span::styled(
                format!(" {} {} ", button.keys, button.label),
                Style::default()
                    .bg(palette::surface_colors(palette::Surface::ModalButton, false).fill)
                    .fg(button_fg(button.tone)),
            )),
            rect,
        );
        geometry.push(rect);
        x = x.saturating_add(width).saturating_add(2);
    }
    geometry
}
