/// geometry facts by hand and never touches `App`.
#[cfg(test)]
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn known_aspect_seeds_loading_reservation_and_first_paint_geometry() {
    let area = Rect::new(0, 0, 20, 10);
    let projection = QueueCardProjection {
        cache_key: Some("now-playing:P".into()),
        primary_image_aspect: Some(1.0),
        images_enabled: true,
        visualizer: false,
    };
    let reserved =
        queue_card_reserved_rect((0, 0), 40, area, true, projection.primary_image_aspect);
    assert_eq!(reserved.width, reserved.height);

    let mut term = Terminal::new(TestBackend::new(20, 10)).unwrap();
    let mut result = (0, 0, false);
    term.draw(|f| {
        result = render_card_painting(f, area, true, &projection, true, None, (0, 0), 40);
    })
    .unwrap();
    assert_eq!((result.0, result.1), (reserved.height, reserved.width));
}

#[test]
fn card_painting_paints_from_projected_state_without_app_access() {
    let mut term = Terminal::new(TestBackend::new(20, 10)).unwrap();
    let area = Rect::new(0, 0, 20, 10);

    // A projected artwork slot with a fetch in flight and no protocol
    // state yet: the painter reserves the loading rectangle and paints
    // the dim loading block.
    let projection = QueueCardProjection {
        cache_key: Some("now-playing:P".into()),
        primary_image_aspect: None,
        images_enabled: true,
        visualizer: false,
    };
    let mut result = (0u16, 0u16, false);
    term.draw(|f| {
        result = render_card_painting(f, area, false, &projection, true, None, (0, 0), 40);
    })
    .unwrap();
    let (height, width, loading) = result;
    assert!(loading, "an in-flight fetch reports image_loading");
    assert!(
        height > 0 && width > 0,
        "the loading reservation reserves rows"
    );
    let buf = term.backend().buffer();
    let loading_fill =
        crate::app::palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false)
            .fill;
    assert_eq!(
        buf[(0, 0)].style().bg,
        Some(loading_fill),
        "the projected loading slot paints the dim reservation block"
    );

    // Terminal images off: the slot reserves its last geometry and paints
    // nothing — no fetch, no placeholder state.
    let off = QueueCardProjection {
        cache_key: Some("now-playing:P".into()),
        primary_image_aspect: None,
        images_enabled: false,
        visualizer: false,
    };
    let mut term = Terminal::new(TestBackend::new(20, 10)).unwrap();
    let mut result = (0u16, 0u16, false);
    term.draw(|f| {
        result = render_card_painting(f, area, false, &off, false, None, (4, 6), 40);
    })
    .unwrap();
    let (height, width, loading) = result;
    assert!(!loading);
    assert_eq!(
        (height, width),
        (4, 6),
        "the images-off slot reports the last painted geometry"
    );
    assert_eq!(
        term.backend().buffer()[(0, 0)].style().bg,
        Some(ratatui::style::Color::Reset),
        "images-off paints nothing (untouched cell)"
    );

    // The placeholder slot with no cached state: the fallback rectangle
    // (the bundled placeholder's reservation) with no loading block.
    let placeholder = QueueCardProjection {
        cache_key: None,
        primary_image_aspect: None,
        images_enabled: true,
        visualizer: false,
    };
    let mut result = (0u16, 0u16, false);
    term.draw(|f| {
        result = render_card_painting(f, area, false, &placeholder, false, None, (0, 0), 40);
    })
    .unwrap();
    let (height, width, loading) = result;
    assert!(!loading);
    assert!(
        height > 0 && width > 0,
        "the empty-queue placeholder still reserves its fallback rectangle, got ({height},{width})"
    );
    assert_eq!(
        term.backend().buffer()[(0, 0)].style().bg,
        Some(ratatui::style::Color::Reset),
        "a not-loading placeholder paints no dim block"
    );
}
