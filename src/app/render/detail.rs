use super::super::ui_util::*;
use super::POWER_RENDER_FILTER;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

const IMG_COLS: u16 = 24;
const IMG_ROWS: u16 = 14;

/// Cache key for the compact movie banner's poster image, under which
/// `fetch_card_image`/`fetch_list_card_image_when_idle` store and look up the
/// resized/encoded image state. Shared by the eager fetch in
/// `compact_banner_layout` and the prefetch loop in `list.rs`'s
/// `render_power_list` (#287) so the two can never format the key
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
fn poster_placeholder_size(font_size: ratatui_image::FontSize) -> (u16, u16) {
    let canonical_poster_aspect = image::DynamicImage::new_rgb8(2, 3);
    let size = ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)).size_for(
        &canonical_poster_aspect,
        font_size,
        ratatui::layout::Size {
            width: IMG_COLS,
            height: IMG_ROWS,
        },
    );
    (size.width, size.height)
}

/// Everything content-dependent about the compact movie-detail banner: the
/// meta line, the "Playing" indicator, and the overview + director text
/// wrapped to the banner's actual panel width. Computed once by
/// `App::compact_banner_layout` and consumed both to size the banner's row
/// budget in the list layout (`list::compact_banner_rows`, run *before* the
/// rest of the list's rows are positioned) and to actually render the
/// banner (`render_power_compact_detail`) -- the two-pass split this issue
/// (#263) introduces, kept in lockstep by sharing this one computation
/// instead of the row count and the render duplicating the wrapping logic.
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
    pub(crate) fn power_selected_movie_item(
        &self,
        lib_idx: usize,
    ) -> Option<mbv_core::api::MediaItem> {
        let lib = self.libs.get(lib_idx)?;
        let coll = lib.library.collection_type.as_str();
        if coll != "movies" && coll != "homevideos" && coll != "podcasts" {
            return None;
        }

        let item = if self.is_feed_home_video_group_view(lib_idx) {
            self.selected_feed_home_video_item(lib_idx)?
        } else if let Some(search) = &lib.search {
            let &idx = search.results.get(search.cursor)?;
            search.items.get(idx)?.clone()
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

    pub(crate) fn power_selected_series_item(
        &self,
        lib_idx: usize,
    ) -> Option<mbv_core::api::MediaItem> {
        let lib = self.libs.get(lib_idx)?;
        if lib.library.collection_type != "tvshows" {
            return None;
        }

        let item = if let Some(search) = &lib.search {
            let &idx = search.results.get(search.cursor)?;
            search.items.get(idx)?.clone()
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
    /// `render_power_compact_detail`). Pure function of `item` + width aside
    /// from the image-state cache lookup/fetch-trigger, so calling it twice
    /// per frame (once to measure, once to render) is safe and idempotent.
    pub(super) fn compact_banner_layout(
        &mut self,
        item: &mbv_core::api::MediaItem,
        panel_width: u16,
    ) -> CompactBannerLayout {
        self.compact_banner_layout_with_overview(item, panel_width, false)
    }

    pub(super) fn compact_banner_layout_with_overview(
        &mut self,
        item: &mbv_core::api::MediaItem,
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

        // `power_right_panel_image_renders_allowed()` (the 150ms nav-idle debounce) exists
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
        let nav_gate_open = self.power_right_panel_image_renders_allowed();
        // `image_picker` is only `None` before the run loop's one-time init
        // (or in tests that don't set one up) -- fall back to the full
        // bounding box in that case, since there's no real font metrics yet
        // to fit the canonical poster aspect ratio against.
        let (placeholder_w, placeholder_h) = self
            .image_picker
            .as_ref()
            .map(|picker| poster_placeholder_size(picker.font_size()))
            .unwrap_or((IMG_COLS, IMG_ROWS));

        let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) =
            if !self.images_enabled() {
                (0, 0, false)
            } else {
                match self.card_image_states.get_mut(&primary_cache_key) {
                    // Fetch resolved with no image for this movie -- nothing to
                    // reserve space for.
                    Some(None) => (0, 0, false),
                    // Fetch resolved with a real image, and the nav-idle gate is
                    // open: show it (or, if resize+encode is still running on
                    // the worker thread -- `size_for` is `None` -- keep showing
                    // the placeholder a beat longer).
                    Some(Some(state)) if nav_gate_open => {
                        match state.size_for(
                            ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER)),
                            ratatui::layout::Size {
                                width: IMG_COLS,
                                height: IMG_ROWS,
                            },
                        ) {
                            Some(actual) => (actual.width, actual.height, false),
                            None => (placeholder_w, placeholder_h, true),
                        }
                    }
                    // Either the fetch is still in flight (no entry yet), or a
                    // real image already resolved but the nav-idle gate hasn't
                    // opened yet -- either way, reserve the placeholder now.
                    _ => (placeholder_w, placeholder_h, true),
                }
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
                .items
                .get(playback.active_idx)
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

    pub(crate) fn render_power_compact_detail(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        let Some(item) = self.power_selected_movie_item(lib_idx) else {
            return;
        };
        if area.height == 0 || area.width < 3 {
            return;
        }

        layout.cursor_screen_y = Some(area.y);

        let truncate_overview =
            self.is_home_video_view(lib_idx) || self.is_podcast_library(lib_idx);
        let content =
            self.compact_banner_layout_with_overview(&item, area.width, truncate_overview);

        let inner_x = area.x;
        let inner_w = area.width as usize;
        let inner_w16 = area.width;
        let mut row = area.y;
        let max_y = area.y + area.height;

        let text_color = if focused {
            palette::WHITE
        } else {
            palette::SUBTLE
        };

        let img_actual_w = content.img_actual_w;
        let img_height = content.img_height;
        let img_is_placeholder = content.img_is_placeholder;
        let img_x = area.x + area.width.saturating_sub(img_actual_w);
        // No title row is drawn here anymore (it duplicated the selected list
        // row's title, already shown in green just above the banner), so the
        // poster starts flush with the banner's own top row instead of being
        // pushed down a row to make room for a redundant title.
        let img_y = area.y.min(area.y + area.height.saturating_sub(1));
        let img_end_row = img_y + img_height;
        layout.inline_image_rect = if img_height > 0 {
            Some(Rect {
                x: img_x,
                y: img_y,
                width: img_actual_w,
                height: img_height,
            })
        } else {
            None
        };

        let narrow_w = inner_w.saturating_sub(img_actual_w as usize);
        let narrow_w16 = inner_w16.saturating_sub(img_actual_w);
        let text_dims = |r: u16| -> (usize, u16) {
            if img_height > 0 && r < img_end_row {
                (narrow_w, narrow_w16)
            } else {
                (inner_w, inner_w16)
            }
        };

        if let Some(meta) = &content.meta_line {
            if row < max_y {
                let (tw, tw16) = text_dims(row);
                // The metadata row directly below the selected movie title
                // renders in #9e9e9e foreground (palette::SUBTLE) — light grey
                // text on the MEDIA_SELECTED_BG block that frames the
                // selected row + banner.
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(meta, tw),
                        Style::default().fg(palette::MUTED_GREEN),
                    ))),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
                row += 1;
                // Spacer row between metadata and description
                row += 1;
            }
        }

        if content.show_playing && row < max_y {
            let (_tw, tw16) = text_dims(row);
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "Playing",
                    Style::default()
                        .fg(palette::GREEN)
                        .add_modifier(Modifier::BOLD),
                ))),
                Rect {
                    x: inner_x,
                    y: row,
                    width: tw16,
                    height: 1,
                },
            );
            row += 1;
        }

        // — Overview + Director (#204, #263) —
        // The banner grows to fit this block's full wrapped height (computed
        // by `compact_banner_layout` and consumed by the list layout before
        // any of this renders, so `area` is already sized to fit every
        // line) instead of clipping or scrolling it.
        for (idx, line_text) in content.lines.iter().enumerate() {
            if row >= max_y {
                break;
            }
            let (tw, tw16) = text_dims(row);
            if Some(idx) == content.director_line_idx {
                f.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled("Director: ", Style::default().fg(palette::MUTED_GREEN)),
                        Span::styled(
                            trunc_str(&item.director, tw),
                            Style::default().fg(palette::TEXT),
                        ),
                    ])),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
            } else if !line_text.is_empty() {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        trunc_str(line_text, tw),
                        Style::default().fg(text_color),
                    ))),
                    Rect {
                        x: inner_x,
                        y: row,
                        width: tw16,
                        height: 1,
                    },
                );
            }
            row += 1;
        }

        if img_height > 0 {
            let img_rect = Rect {
                x: img_x,
                y: img_y,
                width: img_actual_w,
                height: img_height,
            };
            if img_is_placeholder {
                // Image still loading -- draw a dim placeholder block to
                // hold the space (mirrors episode.rs's series-image
                // placeholder).
                f.render_widget(
                    Block::default().style(Style::default().bg(palette::OVERLAY)),
                    img_rect,
                );
            } else {
                let primary_cache_key = compact_banner_image_cache_key(&item.id);
                if let Some(Some(state)) = self.card_image_states.get_mut(&primary_cache_key) {
                    type SImg = ratatui_image::StatefulImage<ratatui_image::thread::ThreadProtocol>;
                    f.render_stateful_widget(
                        SImg::default()
                            .resize(ratatui_image::Resize::Scale(Some(POWER_RENDER_FILTER))),
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
