use super::confirm_modal::render_button_row;
use crate::components::modal_frame::render_modal_frame_bounds;
use mbv_theme as palette;
use mbv_ui_model::confirm::ConfirmButton;
use ratatui::Frame;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::Paragraph;

/// Geometry painted by the Twitch URL prompt modal, reused by its mouse
/// hit-testing (twitch-url-playback D7). The modal's click targets are the
/// button pills; the frame is the outside-click boundary.
#[derive(Debug)]
pub struct TwitchUrlRenderGeometry {
    pub frame: Rect,
    /// Painted button pill rects in button order (submit, cancel).
    pub buttons: Vec<Rect>,
}

/// Paints the Twitch URL prompt: title, the input line with a trailing
/// cursor, the shell-pushed refusal line when one is set, and the submit and
/// cancel pills (twitch-url-playback task 5.2). The refusal takes one extra
/// modal row; without it the modal keeps the save-playlist modal's shape.
pub fn render_twitch_url_content(
    f: &mut Frame,
    dim_backdrop_active: &mut bool,
    input: &str,
    refusal: Option<&str>,
) -> TwitchUrlRenderGeometry {
    let height = if refusal.is_some() { 8 } else { 7 };
    let modal = render_modal_frame_bounds(
        f,
        dim_backdrop_active,
        60,
        height,
        palette::surface_colors(palette::Surface::PopupFrame, false)
            .fill
            .to_ratatui(),
    );
    let inner = modal.inner;
    let label = "URL: ";
    let cursor = "▏";
    let max_input = inner.width as usize - label.len() - cursor.len() - 2;
    let visible: String = input
        .chars()
        .rev()
        .take(max_input)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    let input_line = format!("{label}{visible}{cursor}");
    let title = "Open Twitch URL";
    let buttons_y = inner.y + inner.height.saturating_sub(2);
    f.render_widget(
        Paragraph::new(Span::styled(
            title,
            Style::default()
                .fg(palette::Role::TextHeroTitle.color().to_ratatui())
                .add_modifier(Modifier::BOLD),
        ))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y,
            width: inner.width,
            height: 1,
        },
    );
    f.render_widget(
        Paragraph::new(Span::styled(
            input_line,
            Style::default().fg(palette::Role::TextStrong.color().to_ratatui()),
        ))
        .alignment(Alignment::Center),
        Rect {
            x: inner.x,
            y: inner.y + 1,
            width: inner.width,
            height: 1,
        },
    );
    if let Some(refusal) = refusal {
        f.render_widget(
            Paragraph::new(Span::styled(
                refusal,
                Style::default().fg(palette::Role::StatusError.color().to_ratatui()),
            ))
            .alignment(Alignment::Center),
            Rect {
                x: inner.x,
                y: inner.y + 2,
                width: inner.width,
                height: 1,
            },
        );
    }
    let buttons = vec![
        ConfirmButton::affirmative("Enter", "Submit"),
        ConfirmButton::cancel("Esc", "Cancel"),
    ];
    let buttons_rect = render_button_row(f, inner, buttons_y, &buttons);
    TwitchUrlRenderGeometry {
        frame: modal.outer,
        buttons: buttons_rect,
    }
}
