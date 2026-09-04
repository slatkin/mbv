use mbv_core::api::EmbyItem;
use mbv_core::playback_queue::QueueItem;
use ratatui::{style::Style, text::Span};

use crate::app::palette;
use crate::app::render::components::home_video::format_release_date;
use crate::app::ui_util::{clean_overview, fmt_duration_approx, trunc_str};
use mbv_core::api::TICKS_PER_SECOND;

/// Provider-neutral content exposed to the shared hero presentation.
pub(crate) trait Hero {
    fn title(&self) -> &str;
    fn subtitle(&self) -> Option<&str>;
    fn meta_rows(&self, width: u16) -> Vec<Vec<Span<'static>>>;
    fn title_suffix(&self) -> Option<Span<'static>>;
    fn description(&self) -> Option<String>;
    /// The default-aspect artwork request, i.e. `artwork_for(HeroArtworkAspect::Default)`.
    fn artwork(&self) -> HeroArtwork<'_> {
        self.artwork_for(HeroArtworkAspect::Default)
    }
    fn artwork_for(&self, aspect: HeroArtworkAspect) -> HeroArtwork<'_>;
}

/// Requested semantic shape for `Hero::artwork_for`'s resolved image
/// (design.md D-D). `Landscape` asks the adapter to prefer a wide-aspect
/// image via its locally verified per-item-type candidate chain; the layout
/// requesting it owns the aspect ratio, not the provider's field names.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HeroArtworkAspect {
    Default,
    Landscape,
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

impl Hero for mbv_core::playback_queue::FeedEntry {
    fn title(&self) -> &str {
        &self.title
    }
    fn subtitle(&self) -> Option<&str> {
        None
    }
    fn meta_rows(&self, width: u16) -> Vec<Vec<Span<'static>>> {
        let line = crate::app::render::screens::feeds_model::feed_entry_meta_line(self);
        if line.is_empty() {
            Vec::new()
        } else {
            vec![vec![Span::styled(
                trunc_str(&line, width as usize),
                Style::default().fg(palette::TEXT_SECONDARY),
            )]]
        }
    }
    fn title_suffix(&self) -> Option<Span<'static>> {
        None
    }
    fn description(&self) -> Option<String> {
        None
    }
    fn artwork_for(&self, _aspect: HeroArtworkAspect) -> HeroArtwork<'_> {
        HeroArtwork::Placeholder
    }
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
                        &fmt_duration_approx((ticks / TICKS_PER_SECOND as u64) as i64),
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

    fn artwork_for(&self, _aspect: HeroArtworkAspect) -> HeroArtwork<'_> {
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

impl Hero for EmbyItem {
    fn title(&self) -> &str {
        &self.name
    }

    fn subtitle(&self) -> Option<&str> {
        (self.item_type == "Episode" && !self.series_name.is_empty())
            .then_some(self.series_name.as_str())
    }

    fn meta_rows(&self, width: u16) -> Vec<Vec<Span<'static>>> {
        let mut rows = Vec::new();
        if self.item_type == "Series" {
            // Ported from `series_meta_line()` (`detail_series_view.rs`):
            // year range (`production_year`..`end_year`) and uppercased
            // genre, joined with two spaces, skipping empty parts.
            let year_range = match (self.production_year, self.end_year) {
                (s, e) if s > 0 && e > 0 && e != s => format!("{}-{}", s, e),
                (s, _) if s > 0 => format!("{}", s),
                _ => String::new(),
            };
            let genre_upper = self.genre.to_uppercase();
            let line = [year_range.as_str(), genre_upper.as_str()]
                .iter()
                .filter(|s| !s.is_empty())
                .copied()
                .collect::<Vec<_>>()
                .join("  ");
            if !line.is_empty() {
                rows.push(vec![Span::styled(
                    trunc_str(&line, width as usize),
                    Style::default().fg(palette::TEXT_DETAIL_META),
                )]);
            }
        }
        if !self.premiere_date.is_empty() {
            rows.push(vec![Span::styled(
                format_release_date(&self.premiere_date),
                Style::default().fg(palette::TEXT_SECONDARY),
            )]);
        }
        if self.runtime_ticks > 0 {
            rows.push(vec![Span::styled(
                trunc_str(
                    &fmt_duration_approx(self.runtime_ticks / TICKS_PER_SECOND),
                    width as usize,
                ),
                Style::default().fg(palette::STATUS_AVAILABLE),
            )]);
        }
        rows
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

    fn artwork_for(&self, aspect: HeroArtworkAspect) -> HeroArtwork<'_> {
        if self.id.is_empty() {
            return HeroArtwork::Placeholder;
        }
        let image_types = match (aspect, self.item_type.as_str()) {
            (HeroArtworkAspect::Landscape, "Series") => {
                &["Thumb", "Primary", "Backdrop", "Logo"][..]
            }
            _ => &["Primary", "Backdrop", "Logo"][..],
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
            image_types(series.artwork_for(HeroArtworkAspect::Landscape)),
            &["Thumb", "Primary", "Backdrop", "Logo"]
        );
    }

    #[test]
    fn non_series_landscape_skips_thumb() {
        let movie = item("Movie");
        assert_eq!(
            image_types(movie.artwork_for(HeroArtworkAspect::Landscape)),
            &["Primary", "Backdrop", "Logo"]
        );
    }

    #[test]
    fn default_aspect_is_unchanged_for_every_item_type() {
        for item_type in ["Series", "Movie", "Episode", "Audio"] {
            let it = item(item_type);
            assert_eq!(image_types(it.artwork()), &["Primary", "Backdrop", "Logo"]);
            assert_eq!(
                image_types(it.artwork_for(HeroArtworkAspect::Default)),
                &["Primary", "Backdrop", "Logo"]
            );
        }
    }
}
