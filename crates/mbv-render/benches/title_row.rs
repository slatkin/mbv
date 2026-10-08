//! Baseline bench for the status-pill title row (issue #896, `M-HOTPATH`).
//!
//! Drives the private `status_pill_spans` through its public painter,
//! `render_title_row`, onto a `TestBackend` buffer: the same path the
//! Library strip paints once per frame. No visibility was widened to reach
//! it (`render_title_row` is already public).

use std::hint::black_box;
use std::time::Instant;

use criterion::{Criterion, criterion_group, criterion_main};
use mbv_render::components::chrome_player::{
    PlaybackControls, PlaybackRenderContext, PlaybackStripAreas, TransportAvailability,
    render_title_row,
};
use mbv_theme::Surface;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::Span;

/// The codec/resolution/audio/subtitle cluster the shell projects while
/// media plays: lowercase captions with trailing spaces plus their values,
/// exercising the pill's uppercase and caption-fg passes.
fn status_indicators() -> Vec<Span<'static>> {
    let caption = Style::default();
    let value = Style::default();
    vec![
        Span::styled("codec ", caption),
        Span::styled("h264", value),
        Span::styled("res ", caption),
        Span::styled("1080p", value),
        Span::styled("aud ", caption),
        Span::styled("eng", value),
        Span::styled("sub ", caption),
        Span::styled("eng", value),
    ]
}

fn bench_title_row(c: &mut Criterion) {
    let indicators = status_indicators();
    let area = Rect {
        x: 0,
        y: 0,
        width: 100,
        height: 1,
    };
    let mut terminal =
        Terminal::new(TestBackend::new(100, 1)).expect("test backend fits a 100x1 strip");
    c.bench_function("status_pill_title_row", |bencher| {
        bencher.iter(|| {
            let mut playback = PlaybackStripAreas::default();
            let mut marquee_text = String::new();
            let mut marquee_started_at = Instant::now();
            let mut ctx = PlaybackRenderContext {
                area,
                playback: &mut playback,
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
                    progress: (0, 3_600_000_000, false),
                    idle_feed_title: None,
                },
                now_playing_title: Some((String::from("Some episode title"), Color::White)),
                panel: Surface::PlaybackPanel,
                status_indicators: Some(indicators.clone()),
                title_parts: None,
                marquee_text: &mut marquee_text,
                marquee_started_at: &mut marquee_started_at,
            };
            terminal
                .draw(|frame| {
                    render_title_row(
                        frame,
                        area,
                        black_box("Some episode title"),
                        Color::White,
                        &mut ctx,
                    );
                })
                .expect("title row draws on a 100x1 backend");
        });
    });
}

criterion_group!(benches, bench_title_row);
criterion_main!(benches);
