use mbv_emby_model::EmbyItem;
use mbv_emby_model::TICKS_PER_SECOND;

use crate::components::home_video::format_release_date;
use mbv_ui_model::ui_util::fmt_duration_hms;

/// Canonical TV Wide Series image-type candidate chain. The panel's artwork
/// policy reuses this chain for Series landscape arms.
pub const SERIES_LANDSCAPE_IMAGE_TYPES: &[&str] = &["Thumb", "Primary", "Backdrop", "Logo"];

/// Plain-text metadata rows for the shared hero header, plus the indexes of
/// the air-date row and the duration row (for the painter's `DURATION`
/// colour and row-level overlays such as the TV workspace selection).
#[must_use]
pub fn emby_hero_meta_rows_plain(item: &EmbyItem) -> (Vec<String>, Option<usize>, Option<usize>) {
    let mut rows = Vec::new();
    let mut date_row = None;
    let mut duration_row = None;
    if item.item_type == "Series" {
        let genre_upper = item
            .genres
            .first()
            .map(|genre| genre.to_uppercase())
            .unwrap_or_default();
        if !genre_upper.is_empty() {
            rows.push(genre_upper);
        }
    }
    if !item.premiere_date.is_empty() {
        date_row = Some(rows.len());
        rows.push(format_release_date(&item.premiere_date));
    }
    if item.runtime_ticks > 0 {
        duration_row = Some(rows.len());
        rows.push(fmt_duration_hms(item.runtime_ticks / TICKS_PER_SECOND));
    }
    (rows, date_row, duration_row)
}
