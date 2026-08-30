use super::detail_series_view::{
    render_series_inline_detail, series_meta_line, SeriesInlineDetailCtx, SERIES_IMAGE_COLS,
    SERIES_IMAGE_ROWS,
};
use crate::app::render::HomeImagePaint;
use crate::app::tests::make_item;
use ratatui::{backend::TestBackend, layout::Rect, Terminal};

#[test]
fn series_inline_detail_reserves_portrait_image_budget() {
    let item = make_item("Series", "Series");
    let rows = crate::app::render::screens::detail_series::series_inline_detail_rows(
        true, &item, 40, true,
    );
    assert!(rows >= SERIES_IMAGE_ROWS as usize + 1);
    assert!(series_meta_line(&item).is_empty());
}

#[test]
fn loading_series_art_uses_placeholder_and_portrait_budget() {
    let item = make_item("Series", "Series");
    let area = Rect::new(0, 0, 40, 20);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let mut paint = None;
    terminal
        .draw(|frame| {
            paint = render_series_inline_detail(
                SeriesInlineDetailCtx {
                    item: &item,
                    images_enabled: true,
                    image_loading: true,
                },
                frame,
                area,
                false,
                true,
            );
        })
        .unwrap();

    match paint {
        Some(HomeImagePaint::Series {
            area,
            show_placeholder,
            ..
        }) => {
            assert!(show_placeholder);
            assert_eq!(area.width, SERIES_IMAGE_COLS);
            assert_eq!(area.height, SERIES_IMAGE_ROWS);
        }
        _ => panic!("expected loading Series paint request"),
    }
}

#[test]
fn cached_series_art_uses_series_painter_and_portrait_budget() {
    let item = make_item("Series", "Series");
    let area = Rect::new(0, 0, 40, 20);
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    let mut paint = None;
    terminal
        .draw(|frame| {
            paint = render_series_inline_detail(
                SeriesInlineDetailCtx {
                    item: &item,
                    images_enabled: true,
                    image_loading: false,
                },
                frame,
                area,
                false,
                true,
            );
        })
        .unwrap();

    match paint {
        Some(HomeImagePaint::Series {
            area,
            show_placeholder,
            ..
        }) => {
            assert!(!show_placeholder);
            assert_eq!(area.width, SERIES_IMAGE_COLS);
            assert_eq!(area.height, SERIES_IMAGE_ROWS);
        }
        _ => panic!("expected cached Series paint request"),
    }
}
