use super::detail_series_view::{series_meta_line, SERIES_IMAGE_ROWS};
use crate::app::tests::make_item;

#[test]
fn series_inline_detail_reserves_portrait_image_budget() {
    let item = make_item("Series", "Series");
    let rows = crate::app::render::screens::detail_series::series_inline_detail_rows(
        true, &item, 40, true,
    );
    assert!(rows >= SERIES_IMAGE_ROWS as usize + 1);
    assert!(series_meta_line(&item).is_empty());
}
