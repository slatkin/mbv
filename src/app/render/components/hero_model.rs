use mbv_core::api::EmbyItem;
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
    fn body(&self) -> HeroBody;
    fn artwork(&self) -> HeroArtwork<'_>;
}

pub(crate) enum HeroBody {
    Listing(Vec<String>),
    Description(String),
}

pub(crate) enum HeroArtwork<'a> {
    Image(&'a str),
    Placeholder,
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

    fn body(&self) -> HeroBody {
        HeroBody::Description(clean_overview(&self.overview))
    }

    fn artwork(&self) -> HeroArtwork<'_> {
        if self.id.is_empty() {
            HeroArtwork::Placeholder
        } else {
            HeroArtwork::Image(&self.id)
        }
    }
}
