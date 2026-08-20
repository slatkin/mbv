use super::super::ui_util::*;
use super::home_video::format_release_date;
use super::RENDER_FILTER;
use crate::app::{palette, App};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::QueueItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use textwrap::wrap;

/// The two-column (wide) hero's original 2-col horizontal padding around
/// the overview text block. The single-column hero has none (flush with
/// the title above it).
pub(super) const WIDE_OVERVIEW_PAD: usize = 2;

/// Pre-wrapped content for a legacy top placement item's metadata column, plus the
/// total row count it needs. Computed once (mirroring `compact_banner_layout`'s
/// measure-before-render pattern) so the caller can size the panel to fit
/// before rendering, and so the title and overview are wrapped exactly once
/// per frame rather than once to measure and again to render. Shared by the
/// Emby Keep Watching hero and the generic Audiobookshelf hero -- both are
/// beside-image, legacy top placement items and use the same wrap-around-the-image
/// shape.
pub(super) struct KeepWatchingHeroLayout {
    title_lines: Vec<String>,
    show_name: String,
    /// Overview text lines with a per-line flag: `true` once the line has
    /// wrapped past the image's row extent and reclaims the full hero
    /// width (the image no longer occupies that row), `false` while beside
    /// the image at the narrower meta-column width.
    overview_lines: Vec<(String, bool)>,
    pub(super) height: u16,
}

/// Per-provider metadata for the shared legacy top placement meta block: an optional
/// glyph drawn one space after the last title line (Emby's watch-state icon),
/// plus the metadata rows below the subtitle (release date, duration, ...).
pub(super) struct HeroMetaBlock {
    pub title_suffix: Option<Span<'static>>,
    pub meta_rows: Vec<Vec<Span<'static>>>,
}

/// Prepares a wide (hero-on-left) selected-Emby hero card from `item`,
/// sized into the given content area (the left pane's inner rect after
/// padding). Returns the data needed to build `HeroData::Emby`, or `None`
/// when the area is too small for a usable card (image and metadata).
/// Shared by Home's wide branch and the wide Movies arrangement so the two
/// render the exact same 16:9-artwork-above-metadata card: image occupies
/// the top of the content area at 16:9, metadata (title, show name,
/// release date, duration, overview) below it. The metadata layout uses the
/// full content width for both narrow and wide wrapping (no wrap-around
/// split — the image sits above text, not beside it), matching Home's wide
/// hero-on-left presentation.
pub(super) fn prepare_wide_emby_hero_card(
    item: &mbv_core::api::EmbyItem,
    content_area: Rect,
) -> Option<(KeepWatchingHeroLayout, Rect, Rect)> {
    let meta_w = content_area.width as usize;
    let meta_layout = App::keep_watching_hero_layout(item, meta_w, meta_w, 0, WIDE_OVERVIEW_PAD);
    // Terminal cells are roughly twice as tall as they are wide, so a
    // 16:9 image needs 9 rows for every 32 columns. Ceiling (matching
    // `render_keep_watching_hero_image`'s own budget) so the reserved
    // box never ends above where the image actually draws -- a smaller
    // box would let the image's last row overlap the title's first row.
    let image_height = (content_area.width.saturating_mul(9).saturating_add(31) / 32)
        .max(1)
        .min(content_area.height.saturating_sub(meta_layout.height));
    if meta_layout.height < 4 || image_height == 0 {
        return None;
    }
    let img_area = Rect {
        x: content_area.x,
        y: content_area.y,
        width: content_area.width,
        height: image_height,
    };
    let meta_area = Rect {
        x: content_area.x,
        y: content_area.y + img_area.height + 1,
        width: content_area.width,
        height: meta_layout.height,
    };
    Some((meta_layout, meta_area, img_area))
}

pub(super) enum HeroData {
    Emby(
        Box<mbv_core::api::EmbyItem>,
        Rect,
        Rect,
        Rect,
        KeepWatchingHeroLayout,
    ),
    GenericBeside(QueueItem, Rect, Rect, Rect, KeepWatchingHeroLayout),
    Generic(QueueItem, Rect),
}

impl App {
    /// Image types to request for the Keep Watching hero panel, mirroring
    /// the per-type conventions used for the queue card (`render_card`).
    pub(super) fn keep_watching_hero_image_types(
        item: &mbv_core::api::EmbyItem,
    ) -> &'static [&'static str] {
        match item.item_type.as_str() {
            "Movie" => &["Backdrop", "Primary", "Logo"],
            _ => &["Primary", "Backdrop"],
        }
    }

    /// Builds the Keep Watching hero panel's metadata layout for `item`,
    /// delegating to the shared [`hero_text_layout`] (also used by the
    /// generic Audiobookshelf hero, so both legacy top placement items share one
    /// wrap-around-the-image implementation).
    pub(super) fn keep_watching_hero_layout(
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
        hero_text_layout(
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
    pub(super) fn render_keep_watching_hero_image(
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
    pub(super) fn keep_watching_hero_meta_block(
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
            (if use_nerd_fonts { "\u{e001}" } else { "●" }, palette::AQUA)
        } else if item.playback_position_ticks > 0 {
            (
                if use_nerd_fonts { "\u{e004}" } else { "◐" },
                palette::YELLOW,
            )
        } else {
            (if use_nerd_fonts { "\u{e002}" } else { "○" }, palette::RED)
        };
        let title_suffix = Some(Span::styled(glyph, Style::default().fg(color)));

        let mut meta_rows: Vec<Vec<Span<'static>>> = Vec::new();
        if !release_date.is_empty() {
            meta_rows.push(vec![Span::styled(
                release_date,
                Style::default().fg(palette::SUBTLE),
            )]);
        }
        if !dur_str.is_empty() {
            meta_rows.push(vec![Span::styled(
                trunc_str(&dur_str, width as usize),
                Style::default().fg(palette::GREEN),
            )]);
        }
        HeroMetaBlock {
            title_suffix,
            meta_rows,
        }
    }

    /// Renders a [`KeepWatchingHeroLayout`]'s title/show-name/meta/overview
    /// block, shared by the Emby Keep Watching hero and the generic
    /// Audiobookshelf hero (`home.rs`'s narrow-layout beside-image case) --
    /// only the per-provider `meta_spans` (release date/progress for Emby,
    /// duration alone for Audiobookshelf) differ.
    pub(super) fn render_hero_layout_meta(
        &self,
        f: &mut Frame,
        area: Rect,
        wide_area: Rect,
        layout: &KeepWatchingHeroLayout,
        meta_block: HeroMetaBlock,
        overview_pad: u16,
        focused: bool,
    ) {
        super::hero::render_home_hero_meta_block(
            f,
            area,
            wide_area,
            &layout.title_lines,
            &layout.show_name,
            meta_block.title_suffix,
            meta_block.meta_rows,
            &layout.overview_lines,
            overview_pad,
            focused,
        );
    }

    /// Renders a beside-image legacy top placement item: image column then text
    /// column, unconditionally and in that order for every provider. The
    /// single call site every legacy top placement item with a cover goes through
    /// (Emby Keep Watching and the generic Audiobookshelf hero), so the two
    /// can't drift out of sync the way two hand-written call sites can --
    /// the image render in particular must never be skipped (not even when
    /// there's no cache key yet), or the image column is left showing
    /// whatever the terminal had there before, with text drawn over it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn render_beside_image_hero(
        &mut self,
        f: &mut Frame,
        meta_area: Rect,
        wide_area: Rect,
        img_area: Rect,
        layout: &KeepWatchingHeroLayout,
        meta_block: HeroMetaBlock,
        image_cache_key: &str,
        overview_pad: u16,
        focused: bool,
        centered: bool,
    ) {
        self.render_keep_watching_hero_image(f, img_area, image_cache_key, centered);
        self.render_hero_layout_meta(
            f,
            meta_area,
            wide_area,
            layout,
            meta_block,
            overview_pad,
            focused,
        );
    }

    pub(super) fn render_home_hero_data(
        &mut self,
        f: &mut Frame,
        hero_data: &HeroData,
        two_column: bool,
        focused: bool,
    ) {
        let overview_pad = if two_column {
            WIDE_OVERVIEW_PAD as u16
        } else {
            0
        };
        match hero_data {
            HeroData::Emby(item, meta_area, wide_area, img_area, meta_layout) => {
                let cache_key = format!("{}:pwr_kw", item.id);
                if self.images_enabled() {
                    let img_types = Self::keep_watching_hero_image_types(item);
                    self.fetch_card_image(
                        cache_key.clone(),
                        item.id.clone(),
                        item.series_id.clone(),
                        img_types,
                    );
                }
                let meta_block =
                    Self::keep_watching_hero_meta_block(item, meta_area.width, self.use_nerd_fonts);
                self.render_beside_image_hero(
                    f,
                    *meta_area,
                    *wide_area,
                    *img_area,
                    meta_layout,
                    meta_block,
                    &cache_key,
                    overview_pad,
                    focused,
                    two_column,
                );
            }
            HeroData::GenericBeside(item, meta_area, wide_area, img_area, meta_layout) => {
                let cache_key = match item {
                    QueueItem::Audiobookshelf(episode) => self
                        .audiobookshelf_cover_key(&episode.library_item_id)
                        .unwrap_or_default(),
                    _ => String::new(),
                };
                let meta_block = HeroMetaBlock {
                    title_suffix: None,
                    meta_rows: item
                        .duration()
                        .map(|ticks| {
                            vec![vec![Span::styled(
                                trunc_str(
                                    &fmt_duration_short((ticks / TICKS_PER_SECOND as u64) as i64),
                                    meta_area.width as usize,
                                ),
                                Style::default().fg(palette::SUBTLE),
                            )]]
                        })
                        .unwrap_or_default(),
                };
                self.render_beside_image_hero(
                    f,
                    *meta_area,
                    *wide_area,
                    *img_area,
                    meta_layout,
                    meta_block,
                    &cache_key,
                    0,
                    focused,
                    false,
                );
            }
            HeroData::Generic(item, area) => {
                self.render_home_latest_detail(f, *area, item, focused, overview_pad as usize);
            }
        }
    }
}

/// Beside-image legacy top placement dims: image width, the wrap-around text layout,
/// and the image's row count. The single source of this geometry for every
/// legacy top placement item with a cover -- Emby Keep Watching and the generic
/// Audiobookshelf hero both call this so their layouts can't drift apart.
pub(super) fn beside_image_hero_dims(
    title: &str,
    show_name: &str,
    overview: &str,
    inner_w: u16,
    max_allowed: u16,
    meta_row_count: u16,
) -> (u16, KeepWatchingHeroLayout, u16) {
    let img_w = inner_w / 2;
    let meta_w = inner_w.saturating_sub(img_w + 1) as usize;
    let image_rows = (img_w.saturating_mul(9).saturating_add(31) / 32).min(max_allowed);
    let layout = hero_text_layout(
        title,
        show_name,
        overview,
        meta_w,
        inner_w as usize,
        image_rows,
        0,
        meta_row_count,
    );
    (img_w, layout, image_rows)
}

/// Beside-image legacy top placement `Rect`s: the metadata column (left) and image
/// column (right), both stretched to the taller of the two so the shorter
/// one's background/border still spans the full row height. The single
/// source of this geometry, shared the same way as [`beside_image_hero_dims`].
pub(super) fn beside_image_hero_rects(
    hero_content: Rect,
    img_w: u16,
    layout_height: u16,
    image_rows: u16,
) -> (Rect, Rect) {
    // Clamp to `hero_content.height` -- the panel's actual granted height,
    // which `placement-neutral geometry` can clamp smaller than what `image_rows`/
    // `layout_height` asked for when the terminal doesn't have room for
    // everything requested. Sizing the image/meta `Rect`s from the desired
    // height alone (unclamped) lets them extend past the hero panel's real
    // bottom edge, where the image's overflow gets drawn over by whatever
    // renders below it (pills/list) -- looking like the image is cut off.
    let hero_height = image_rows.max(layout_height).min(hero_content.height);
    let meta_area = Rect {
        x: hero_content.x,
        y: hero_content.y,
        width: hero_content.width.saturating_sub(img_w + 1),
        height: hero_height,
    };
    let img_area = Rect {
        x: hero_content.x + hero_content.width.saturating_sub(img_w),
        y: hero_content.y,
        width: img_w,
        height: hero_height,
    };
    (meta_area, img_area)
}

/// Wrap-around-the-image text layout shared by every legacy top placement item with a
/// beside-text image: title wrap lines, then one row each for the show-name
/// line, the duration/progress line, and the blank separator, then the
/// wrapped overview. The overview wraps around the image: it wraps at
/// `text_w` (the meta column, beside the image) for however many of its rows
/// still fall within `image_rows`, then reclaims the full `wide_w` for any
/// remaining rows once past the image's bottom edge. `overview_pad` is the
/// two-column (wide) hero's original 2-col horizontal padding around the
/// overview block; the single-column hero passes 0 so its overview stays
/// flush with the title above it. `meta_row_count` is the number of reserved
/// metadata rows below the title/show-name (Emby's hero now uses 2: one for
/// the release date, one for the duration; other heroes use 1).
pub(super) fn hero_text_layout(
    title: &str,
    show_name: &str,
    overview: &str,
    text_w: usize,
    wide_w: usize,
    image_rows: u16,
    overview_pad: usize,
    meta_row_count: u16,
) -> KeepWatchingHeroLayout {
    if text_w == 0 {
        return KeepWatchingHeroLayout {
            title_lines: Vec::new(),
            show_name: String::new(),
            overview_lines: Vec::new(),
            height: 0,
        };
    }
    let title_lines: Vec<String> = wrap(title, text_w)
        .into_iter()
        .map(|s| s.into_owned())
        .collect();
    let header_rows = title_lines.len() as u16
        + if show_name.is_empty() { 0 } else { 1 } // show name row (only for episodes)
        + meta_row_count // metadata rows (release date, duration, ...)
        + 1; // blank separator row
    let ov_text_w = text_w.saturating_sub(overview_pad * 2);
    let ov_wide_w = wide_w.saturating_sub(overview_pad * 2);
    let overview_lines: Vec<(String, bool)> = if overview.is_empty() {
        Vec::new()
    } else {
        let narrow_capacity = image_rows.saturating_sub(header_rows) as usize;
        if narrow_capacity == 0 {
            wrap(overview, ov_wide_w.max(1))
                .into_iter()
                .map(|s| (s.into_owned(), true))
                .collect()
        } else {
            let narrow_all: Vec<String> = wrap(overview, ov_text_w)
                .into_iter()
                .map(|s| s.into_owned())
                .collect();
            if narrow_all.len() <= narrow_capacity {
                narrow_all.into_iter().map(|l| (l, false)).collect()
            } else {
                let consumed_words: usize = narrow_all[..narrow_capacity]
                    .iter()
                    .map(|l| l.split_whitespace().count())
                    .sum();
                let remainder: String = overview
                    .split_whitespace()
                    .skip(consumed_words)
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut lines: Vec<(String, bool)> = narrow_all[..narrow_capacity]
                    .iter()
                    .cloned()
                    .map(|l| (l, false))
                    .collect();
                lines.extend(
                    wrap(&remainder, ov_wide_w.max(1))
                        .into_iter()
                        .map(|s| (s.into_owned(), true)),
                );
                lines
            }
        }
    };
    let height = header_rows
        + if overview_lines.is_empty() {
            0
        } else {
            overview_lines.len() as u16
                + 1 // overview lines + bottom pad
                + if overview_pad > 0 {
                    1 // gap row above the hero-on-left overview box
                } else {
                    0
                }
        };
    KeepWatchingHeroLayout {
        title_lines,
        show_name: show_name.to_string(),
        overview_lines,
        height,
    }
}
