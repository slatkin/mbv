use super::*;
use mbv_ui_model::playback::NowPlayingTitleSite;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

fn loading_projection() -> QueueCardProjection {
    QueueCardProjection {
        cache_key: Some("k".into()),
        plain_cache_key: None,
        images_enabled: true,
        visualizer: false,
        title_site: NowPlayingTitleSite::Header,
    }
}

/// The loading placeholder paints only inside the slot region: on a terminal
/// too short to offer the tier cap, the slot is shorter than the full-target
/// reservation, and an unclamped paint would overwrite the Queue panel below
/// it. The recorded size stays the full target either way.
#[test]
fn the_loading_placeholder_stays_inside_the_slot_region() {
    let mut terminal = Terminal::new(TestBackend::new(40, 30)).unwrap();
    let mut recorded = None;
    terminal
        .draw(|f| {
            recorded = Some(render_card_painting(
                f,
                Rect::new(0, 0, 40, 6),
                false,
                &loading_projection(),
                true,
                None,
                (0, 0),
                40,
            ));
        })
        .unwrap();
    let (recorded_h, recorded_w, loading, painted) = recorded.unwrap();

    let fill = palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill;
    let buf = terminal.backend().buffer();
    assert_eq!(
        buf[(0, 0)].style().bg,
        Some(fill),
        "the slot region is reserved"
    );
    for y in 6..buf.area.height {
        for x in 0..buf.area.width {
            assert_ne!(
                buf[(x, y)].style().bg,
                Some(fill),
                "no placeholder cell may paint below the slot region (row {y})"
            );
        }
    }
    // The record is the full target regardless of the shorter slot: the
    // square-estimate width bound (20) against the tier cap at terminal
    // height 40 (16).
    assert_eq!((recorded_h, recorded_w), (16, 40));
    assert!(loading);
    assert!(!painted);
}
