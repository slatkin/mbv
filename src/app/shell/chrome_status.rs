//! Status-bar span builders (task 2.2, design D3).
//!
//! The span builders the shell projects into the mounted `StatusBarPanel`
//! Interactive Component. They build ratatui values, so they are shell free
//! functions over `App` reads, moved here from `state/projection`. The
//! playback-target labels and the in-app toast gate they read build no
//! ratatui value and stay in `state::projection::chrome_status`.
//!
//! The Local/Peer queue-link pills are queue concern and paint in the
//! `QueueColumn` footer (`render_queue_status`), never here.

use crate::app::{App, PanelFocus};
use mbv_render::components::widgets::service_state_color;
use mbv_render::indicators as indicator_paint;
use mbv_tui_support::ToRatatui;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

use super::Model;

/// Build the playback status indicator items (res/codec, audio lang, CC), space-separated.
/// Returns None if the local player is not active.
/// Callers wrap these in [ ... ] with whatever surrounding style they need.
pub(in crate::app) fn build_status_indicator_spans(app: &App) -> Option<Vec<Span<'static>>> {
    let data = crate::app::LocalPlaybackTarget::indicator_data(app)?;
    Some(indicator_paint::indicator_spans(
        app.indicator_style(),
        &data,
        app.use_nerd_fonts(),
    ))
}

pub(in crate::app) fn mute_status_spans(app: &App) -> Option<Vec<Span<'static>>> {
    crate::app::LocalPlaybackTarget::displayed_mute(app).then(|| {
        vec![
            Span::styled(
                " ",
                Style::default().bg(mbv_theme::surface_colors(
                    mbv_theme::Surface::StatusBarPill,
                    false,
                )
                .fill
                .to_ratatui()),
            ),
            Span::styled(
                "muted",
                Style::default()
                    .fg(mbv_theme::Role::StatusError.color().to_ratatui())
                    .bg(
                        mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                            .fill
                            .to_ratatui(),
                    )
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " ",
                Style::default().bg(mbv_theme::surface_colors(
                    mbv_theme::Surface::StatusBarPill,
                    false,
                )
                .fill
                .to_ratatui()),
            ),
        ]
    })
}

pub(in crate::app) fn volume_status_spans(app: &App) -> Vec<Span<'static>> {
    let volume = crate::app::LocalPlaybackTarget::displayed_volume(app);
    // Speaker glyph reflects the volume state (0 / low / mid / high).
    let icon = if volume == 0 {
        "\u{1F507}"
    } else if volume <= 25 {
        "\u{1F508}"
    } else if volume <= 75 {
        "\u{1F509}"
    } else {
        "\u{1F50A}"
    };
    vec![
        Span::styled(
            " ",
            Style::default().bg(mbv_theme::surface_colors(
                mbv_theme::Surface::StatusBarPill,
                false,
            )
            .fill
            .to_ratatui()),
        ),
        Span::styled(
            icon,
            Style::default()
                .fg(mbv_theme::Role::PlaybackMetaFg.color().to_ratatui())
                .bg(
                    mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                        .fill
                        .to_ratatui(),
                ),
        ),
        Span::styled(
            format!(" {volume}"),
            Style::default()
                .fg(mbv_theme::Role::Accent.color().to_ratatui())
                .bg(
                    mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                        .fill
                        .to_ratatui(),
                )
                .add_modifier(Modifier::BOLD),
        ),
    ]
}

/// The status row's right segment: queue-source link label, username,
/// and the service-state glyphs (Emby,
/// Audiobookshelf, stay-alive) — always visible, coloured by state. Built
/// shell-side (task 2.2); the mounted `StatusBarPanel` positions and
/// paints it.
pub(in crate::app) fn status_bar_right_spans(model: &Model) -> Vec<Span<'static>> {
    let app = &model.app;
    let (username, stay_alive) = {
        let username = app.configured_username();
        let stay_alive = app.stay_alive_configured();
        (username, stay_alive)
    };
    // Stay-alive indicator: red whenever this process runs in stay-alive
    // mode (the daemon persists after quit), yellow when that daemon is
    // lost, grey when stay-alive is off. Deliberately independent of the
    // current player target: a stay-alive client that routes playback to
    // another daemon is still in stay-alive mode.
    let alive_color = if !stay_alive {
        mbv_theme::Role::TextMuted.color().to_ratatui()
    } else if app.player_remote_disconnected() {
        mbv_theme::Role::TextFocusAccent.color().to_ratatui()
    } else {
        mbv_theme::Role::StatusError.color().to_ratatui()
    };
    let mut right_spans: Vec<Span> = Vec::new();
    let source_label = queue_source_status_label(
        app.playback_queue().source(),
        matches!(model.app.effective_panel_focus(), PanelFocus::Queue),
    );
    let append_right = |right_spans: &mut Vec<Span<'static>>, span: Span<'static>| {
        if !right_spans.is_empty() {
            right_spans.push(Span::raw(" "));
        }
        right_spans.push(span);
    };
    if let Some((label, color)) = source_label {
        append_right(
            &mut right_spans,
            Span::styled(
                format!(" {label} "),
                Style::default().fg(color).bg(mbv_theme::surface_colors(
                    mbv_theme::Surface::StatusBarPill,
                    false,
                )
                .fill
                .to_ratatui()),
            ),
        );
    }
    if !username.is_empty() {
        if !right_spans.is_empty() {
            right_spans.push(Span::raw(" "));
        }
        right_spans.push(Span::styled(
            " 🯅",
            Style::default()
                .fg(mbv_theme::Role::TextMetadata.color().to_ratatui())
                .bg(
                    mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                        .fill
                        .to_ratatui(),
                ),
        ));
        right_spans.push(Span::styled(
            format!(" {username} "),
            Style::default()
                .fg(mbv_theme::Role::PlaybackMetaFg.color().to_ratatui())
                .bg(
                    mbv_theme::surface_colors(mbv_theme::Surface::StatusBarPill, false)
                        .fill
                        .to_ratatui(),
                ),
        ));
    }
    // Service-state indicators — Emby and Audiobookshelf coloured by
    // state (brand colour when ready, grey when not configured, error
    // otherwise), stay-alive daemon lost = yellow. One leading space per
    // indicator, no trailing space. Without Nerd Fonts the Emby and
    // Audiobookshelf private-use code points fall back to the plain labels
    // the sessions panel uses (bug fix: the service glyphs ignored
    // `use_nerd_fonts` and painted as missing-glyph boxes under the default
    // config).
    right_spans.extend([
        Span::raw(" "),
        Span::styled(
            if app.use_nerd_fonts() {
                "\u{F06B4}"
            } else {
                "EMBY"
            },
            Style::default().fg(service_state_color(
                app.emby_service_state(),
                mbv_theme::Role::Accent.color().to_ratatui(),
            )),
        ),
        Span::raw(" "),
        Span::styled(
            if app.use_nerd_fonts() {
                "\u{EDE2}"
            } else {
                "ABS"
            },
            Style::default().fg(service_state_color(
                app.audiobookshelf_service_state(),
                mbv_theme::Role::AccentAudiobookshelf.color().to_ratatui(),
            )),
        ),
        Span::raw(" "),
        Span::styled(
            if app.use_nerd_fonts() {
                "\u{f004}"
            } else {
                "\u{2665}"
            },
            Style::default().fg(alive_color),
        ),
        Span::raw(" "),
    ]);
    // Peer queue link is omitted here: the active queue is already
    // apparent from the queue UI.
    right_spans
}

fn queue_source_status_label(
    source: &mbv_queue::QueueSource,
    queue_focused: bool,
) -> Option<(String, Color)> {
    if !queue_focused {
        return None;
    }
    let label = match source {
        mbv_queue::QueueSource::Album => "ALBUM".to_string(),
        mbv_queue::QueueSource::Series => "SERIES".to_string(),
        // Shuffle is no pill (2026-10-09 user rule): the shuffle-sourced
        // queue's pill was never requested, so it reads as no label at all.
        mbv_queue::QueueSource::Shuffle => return None,
        mbv_queue::QueueSource::Remote => "REMOTE Q".to_string(),
        mbv_queue::QueueSource::Collection { collection_type } => collection_type.to_uppercase(),
        mbv_queue::QueueSource::Playlist { .. } | mbv_queue::QueueSource::Unknown => {
            return None;
        }
    };
    Some((label, mbv_theme::Role::TextMuted.color().to_ratatui()))
}

#[cfg(test)]
mod tests {
    use crate::app::tests::make_app_stub;
    use mbv_render::components::widgets::service_state_color;
    use mbv_tui_support::ToRatatui;

    /// With the default (non-Nerd-Font) config, the service indicators emit
    /// the plain text labels instead of Nerd Font private-use code points,
    /// which would paint as missing-glyph boxes (bug: the service glyphs
    /// ignored `use_nerd_fonts`).
    #[test]
    fn service_indicators_fall_back_to_text_without_nerd_fonts() {
        let model = super::Model::new(make_app_stub());
        assert!(
            !model.app.use_nerd_fonts(),
            "the stub defaults to no Nerd Fonts"
        );

        let spans = super::status_bar_right_spans(&model);

        assert!(
            !spans
                .iter()
                .any(|span| span.content.contains('\u{F06B4}')
                    || span.content.contains('\u{EDE2}')),
            "no Nerd Font private-use glyph without use_nerd_fonts"
        );
        assert!(spans.iter().any(|span| span.content.contains("EMBY")));
        assert!(spans.iter().any(|span| span.content.contains("ABS")));
    }

    /// The status row's Audiobookshelf glyph colours through the same
    /// `service_state_color` policy as the Emby glyph (issue #976: it was
    /// always painted `AccentAudiobookshelf`, so an unconfigured, connecting,
    /// or failed Service looked healthy).
    #[rstest::rstest]
    #[case::not_configured(mbv_core::service_runtime::ServiceState::NotConfigured)]
    #[case::ready(mbv_core::service_runtime::ServiceState::Ready)]
    #[case::unavailable(mbv_core::service_runtime::ServiceState::Unavailable)]
    fn the_abs_glyph_colour_follows_the_audiobookshelf_service_state(
        #[case] state: mbv_core::service_runtime::ServiceState,
    ) {
        let mut app = make_app_stub();
        app.seed_audiobookshelf_service_state_for_test(state);
        let model = super::Model::new(app);

        let expected = service_state_color(
            state,
            mbv_theme::Role::AccentAudiobookshelf.color().to_ratatui(),
        );

        let spans = super::status_bar_right_spans(&model);
        let abs = spans
            .iter()
            .find(|span| span.content.contains("ABS"))
            .expect("the status row shows the Audiobookshelf glyph");
        assert_eq!(abs.style.fg, Some(expected));
    }
}
