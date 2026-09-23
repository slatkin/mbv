/// geometry facts by hand and never touches `App`.
#[cfg(test)]
use super::*;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn initial_reservation_matches_square_artwork_in_a_width_limited_slot() {
    let area = Rect::new(0, 0, 36, 30);
    // 50 rows of terminal height puts the cap (24) above the slot, so the
    // width-limited measurement below is what shapes the reservation.
    let reserved = queue_card_reserved_rect((0, 0), 50, area, false);
    let image = image::DynamicImage::new_rgb8(400, 400);
    let measured = ratatui_image::Resize::Scale(Some(RENDER_FILTER)).size_for(
        &image,
        ratatui_image::FontSize::new(10, 20),
        area.as_size(),
    );

    assert_eq!(reserved.as_size(), measured);
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
