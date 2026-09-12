use super::home_hero::HomeImagePaint;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

use crate::app::render::RENDER_FILTER;

impl App {
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
            Block::default().style(Style::default().bg(
                palette::surface_colors(palette::Surface::ArtworkLoadingPlaceholder, false).fill,
            )),
            Rect {
                height: natural_h,
                ..img_area
            },
        );
    }

    fn paint_audiobookshelf_cover(
        &mut self,
        f: &mut Frame,
        area: Rect,
        cache_key: &str,
        show_placeholder: bool,
        centered: bool,
    ) {
        if show_placeholder || self.cached_image_protocol_mut(cache_key).is_some() {
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
