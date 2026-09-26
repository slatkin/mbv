use mbv_core::api::EmbyItem;
use mbv_core::api::TICKS_PER_SECOND;

use crate::app::render::components::home_video::format_release_date;
use crate::app::ui_util::fmt_duration_hms;

/// Canonical TV Wide Series image-type candidate chain. The panel's artwork
/// policy reuses this chain for Series landscape arms.
pub(in crate::app) const SERIES_LANDSCAPE_IMAGE_TYPES: &[&str] =
    &["Thumb", "Primary", "Backdrop", "Logo"];

/// Plain-text metadata rows for the shared hero header, plus the index of
/// the duration row (for the painter's `DURATION` colour).
pub(in crate::app) fn emby_hero_meta_rows_plain(item: &EmbyItem) -> (Vec<String>, Option<usize>) {
    let mut rows = Vec::new();
    let mut duration_row = None;
    if item.item_type == "Series" {
        let year_range = match (item.production_year, item.end_year) {
            (s, e) if s > 0 && e > 0 && e != s => format!("{s}-{e}"),
            (s, _) if s > 0 => format!("{s}"),
            _ => String::new(),
        };
        let genre_upper = item
            .genres
            .first()
            .map(|genre| genre.to_uppercase())
            .unwrap_or_default();
        let line = [year_range.as_str(), genre_upper.as_str()]
            .iter()
            .filter(|s| !s.is_empty())
            .copied()
            .collect::<Vec<_>>()
            .join("  ");
        if !line.is_empty() {
            rows.push(line);
        }
    }
    if !item.premiere_date.is_empty() {
        rows.push(format_release_date(&item.premiere_date));
    }
    if item.runtime_ticks > 0 {
        duration_row = Some(rows.len());
        rows.push(fmt_duration_hms(item.runtime_ticks / TICKS_PER_SECOND));
    }
    (rows, duration_row)
}
