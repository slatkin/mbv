use super::*;
use crate::app::render::HomeImagePaint;
use crate::app::tests::{make_app_stub, make_item};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::{Duration, Instant};
// Characterization coverage stays beside the moved detail component.

#[test]
fn content_rows_is_never_shorter_than_the_rendered_image_height() {
    let short_text_layout = CompactBannerLayout {
        meta_line: None,
        show_playing: false,
        lines: vec!["A short overview.".to_string()],
        director_line_idx: None,
        img_actual_w: 18,
        img_height: 12,
        img_is_placeholder: false,
    };
    assert_eq!(
        short_text_layout.content_rows(),
        13,
        "banner must reserve the image's height and bottom gutter even when the \
         wrapped text alone would need far fewer rows"
    );
    assert_eq!(short_text_layout.content_rows_with_title(1), 13);

    let tall_text_layout = CompactBannerLayout {
        meta_line: Some("Crime  1974  1h33m".to_string()),
        show_playing: false,
        lines: vec!["line".to_string(); 20],
        director_line_idx: None,
        img_actual_w: 18,
        img_height: 12,
        img_is_placeholder: false,
    };
    assert_eq!(
        tall_text_layout.content_rows(),
        21,
        "when the text is taller than the image, the image must not \
         clip the banner back down to its own height"
    );

    let no_image_layout = CompactBannerLayout {
        meta_line: None,
        show_playing: false,
        lines: vec!["A short overview.".to_string()],
        director_line_idx: None,
        img_actual_w: 0,
        img_height: 0,
        img_is_placeholder: false,
    };
    assert_eq!(
        no_image_layout.content_rows(),
        1,
        "with no image (e.g. images disabled), sizing stays text-only"
    );

    let empty_layout = CompactBannerLayout {
        lines: Vec::new(),
        ..no_image_layout
    };
    assert_eq!(empty_layout.content_rows(), 0);
}

#[test]
fn compact_detail_pure_fn_returns_image_paint_without_app() {
    let item = make_item("movie-1", "Movie");
    let mut term = Terminal::new(TestBackend::new(60, 20)).unwrap();

    for placeholder in [true, false] {
        let layout = CompactBannerLayout {
            meta_line: None,
            show_playing: false,
            lines: vec!["A short overview.".to_string()],
            director_line_idx: None,
            img_actual_w: 18,
            img_height: 12,
            img_is_placeholder: placeholder,
        };
        let mut paint = None;
        term.draw(|f| {
            let area = f.area();
            paint = render_compact_detail_with_ctx(
                CompactDetailCtx {
                    item: &item,
                    layout,
                },
                f,
                area,
                true,
                true,
            );
        })
        .unwrap();

        match paint {
            Some(HomeImagePaint::CompactBanner {
                show_placeholder, ..
            }) => assert_eq!(
                show_placeholder, placeholder,
                "placeholder flag must pass through from the layout"
            ),
            _ => panic!("expected a CompactBanner image paint"),
        }
    }
}

#[test]
fn compact_banner_wrapper_fetches_while_nav_gate_only_controls_display() {
    let mut app = make_app_stub();
    app.image_protocol_enabled = true;
    app.last_library_nav_at = Instant::now();
    let item = make_item("movie", "Movie");
    let _ = app.compact_banner_layout_with_overview(&item, 60, false);
    let key = compact_banner_image_cache_key(&item.id);
    assert!(app.card_image_loading.contains(&key) || app.card_image_states.contains_key(&key));

    app.last_library_nav_at =
        Instant::now() - crate::app::images::NAV_IMAGE_FETCH_IDLE_DELAY - Duration::from_millis(1);
    let gated = app.compact_banner_layout_with_overview(&item, 60, false);
    assert!(!gated.img_is_placeholder || gated.img_height > 0);
}
