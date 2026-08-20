use super::super::ui_util::*;
use super::hero::{HeroContent, HeroImage, HeroLine, ImageTop};
use super::RENDER_FILTER;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
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
pub(super) fn compact_banner_image_cache_key(item_id: &str) -> String {
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
/// `render_compact_detail` to actually render the banner, so the
/// row-count estimate and the render never duplicate the wrapping logic.
pub(super) struct CompactBannerLayout {
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
    pub(super) fn content_rows(&self) -> usize {
        let text_rows =
            self.meta_line.is_some() as usize + self.show_playing as usize + self.lines.len();
        text_rows.max(self.img_height as usize)
    }
}

impl App {
    pub(crate) fn selected_movie_item(&self, lib_idx: usize) -> Option<mbv_core::api::EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        let coll = lib.library.collection_type.as_str();
        if coll != "movies" && coll != "homevideos" && coll != "podcasts" {
            return None;
        }

        let item = if self.is_feed_home_video_group_view(lib_idx) {
            self.selected_feed_home_video_item(lib_idx)?
        } else {
            let level = lib.nav_stack.last()?;
            level.items.get(level.cursor)?.clone()
        };

        if item.is_folder {
            return None;
        }
        if coll == "movies" && item.item_type != "Movie" {
            return None;
        }

        Some(item)
    }

    pub(crate) fn selected_series_item(&self, lib_idx: usize) -> Option<mbv_core::api::EmbyItem> {
        let lib = self.libs.get(lib_idx)?;
        if lib.library.collection_type != "tvshows" {
            return None;
        }

        let item = if let Some(search) = &lib.search {
            let item_idx = *search.results.get(search.cursor)?;
            search.items.get(item_idx)?.clone()
        } else {
            let level = lib.nav_stack.last()?;
            level.items.get(level.cursor)?.clone()
        };

        if item.item_type != "Series" {
            return None;
        }

        Some(item)
    }

    /// Computes the compact banner's content for `item`, given the panel
    /// width it will render into (i.e. the eventual `area.width` passed to
    /// `render_compact_detail`). Pure function of `item` + width aside
    /// from the image-state cache lookup/fetch-trigger, so calling it twice
    /// per frame (once to measure, once to render) is safe and idempotent.
    pub(super) fn compact_banner_layout_with_overview(
        &mut self,
        item: &mbv_core::api::EmbyItem,
        panel_width: u16,
        truncate_overview: bool,
    ) -> CompactBannerLayout {
        let inner_w = (panel_width as usize).saturating_sub(2);

        let primary_cache_key = compact_banner_image_cache_key(&item.id);
        if self.images_enabled() {
            self.fetch_card_image(
                primary_cache_key.clone(),
                item.id.clone(),
                item.series_id.clone(),
                &["Primary"],
            );
        }

        // `right_panel_image_renders_allowed()` (the 150ms nav-idle debounce) exists
        // to stop the *real* poster from flickering in and out while rapidly
        // scrolling through many different movies -- it must keep gating
        // which image is actually substituted in. But the placeholder box's
        // size is fixed (IMG_COLS x IMG_ROWS) regardless of which movie is
        // selected, so reserving it doesn't cause that flicker; gating the
        // reservation itself only desynced the poster's placeholder from the
        // rest of the banner's content (meta line, overview), which renders
        // at its final layout immediately, on the very first frame. So the
        // placeholder is reserved unconditionally here whenever a real image
        // isn't yet ready to show, and only the "is it the real image or the
        // placeholder" choice below still depends on the nav-idle gate.
        let nav_gate_open = self.right_panel_image_renders_allowed();
        // Cap the poster's bounding-box width at half the panel's inner width,
        // so a narrow panel (single-panel view) doesn't reserve the same
        // wide/tall box a full-width panel would -- the fixed IMG_COLS box
        // looked oversized (and briefly all the more so as the placeholder)
        // once the panel got narrow enough for it to dominate the banner.
        let img_cols = IMG_COLS.min((inner_w / 2) as u16);
        // `image_picker` is only `None` before the run loop's one-time init
        // (or in tests that don't set one up) -- fall back to the full
        // bounding box in that case, since there's no real font metrics yet
        // to fit the canonical poster aspect ratio against.
        let (placeholder_w, placeholder_h) = self
            .image_picker
            .as_ref()
            .map(|picker| poster_placeholder_size(picker.font_size(), img_cols))
            .unwrap_or((img_cols, IMG_ROWS));

        let has_no_art = self
            .card_image_states
            .get(&primary_cache_key)
            .is_some_and(|e| e.img.is_none());
        let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) =
            if !self.images_enabled() {
                (0, 0, false)
            } else if has_no_art {
                // Fetch resolved with no image for this movie -- nothing to
                // reserve space for.
                (0, 0, false)
            } else if nav_gate_open {
                match self.cached_image_protocol_mut(&primary_cache_key) {
                    // A real image resolved; show it (or, if resize+encode is
                    // still running on the worker thread -- `size_for` is
                    // `None` -- keep showing the placeholder a beat longer).
                    Some(state) => match state.size_for(
                        ratatui_image::Resize::Scale(Some(RENDER_FILTER)),
                        ratatui::layout::Size {
                            width: img_cols,
                            height: IMG_ROWS,
                        },
                    ) {
                        Some(actual) => (actual.width, actual.height, false),
                        None => (placeholder_w, placeholder_h, true),
                    },
                    // Either the fetch is still in flight (no entry yet), or a
                    // real image already resolved but the nav-idle gate hasn't
                    // opened yet -- either way, reserve the placeholder now.
                    None => (placeholder_w, placeholder_h, true),
                }
            } else {
                (placeholder_w, placeholder_h, true)
            };

        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);

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

        let playback = self.effective_playback_state();
        let now_playing_id: Option<String> = if playback.active {
            self.playback_queue()
                .emby_item_at(playback.active_idx)
                .map(|i| i.id.clone())
        } else {
            None
        };
        let show_playing = now_playing_id.as_deref() == Some(item.id.as_str());
        if show_playing {
            rows_before_overview += 1;
        }

        // Rows before the overview block sit above the poster image's
        // bottom edge too (as long as there aren't more of them than the
        // image is tall), so they narrow the wrap width the same way
        // overview/director lines do; `shadow_lines` counts how many of the
        // *upcoming* overview/director lines still fall within the image's
        // row span.
        let shadow_lines = (img_height as usize).saturating_sub(rows_before_overview);

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

    pub(crate) fn render_compact_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        show_title: bool,
        _layout: &mut LayoutMain,
    ) {
        // The hero shows the selected leaf movie (movies/homevideos/podcasts)
        // or, on a tvshows library, the selected Series — the compact banner
        // layout is generic over the item, so a Series renders its meta +
        // overview the same way a Movie does (design decision 6).
        let Some(item) = self
            .selected_movie_item(lib_idx)
            .or_else(|| self.selected_series_item(lib_idx))
        else {
            return;
        };
        if area.height == 0 || area.width < 3 {
            return;
        }

        let truncate_overview =
            self.is_home_video_view(lib_idx) || self.is_podcast_library(lib_idx);
        let content =
            self.compact_banner_layout_with_overview(&item, area.width, truncate_overview);

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
            // renders in #9e9e9e foreground (palette::SUBTLE) — light grey
            // text on the SURFACE_FOCUSED block that frames the selected
            // row + banner.
            meta_line: content.meta_line.as_deref(),
            meta_color: palette::MUTED_GREEN,
            show_playing: content.show_playing,
            unconditional_spacer_after_meta: false,
            lines: &lines,
            // The poster is right-aligned on the hero's top row, sharing that
            // row with the title in two-column lists or flush with the
            // hero's top border in one-column lists.
            image: (content.img_height > 0).then_some(HeroImage {
                actual_w: content.img_actual_w,
                height: content.img_height,
                top: ImageTop::AreaTop,
            }),
        };
        let result = super::hero::paint_hero_content(f, area, &hero_content, focused);

        if let Some(img_rect) = result.img_rect {
            if content.img_is_placeholder {
                // Image still loading -- draw a dim placeholder block to
                // hold the space (mirrors episode.rs's series-image
                // placeholder).
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                    img_rect,
                );
            } else {
                let primary_cache_key = compact_banner_image_cache_key(&item.id);
                if let Some(state) = self.cached_image_protocol_mut(&primary_cache_key) {
                    type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                    f.render_stateful_widget(
                        SImg::default().resize(ratatui_image::Resize::Scale(Some(RENDER_FILTER))),
                        img_rect,
                        state,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
