use super::home_hero::{HeroMetaBlock, HomeImagePaint, KeepWatchingHeroLayout};
use super::home_video::format_release_date;
use crate::app::ui_util::*;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

use crate::app::render::RENDER_FILTER;

impl App {
    /// Image types to request for the Keep Watching hero panel, mirroring
    /// the per-type conventions used for the queue card (`render_card`).
    pub(in crate::app::render) fn keep_watching_hero_image_types(
        item: &mbv_core::api::EmbyItem,
    ) -> &'static [&'static str] {
        match item.item_type.as_str() {
            "Movie" => &["Backdrop", "Primary", "Logo"],
            _ => &["Primary", "Backdrop"],
        }
    }

    /// Builds the Keep Watching hero panel's metadata layout for `item`,
    /// delegating to the shared [`hero_text_layout`] (also used by the
    /// generic Audiobookshelf hero, so both inline items share one
    /// wrap-around-the-image implementation).
    pub(in crate::app::render) fn keep_watching_hero_layout(
        item: &mbv_core::api::EmbyItem,
        text_w: usize,
        wide_w: usize,
        image_rows: u16,
        overview_pad: usize,
    ) -> KeepWatchingHeroLayout {
        let show_name = if item.item_type == "Episode" {
            item.series_name.clone()
        } else {
            String::new()
        };
        let overview = if item.overview.is_empty() {
            String::new()
        } else {
            clean_overview(&item.overview)
        };
        super::home_hero::hero_text_layout(
            &item.name,
            &show_name,
            &overview,
            text_w,
            wide_w,
            image_rows,
            overview_pad,
            2, // release-date row + duration row
        )
    }

    /// Renders the Keep Watching hero panel's image column into `area`,
    /// top-aligned and, in wide two-column layouts, horizontally centered. The column is a fixed reserved
    /// box (unlike the queue card's growing/shrinking slot), so a dim
    /// placeholder simply fills it while no artwork is ready yet. Shared by
    /// the Emby and generic Audiobookshelf heroes.
    pub(in crate::app::render) fn render_keep_watching_hero_image(
        &mut self,
        f: &mut Frame,
        area: Rect,
        cache_key: &str,
        centered: bool,
    ) {
        if area.width == 0 || area.height == 0 {
            return;
        }
        let img_area = area;
        // `img_area`'s height is sometimes stretched to match the metadata
        // column beside it (e.g. a long overview in narrow layout, home.rs's
        // `hero_height = image_rows.max(meta_layout.height)`), so it can be
        // taller than the image's own 16:9 row budget -- the text layout
        // already wrapped its overview around that budget (`image_rows` in
        // `hero_text_layout`), not around the stretched panel height. A 16:9
        // backdrop naturally renders within the budget regardless, but a
        // squarer cover (Audiobookshelf/podcast art) would otherwise grow
        // into the stretched extra space and overlap the "past the image"
        // overview text that assumed it wouldn't. Cap `avail` to the same
        // budget the placeholder below already caps to, so the real image
        // never renders past where the text thinks it ends.
        let natural_h = (img_area.width.saturating_mul(9).saturating_add(31) / 32)
            .max(1)
            .min(img_area.height);
        if let Some(state) = self.cached_image_protocol_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = Size {
                width: img_area.width,
                height: natural_h,
            };
            if let Some(actual) =
                state.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
                let img_rect = Rect {
                    x: if centered {
                        img_area.x + img_area.width.saturating_sub(actual.width) / 2
                    } else {
                        img_area.x + img_area.width.saturating_sub(actual.width)
                    },
                    y: img_area.y,
                    width: actual.width,
                    height: actual.height,
                };
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    img_rect,
                    state,
                );
                return;
            }
        }
        // Same budget as the real-image branch above, so the placeholder
        // never renders as a too-tall block while no artwork is ready yet.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
            Rect {
                height: natural_h,
                ..img_area
            },
        );
    }

    /// Builds the Keep Watching hero's meta content: the watch-state glyph to
    /// render one space after the title, plus the metadata rows (release
    /// date, then the duration on its own row in green). Emby-specific input
    /// to `render_beside_image_hero`'s shared `meta_block` parameter.
    pub(in crate::app::render) fn keep_watching_hero_meta_block(
        item: &mbv_core::api::EmbyItem,
        width: u16,
        use_nerd_fonts: bool,
    ) -> HeroMetaBlock {
        let release_date = if item.premiere_date.is_empty() {
            String::new()
        } else {
            format_release_date(&item.premiere_date)
        };
        let dur_str = if item.runtime_ticks > 0 {
            fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
        } else {
            String::new()
        };
        // Watch-state glyph: watched, in-progress, or unwatched. Icons are
        // Nerd Font codepoints (watched e001, in-progress e004, unwatched
        // e002); without Nerd Fonts, fall back to Unicode symbols that render
        // in ordinary terminal fonts.
        let (glyph, color) = if item.played {
            (
                if use_nerd_fonts { "\u{e001}" } else { "●" },
                palette::ACCENT,
            )
        } else if item.playback_position_ticks > 0 {
            (
                if use_nerd_fonts { "\u{e004}" } else { "◐" },
                palette::TEXT_FOCUS_ACCENT,
            )
        } else {
            (
                if use_nerd_fonts { "\u{e002}" } else { "○" },
                palette::STATUS_ERROR,
            )
        };
        let title_suffix = Some(Span::styled(glyph, Style::default().fg(color)));

        let mut meta_rows: Vec<Vec<Span<'static>>> = Vec::new();
        if !release_date.is_empty() {
            meta_rows.push(vec![Span::styled(
                release_date,
                Style::default().fg(palette::TEXT_SECONDARY),
            )]);
        }
        if !dur_str.is_empty() {
            meta_rows.push(vec![Span::styled(
                trunc_str(&dur_str, width as usize),
                Style::default().fg(palette::STATUS_AVAILABLE),
            )]);
        }
        HeroMetaBlock {
            title_suffix,
            meta_rows,
        }
    }

    /// Paints cached Series artwork using its portrait inline-detail budget.
    fn paint_series_image(&mut self, f: &mut Frame, area: Rect, cache_key: &str) {
        if let Some(image) = self.cached_image_protocol_mut(cache_key) {
            type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
            let avail = Size {
                width: area.width,
                height: area.height,
            };
            if let Some(actual) =
                image.size_for(ratatui_image::Resize::Scale(Some(RENDER_FILTER)), avail)
            {
                f.render_stateful_widget(
                    SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                    Rect {
                        width: actual.width,
                        height: actual.height,
                        ..area
                    },
                    image,
                );
            }
        }
    }

    fn paint_audiobookshelf_cover(
        &mut self,
        f: &mut Frame,
        area: Rect,
        cache_key: &str,
        show_placeholder: bool,
        centered: bool,
    ) {
        if show_placeholder {
            self.render_keep_watching_hero_image(f, area, cache_key, centered);
        } else {
            self.render_keep_watching_hero_image(f, area, cache_key, centered);
        }
    }

    /// Fetches (if needed) and paints the image a [`HomeImagePaint`] request
    /// describes, using App's image-cache authority. Shared by the
    /// `App::render_home_list` wrapper (`home.rs`), which computes its own
    /// `HomeImagePaint` via the shared `render_home_content` orchestration.
    pub(in crate::app) fn paint_home_image(
        &mut self,
        f: &mut Frame,
        image_paint: Option<HomeImagePaint>,
    ) {
        match image_paint {
            Some(HomeImagePaint::Emby {
                area,
                item,
                centered,
            }) => {
                let cache_key = format!("{}:pwr_kw", item.id);
                if self.images_enabled() {
                    let img_types = Self::keep_watching_hero_image_types(&item);
                    self.fetch_card_image(
                        cache_key.clone(),
                        item.id.clone(),
                        item.series_id.clone(),
                        img_types,
                    );
                }
                self.render_keep_watching_hero_image(f, area, &cache_key, centered);
            }
            Some(HomeImagePaint::Series {
                area,
                item,
                show_placeholder,
                image_types,
            }) => {
                let cache_key = format!("{}:ser:{}", item.id, image_types.join(","));
                if self.images_enabled() {
                    self.fetch_card_image(
                        cache_key.clone(),
                        item.id.clone(),
                        String::new(),
                        image_types,
                    );
                }
                if show_placeholder {
                    // Series artwork uses the portrait inline-detail budget,
                    // not the generic 16:9 hero budget.
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                        area,
                    );
                } else {
                    self.paint_series_image(f, area, &cache_key);
                }
            }
            Some(HomeImagePaint::CompactBanner {
                area,
                item,
                show_placeholder,
            }) => {
                if show_placeholder {
                    f.render_widget(
                        Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                        area,
                    );
                } else {
                    let cache_key = super::detail::compact_banner_image_cache_key(&item.id);
                    if let Some(state) = self.cached_image_protocol_mut(&cache_key) {
                        type SImg =
                            ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                        f.render_stateful_widget(
                            SImg::default()
                                .resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                            area,
                            state,
                        );
                    }
                }
            }
            Some(HomeImagePaint::AudiobookshelfCover {
                area,
                library_item_id,
                show_placeholder,
            }) => {
                if let Some(cache_key) = self.audiobookshelf_cover_key(&library_item_id) {
                    self.paint_audiobookshelf_cover(f, area, &cache_key, show_placeholder, true);
                }
            }
            Some(HomeImagePaint::AudiobookshelfBookCover {
                area,
                library_item_id,
                show_placeholder,
            }) => {
                if let Some(cache_key) = self.audiobookshelf_book_cover_key(&library_item_id) {
                    self.paint_audiobookshelf_cover(f, area, &cache_key, show_placeholder, false);
                }
            }
            None => {}
        }
    }
}
