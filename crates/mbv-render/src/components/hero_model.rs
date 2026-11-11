use mbv_emby_model::EmbyItem;
use mbv_emby_model::TICKS_PER_SECOND;
use mbv_ui_model::meta::{MetaKind, MetaSegment, join_segments};

use crate::components::home_video::format_release_date;
use mbv_ui_model::ui_util::fmt_duration_hms;

/// Canonical TV Wide Series image-type candidate chain. The panel's artwork
/// policy reuses this chain for Series landscape arms.
pub const SERIES_LANDSCAPE_IMAGE_TYPES: &[&str] = &["Thumb", "Primary", "Backdrop", "Logo"];

/// Typed meta rows for the shared hero header, plus the index of the duration
/// row (for the painter's `DURATION` colour). Each row is a list of typed
/// segments; the TUI joins a row's text, the GUI paints each segment's kind.
#[must_use]
pub fn emby_hero_meta_rows_plain(item: &EmbyItem) -> (Vec<Vec<MetaSegment>>, Option<usize>) {
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
        let line = join_segments(
            [
                MetaSegment::new(year_range, MetaKind::Date),
                MetaSegment::plain(genre_upper),
            ],
            "  ",
        );
        if !line.is_empty() {
            rows.push(line);
        }
    }
    if !item.premiere_date.is_empty() {
        rows.push(vec![MetaSegment::new(
            format_release_date(&item.premiere_date),
            MetaKind::Date,
        )]);
    }
    if item.runtime_ticks > 0 {
        duration_row = Some(rows.len());
        rows.push(vec![MetaSegment::new(
            fmt_duration_hms(item.runtime_ticks / TICKS_PER_SECOND),
            MetaKind::Time,
        )]);
    }
    (rows, duration_row)
}
