use crate::app::render::components::hero::{
    inline_hero_text_width, HeroContent, HeroImage, HeroLine,
};
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::render::HomeImagePaint;
use crate::app::render::RENDER_FILTER;
use crate::app::ui_util::*;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::Frame;
use textwrap::wrap;

const IMG_COLS: u16 = 24;
const IMG_ROWS: u16 = 14;

/// Cache key for the compact movie banner's poster image, under which
/// `fetch_card_image`/`fetch_list_card_image_when_idle` store and look up the
/// resized/encoded image state. Shared by the eager fetch in
/// `compact_banner_layout` and the prefetch loop in `list.rs`'s
/// `render_list` (#287) so the two can never format the key
/// differently and silently miss each other's cache entries.
pub(in crate::app::render) fn compact_banner_image_cache_key(item_id: &str) -> String {
    format!("{item_id}:cmp_primary")
}

/// Estimated placeholder size for a poster that hasn't been fetched/decoded
/// yet. Emby/TMDb primary movie art is overwhelmingly a 2:3 (width:height)
/// aspect ratio, so fitting that ratio into the same `IMG_COLS x IMG_ROWS`
/// pixel bounding box a real image would be fit into -- via the exact same
/// `Resize::size_for` math `ThreadProtocol`/`StatefulProtocol` use for a real
/// decoded image -- gives a reserved width that matches what a real poster
/// resolves to almost exactly, instead of reserving the full bounding-box
/// width and causing a second, smaller reflow once the real image swaps in.
/// Only the ratio of the dummy image matters here, not its absolute size, so
/// it's kept tiny (2x3 px) to make the allocation this runs once per render
/// frame effectively free.
fn poster_placeholder_size(font_size: ratatui_image::FontSize, img_cols: u16) -> (u16, u16) {
    let canonical_poster_aspect = image::DynamicImage::new_rgb8(2, 3);
    let size = ratatui_image::Resize::Scale(Some(RENDER_FILTER)).size_for(
        &canonical_poster_aspect,
        font_size,
        ratatui::layout::Size {
            width: img_cols,
            height: IMG_ROWS,
        },
    );
    (size.width, size.height)
}

/// Everything content-dependent about the compact movie-detail banner: the
/// meta line, the "Playing" indicator, and the overview + director text
/// wrapped to the banner's actual panel width. Computed once by
/// `App::compact_banner_layout_with_overview` and consumed by
/// `render_compact_detail_with_ctx` to actually render the banner, so the
/// row-count estimate and the render never duplicate the wrapping logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::app) struct CompactBannerLayout {
    meta_line: Option<String>,
    show_playing: bool,
    /// Wrapped overview lines, plus (if there's a director) a blank
    /// separator line and a placeholder line at `director_line_idx` that
    /// renders as "Director: <name>" instead of plain text.
    lines: Vec<String>,
    director_line_idx: Option<usize>,
    img_actual_w: u16,
    img_height: u16,
    /// True when `img_actual_w`/`img_height` describe a reserved-but-not-yet-
    /// loaded box (fetch in flight, or resize+encode still running on the
    /// worker thread) rather than a real decoded image. The render pass uses
    /// this to draw a dim placeholder block instead of `StatefulImage`.
    img_is_placeholder: bool,
}

impl CompactBannerLayout {
    /// Total rows the banner's content needs: meta line + "Playing" line (if
    /// present) + every wrapped overview/director line, but never fewer than
    /// the poster image's rendered height. Text-only sizing regressed the
    /// banner below the image's height whenever the overview was short
    /// (e.g. a couple of wrapped lines) -- the image rendered at its fixed
    /// height regardless of how few text rows were reserved, so it spilled
    /// past the banner's row budget into the list rows below it. No upper
    /// cap is applied to the text side -- real Emby movie metadata is short
    /// by convention (#263), so unbounded growth there is intended.
    #[cfg(test)]
    pub(in crate::app) fn content_rows(&self) -> usize {
        self.content_rows_with_title(0)
    }

    pub(in crate::app) fn content_rows_with_title(&self, title_rows: u16) -> usize {
        let text_rows = title_rows as usize
            + self.meta_line.is_some() as usize
            + self.show_playing as usize
            + self.lines.len();
        let image_rows = if self.img_height > 0 {
            self.img_height.saturating_add(1) as usize
        } else {
            0
        };
        text_rows.max(image_rows)
    }
}

impl App {
    pub(crate) fn selected_movie_item(
        &self,
        lib_idx: usize,
        cursor: usize,
    ) -> Option<mbv_core::api::EmbyItem> {
        let ctx = self.library_list_render_ctx(lib_idx, false, cursor, 0);
        self.selected_movie_item_with_ctx(lib_idx, &ctx)
    }

    fn selected_movie_item_with_ctx(
        &self,
        lib_idx: usize,
        ctx: &LibraryListRenderCtx,
    ) -> Option<mbv_core::api::EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        let coll = lib.library.collection_type.as_str();
        if coll != "movies" && coll != "homevideos" && coll != "podcasts" {
            return None;
        }

        let item = if self.is_feed_home_video_group_view(lib_idx) {
            self.selected_feed_home_video_item(lib_idx)?
        } else {
            ctx.items.get(ctx.cursor)?.clone()
        };

        if item.is_folder {
            return None;
        }
        if coll == "movies" && item.item_type != "Movie" {
            return None;
        }

        Some(item)
    }

    pub(crate) fn selected_series_item(
        &self,
        lib_idx: usize,
        cursor: usize,
    ) -> Option<mbv_core::api::EmbyItem> {
        let ctx = self.library_list_render_ctx(lib_idx, false, cursor, 0);
        self.selected_series_item_with_ctx(lib_idx, &ctx)
    }

    fn selected_series_item_with_ctx(
        &self,
        lib_idx: usize,
        ctx: &LibraryListRenderCtx,
    ) -> Option<mbv_core::api::EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if lib.library.collection_type != "tvshows" {
            return None;
        }

        let item = ctx.items.get(ctx.cursor)?.clone();

        if item.item_type != "Series" {
            return None;
        }

        Some(item)
    }

    /// Computes the compact banner's content for `item`, given the panel
    /// width it will render into (i.e. the eventual `area.width` passed to
    /// `render_compact_detail_with_ctx`). Pure function of `item` + width aside
    /// from the image-state cache lookup/fetch-trigger, so calling it twice
    /// per frame (once to measure, once to render) is safe and idempotent.
    pub(in crate::app::render) fn compact_banner_layout_with_overview(
        &mut self,
        item: &mbv_core::api::EmbyItem,
        panel_width: u16,
        truncate_overview: bool,
    ) -> CompactBannerLayout {
        let key = compact_banner_image_cache_key(&item.id);
        let images_enabled = self.images_enabled();
        if images_enabled {
            self.fetch_card_image(
                key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }
        let inner_w = panel_width as usize;
        let img_cols = IMG_COLS.min((inner_w / 2) as u16);
        let placeholder_size = self
            .image_picker
            .as_ref()
            .map(|picker| poster_placeholder_size(picker.font_size(), img_cols))
            .unwrap_or((img_cols, IMG_ROWS));
        let has_no_art = self
            .card_image_states
            .get(&key)
            .is_some_and(|e| e.img.is_none());
        let cached_size = if self.right_panel_image_renders_allowed() {
            self.cached_image_protocol_mut(&key).and_then(|state| {
                state
                    .size_for(
                        ratatui_image::Resize::Scale(Some(RENDER_FILTER)),
                        ratatui::layout::Size {
                            width: img_cols,
                            height: IMG_ROWS,
                        },
                    )
                    .map(|s| (s.width, s.height))
            })
        } else {
            None
        };
        let playback = self.effective_playback_state();
        let show_playing = playback.active
            && self
                .playback_queue()
                .emby_item_at(playback.active_idx)
                .is_some_and(|i| i.id == item.id);
        compact_banner_layout(
            item,
            panel_width,
            truncate_overview,
            images_enabled,
            self.right_panel_image_renders_allowed(),
            has_no_art,
            cached_size,
            placeholder_size,
            show_playing,
        )
    }
}

pub(in crate::app) fn compact_banner_layout(
    item: &mbv_core::api::EmbyItem,
    panel_width: u16,
    truncate_overview: bool,
    images_enabled: bool,
    nav_gate_open: bool,
    has_no_art: bool,
    cached_size: Option<(u16, u16)>,
    placeholder_size: (u16, u16),
    show_playing: bool,
) -> CompactBannerLayout {
    let inner_w = panel_width as usize;

    let (placeholder_w, placeholder_h) = placeholder_size;
    let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) =
        if !images_enabled || has_no_art {
            (0, 0, false)
        } else if nav_gate_open {
            match cached_size {
                Some((width, height)) => (width, height, false),
                None => (placeholder_w, placeholder_h, true),
            }
        } else {
            (placeholder_w, placeholder_h, true)
        };

    let narrow_w = inline_hero_text_width(inner_w as u16, img_actual_w, img_height, 0) as usize;

    let mut rows_before_overview = 0usize;

    let dur_str = if item.runtime_ticks > 0 {
        fmt_duration_approx(item.runtime_ticks / TICKS_PER_SECOND)
    } else {
        String::new()
    };
    let year_str = if item.production_year > 0 {
        item.production_year.to_string()
    } else {
        String::new()
    };
    let meta = [item.genre.as_str(), year_str.as_str(), dur_str.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("  ");
    let meta_line = if meta.is_empty() {
        None
    } else {
        rows_before_overview += 1;
        Some(meta)
    };

    if show_playing {
        rows_before_overview += 1;
    }

    // Rows before the overview block sit above the poster image's
    // bottom edge too (as long as there aren't more of them than the
    // image is tall), so they narrow the wrap width the same way
    // overview/director lines do; `shadow_lines` counts how many of the
    // *upcoming* overview/director lines still fall within the image's
    // row span.
    let shadow_lines = (img_height.saturating_add(1) as usize).saturating_sub(rows_before_overview);

    let mut lines: Vec<String> = Vec::new();
    let mut director_line_idx: Option<usize> = None;
    if !item.overview.is_empty() || !item.director.is_empty() {
        let cleaned_overview = if truncate_overview {
            trunc_overview(&item.overview)
        } else {
            clean_overview(&item.overview)
        };
        for paragraph in cleaned_overview.lines() {
            let paragraph = if paragraph.trim().is_empty() {
                " "
            } else {
                paragraph.trim()
            };
            let line_idx = lines.len();
            let wrap_w = if line_idx < shadow_lines {
                narrow_w.max(1)
            } else {
                inner_w.max(1)
            };
            for wrapped in wrap(paragraph, wrap_w) {
                lines.push(wrapped.into_owned());
            }
        }

        // Director flows after the overview: blank gap then the director
        // line (rendered specially so its "Director: " label keeps its
        // own style, matching the banner's previous look).
        if !item.director.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            director_line_idx = Some(lines.len());
            lines.push(String::new());
        }
    }

    CompactBannerLayout {
        meta_line,
        show_playing,
        lines,
        director_line_idx,
        img_actual_w,
        img_height,
        img_is_placeholder,
    }
}

/// Everything the shell resolves for the pure compact-banner painter: the
/// selected item plus its already-computed [`CompactBannerLayout`] (so this
/// path never re-enters `compact_banner_layout_with_overview` or the image
/// cache).
pub(in crate::app) struct CompactDetailCtx<'a> {
    pub(in crate::app) item: &'a mbv_core::api::EmbyItem,
    pub(in crate::app) layout: CompactBannerLayout,
}

/// Pure compact movie/Series banner painter: builds the [`HeroContent`],
/// paints it, and returns the poster image still needing paint as a
/// [`HomeImagePaint::CompactBanner`] request (executed by
/// [`App::paint_home_image`]). No `App`, no image-cache access, no fetch.
pub(in crate::app) fn render_compact_detail_with_ctx(
    ctx: CompactDetailCtx<'_>,
    f: &mut Frame,
    area: Rect,
    focused: bool,
    show_title: bool,
) -> Option<HomeImagePaint> {
    let CompactDetailCtx {
        item,
        layout: content,
    } = ctx;
    if area.height == 0 || area.width < 3 {
        return None;
    }

    // — Title —
    // The caller decides whether the selected item's name belongs in the
    // hero or remains in the ordinary list row.
    let title = item.display_name();

    // — Overview + Director (#204, #263) —
    // The Director line is rendered specially (its own label style); every
    // other line is the banner's wrapped overview text, which grows to
    // fit the block's full wrapped height (computed by
    // `compact_banner_layout` and consumed by the list layout before any
    // of this renders, so `area` is already sized to fit every line)
    // instead of clipping or scrolling it.
    let lines: Vec<HeroLine> = content
        .lines
        .iter()
        .enumerate()
        .map(|(idx, line_text)| {
            if Some(idx) == content.director_line_idx {
                HeroLine::Prefixed {
                    label: "Director: ",
                    value: item.director.clone(),
                }
            } else {
                HeroLine::Plain(line_text.clone())
            }
        })
        .collect();

    let hero_content = HeroContent {
        title: show_title.then_some(title.as_str()),
        // The metadata row directly below the selected movie title
        // renders in #9e9e9e foreground (palette::TEXT_SECONDARY) — light grey
        // text on the SURFACE_FOCUSED block that frames the selected
        // row + banner.
        meta_line: content.meta_line.as_deref(),
        meta_color: palette::TEXT_DETAIL_META,
        show_playing: content.show_playing,
        unconditional_spacer_after_meta: false,
        lines: &lines,
        image: (content.img_height > 0).then_some(HeroImage {
            actual_w: content.img_actual_w,
            height: content.img_height,
        }),
    };
    let result =
        crate::app::render::components::hero::paint_hero_content(f, area, &hero_content, focused);

    let img_rect = result.img_rect?;
    Some(HomeImagePaint::CompactBanner {
        area: img_rect,
        item: Box::new(item.clone()),
        show_placeholder: content.img_is_placeholder,
    })
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
