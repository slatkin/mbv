use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use ratatui::{style::Style, text::Span};

use crate::app::palette;
use crate::app::render::components::home_video::format_release_date;
use crate::app::ui_util::{clean_overview, fmt_duration_approx, fmt_duration_short, trunc_str};
use mbv_core::api::TICKS_PER_SECOND;

/// Provider-neutral content exposed to the shared hero presentation.
pub(crate) trait Hero {
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;
    fn meta_rows(&self, width: u16) -> Vec<Vec<Span<'static>>>;
    fn title_suffix(&self) -> Option<Span<'static>>;
    fn description(&self) -> Option<String>;
    /// The default-aspect artwork request: the provider's primary image chain.
    fn artwork(&self) -> HeroArtwork<'_>;
    /// The landscape-aspect artwork request: the provider's wide-image chain
    /// (Series overrides it with the `Thumb`-first chain). Task 5.4 deleted
    /// the per-aspect enum; this method keeps the landscape-chain role with
    /// the trait until the panel's artwork policy replaces this path when the
    /// destination converts (task 8.2).
    fn landscape_artwork(&self) -> HeroArtwork<'_> {
        self.artwork()
    }
}

pub(crate) enum HeroArtwork<'a> {
    Image {
        item_id: &'a str,
        /// Ordered Emby image-type candidate chain, same shape and
        /// precedent as `card.rs::card_image_types`.
        image_types: &'static [&'static str],
    },
    Placeholder,
}

impl Hero for QueueItem {
    fn title(&self) -> &str {
        self.title()
    }

    fn subtitle(&self) -> Option<&str> {
        match self {
            QueueItem::Audiobookshelf(item) => item.show_title.as_deref(),
            QueueItem::AudiobookshelfBook(item) => item.author.as_deref(),
            _ => None,
        }
    }

    fn meta_rows(&self, width: u16) -> Vec<Vec<Span<'static>>> {
        self.duration()
            .map(|ticks| {
                vec![vec![Span::styled(
                    trunc_str(
                        &fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64),
                        width as usize,
                    ),
                    Style::default().fg(palette::TEXT_SECONDARY),
                )]]
            })
            .unwrap_or_default()
    }

    fn title_suffix(&self) -> Option<Span<'static>> {
        None
    }

    fn description(&self) -> Option<String> {
        match self {
            QueueItem::Audiobookshelf(item) => item.description.clone(),
            _ => None,
        }
    }

    fn artwork(&self) -> HeroArtwork<'_> {
        match self {
            QueueItem::Audiobookshelf(item) => item
                .cover_path
                .as_deref()
                .map(|id| HeroArtwork::Image {
                    item_id: id,
                    image_types: &["Primary"],
                })
                .unwrap_or(HeroArtwork::Placeholder),
            QueueItem::AudiobookshelfBook(item) => item
                .cover_path
                .as_deref()
                .map(|id| HeroArtwork::Image {
                    item_id: id,
                    image_types: &["Primary"],
                })
                .unwrap_or(HeroArtwork::Placeholder),
            _ => HeroArtwork::Placeholder,
        }
    }
}

/// Canonical TV Wide Series image-type candidate chain: the `Thumb`-first
/// landscape chain `Hero::landscape_artwork` declares (task 5.4 deleted the
/// per-aspect enum). The shell prefetch requests this same item (not a copy)
/// so it warms the identical key the painter fetches under
/// `series_image_cache_key`; the panel's artwork policy reuses it for Series
/// landscape arms.
pub(in crate::app) const SERIES_LANDSCAPE_IMAGE_TYPES: &[&str] =
    &["Thumb", "Primary", "Backdrop", "Logo"];

/// The plain-text metadata rows (task 5.4, design D5): one entry per row, no
/// width — the panel's header painter owns truncation and wrapping. Shared by
/// the `Hero` impl below (which truncates and styles for the un-migrated
/// painters) and the panel's Emby hero producer.
pub(crate) fn emby_hero_meta_rows_plain(item: &EmbyItem) -> Vec<String> {
    let mut rows = Vec::new();
    if item.item_type == "Series" {
        // Ported from `series_meta_line()` (`detail_series_view.rs`):
        // year range (`production_year`..`end_year`) and uppercased
        // genre, joined with two spaces, skipping empty parts.
        let year_range = match (item.production_year, item.end_year) {
            (s, e) if s > 0 && e > 0 && e != s => format!("{}-{}", s, e),
            (s, _) if s > 0 => format!("{}", s),
            _ => String::new(),
        };
        let genre_upper = item.genre.to_uppercase();
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
        rows.push(fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND));
    }
    rows
}

/// The legacy `Hero` painters' historical per-row colours by MEANING, in the
/// row order [`emby_hero_meta_rows_plain`] emits: the Series year-range/
/// genre line, then the release date, then the duration. Unlike the panel's
/// `HeroHeader` painter (`hero_header.rs::paint_title_and_meta`), which
/// cycles `HERO_META_ROLES[n % 3]` positionally, this path styles by row
/// meaning — Movies/Episodes have no series row, so positional cycling would
/// shift the date/duration colours. Two tables styling the same rows is a
/// recorded judgement call until 9.1 converges them onto one painter.
fn emby_meta_row_styles(item: &EmbyItem) -> Vec<ratatui::style::Color> {
    let mut styles = Vec::new();
    if item.item_type == "Series" {
        styles.push(palette::TEXT_DETAIL_META);
    }
    if !item.premiere_date.is_empty() {
        styles.push(palette::TEXT_SECONDARY);
    }
    if item.runtime_ticks > 0 {
        styles.push(palette::STATUS_AVAILABLE);
    }
    styles
}

impl Hero for EmbyItem {
    fn title(&self) -> &str {
        &self.name
    }

    fn subtitle(&self) -> Option<&str> {
        (self.item_type == "Episode" && !self.series_name.is_empty())
            .then_some(self.series_name.as_str())
    }

    fn meta_rows(&self, width: u16) -> Vec<Vec<Span<'static>>> {
        // Rows keep the painters' historical per-row colours by meaning
        // (see `emby_meta_row_styles`).
        let styles = emby_meta_row_styles(self);
        emby_hero_meta_rows_plain(self)
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                vec![Span::styled(
                    trunc_str(&row, width as usize),
                    Style::default().fg(styles
                        .get(index)
                        .copied()
                        .unwrap_or(palette::TEXT_SECONDARY)),
                )]
            })
            .collect()
    }

    fn title_suffix(&self) -> Option<Span<'static>> {
        let glyph = if self.played {
            "●"
        } else if self.playback_position_ticks > 0 {
            "◐"
        } else {
            "○"
        };
        let color = if self.played {
            palette::ACCENT
        } else if self.playback_position_ticks > 0 {
            palette::TEXT_FOCUS_ACCENT
        } else {
            palette::STATUS_ERROR
        };
        Some(Span::styled(glyph, Style::default().fg(color)))
    }

    fn description(&self) -> Option<String> {
        let d = clean_overview(&self.overview);
        (!d.is_empty()).then_some(d)
    }

    fn artwork(&self) -> HeroArtwork<'_> {
        if self.id.is_empty() {
            return HeroArtwork::Placeholder;
        }
        HeroArtwork::Image {
            item_id: &self.id,
            image_types: &["Primary", "Backdrop", "Logo"],
        }
    }

    fn landscape_artwork(&self) -> HeroArtwork<'_> {
        if self.id.is_empty() {
            return HeroArtwork::Placeholder;
        }
        let image_types = if self.item_type == "Series" {
            SERIES_LANDSCAPE_IMAGE_TYPES
        } else {
            &["Primary", "Backdrop", "Logo"]
        };
        HeroArtwork::Image {
            item_id: &self.id,
            image_types,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::make_item;

    fn item(item_type: &str) -> EmbyItem {
        make_item("Test", item_type)
    }

    fn image_types(artwork: HeroArtwork<'_>) -> &'static [&'static str] {
        match artwork {
            HeroArtwork::Image { image_types, .. } => image_types,
            HeroArtwork::Placeholder => panic!("expected HeroArtwork::Image"),
        }
    }

    #[test]
    fn series_landscape_prefers_thumb() {
        let series = item("Series");
        assert_eq!(
            image_types(series.landscape_artwork()),
            &["Thumb", "Primary", "Backdrop", "Logo"]
        );
    }

    #[test]
    fn non_series_landscape_skips_thumb() {
        let movie = item("Movie");
        assert_eq!(
            image_types(movie.landscape_artwork()),
            &["Primary", "Backdrop", "Logo"]
        );
    }

    #[test]
    fn default_chain_is_unchanged_for_every_item_type() {
        for item_type in ["Series", "Movie", "Episode", "Audio"] {
            let it = item(item_type);
            assert_eq!(image_types(it.artwork()), &["Primary", "Backdrop", "Logo"]);
        }
    }
}
