use super::*;
use crate::components::chrome_player::{
    PlaybackControls, PlaybackStripAreas, TransportAvailability,
};

fn pill_context<'a>(
    playback: &'a mut PlaybackStripAreas,
    marquee_text: &'a mut String,
    marquee_started_at: &'a mut std::time::Instant,
    indicators: Option<Vec<Span<'static>>>,
) -> PlaybackRenderContext<'a> {
    PlaybackRenderContext {
        area: Rect::default(),
        playback,
        player_h: 2,
        controls: PlaybackControls {
            show: true,
            use_nerd_fonts: false,
            availability: TransportAvailability {
                stop: true,
                next: true,
                previous: true,
            },
            panel_focused: false,
            progress: (0, 0, false),
            idle_feed_title: None,
        },
        now_playing_title: None,
        panel: palette::Surface::PlaybackPanel,
        status_indicators: indicators,
        title_parts: None,
        marquee_text,
        marquee_started_at,
    }
}

/// The status pill paints the indicator cluster uppercased with caption fg on
/// captions, the producer fg kept on values, the pill bg on every span, and
/// a pad on both sides (issue #896: pins the one-pass `status_pill_spans`
/// rewrite against output drift).
#[test]
fn status_pill_spans_match_legacy_output_issue_896() {
    let pill_bg = palette::surface_colors(palette::Surface::PlaybackStatusPill, false).fill;
    let mut playback = PlaybackStripAreas::default();
    let mut marquee_text = String::new();
    let mut marquee_started_at = std::time::Instant::now();
    let ctx = pill_context(
        &mut playback,
        &mut marquee_text,
        &mut marquee_started_at,
        Some(vec![
            Span::styled("codec ", Style::default()),
            Span::styled("h264", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled("res ", Style::default()),
            Span::styled("1080p", Style::default().fg(Color::Red)),
        ]),
    );
    assert_eq!(
        padded_status_pill(&ctx),
        vec![
            Span::styled(" ", Style::default().bg(pill_bg)),
            Span::styled(
                "CODEC ",
                Style::default().fg(palette::PLAYBACK_META_FG).bg(pill_bg),
            ),
            Span::styled(
                "H264",
                Style::default()
                    .fg(palette::PLAYBACK_VALUE_FG)
                    .bg(pill_bg)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "RES ",
                Style::default().fg(palette::PLAYBACK_META_FG).bg(pill_bg),
            ),
            Span::styled("1080P", Style::default().fg(Color::Red).bg(pill_bg),),
            Span::styled(" ", Style::default().bg(pill_bg)),
        ]
    );
}
