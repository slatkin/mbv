use crate::app::palette;
use crate::app::render::components::hero::{
    inline_hero_text_width, wrap_overview_lines, HeroContent, HeroImage, HeroLine,
};
use crate::app::render::HomeImagePaint;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::widgets::*;
use ratatui::Frame;

pub(in crate::app::render) const SERIES_DETAIL_DIVIDER_ROWS: usize = 1;
pub(in crate::app::render) const SERIES_DETAIL_EPISODE_ROWS_ESTIMATE: usize = 8;
pub(in crate::app::render) const SERIES_DETAIL_TRAILING_BLANK_ROWS: usize = 1;
pub(in crate::app::render) const SERIES_IMAGE_COLS: u16 = 18;
pub(in crate::app::render) const SERIES_IMAGE_ROWS: u16 = 12;
pub(in crate::app::render) const SERIES_IMAGE_PLACEHOLDER_ROWS: u16 = SERIES_IMAGE_ROWS;

pub(in crate::app::render) fn series_meta_line(item: &mbv_core::api::EmbyItem) -> String {
    let year_range = match (item.production_year, item.end_year) {
        (s, e) if s > 0 && e > 0 && e != s => format!("{}-{}", s, e),
        (s, _) if s > 0 => format!("{}", s),
        _ => String::new(),
    };
    let genre_upper = item.genre.to_uppercase();
    [year_range.as_str(), genre_upper.as_str()]
        .iter()
        .filter(|s| !s.is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("  ")
}

/// Renders the selected Series' season pills + episode table into the
/// inline hero slot (`render_list` reserves `area`'s rows via
/// `series_inline_detail_rows` and paints the surrounding block
/// border/background itself -- this draws only the content, mirroring
/// how `render_compact_detail` is the movie hero's content-only
/// counterpart).
pub(in crate::app::render) struct SeriesInlineDetailCtx<'a> {
    pub(in crate::app::render) item: &'a mbv_core::api::EmbyItem,
    pub(in crate::app::render) images_enabled: bool,
    pub(in crate::app::render) image_loading: bool,
}

pub(in crate::app::render) fn render_series_inline_detail(
    ctx: SeriesInlineDetailCtx<'_>,
    f: &mut Frame,
    area: Rect,
    focused: bool,
    show_title: bool,
) -> Option<HomeImagePaint> {
    if area.height == 0 {
        return None;
    }

    let item = ctx.item;
    let max_y = area.y + area.height;

    // ── Series Primary image sizing (right-aligned, text wraps around
    //    it) -- resolved here (needs `self`'s image cache) and handed to
    //    the `Hero` component to lay text out around ───────────────────
    let img_loading = !item.id.is_empty() && ctx.images_enabled && ctx.image_loading;
    let (img_actual_w, img_height, img_is_placeholder): (u16, u16, bool) = {
        if ctx.images_enabled {
            // The shell owns cache lookup. `image_loading` is the only
            // component-visible state that should select the placeholder;
            // cached artwork must reach the Series-specific painter.
            (
                SERIES_IMAGE_COLS,
                SERIES_IMAGE_PLACEHOLDER_ROWS,
                ctx.image_loading,
            )
        } else {
            (0, 0, false)
        }
    };

    // Series metadata (year range + genre) and overview need the same
    // width-narrowing `text_dims` the movie hero uses, computed here
    // (before the image's actual on-screen row is known) only for the
    // overview's line-by-line wrap width.
    let title_rows = if show_title { 1 } else { 0 };
    let text_dims_pre = |r: u16| -> usize {
        inline_hero_text_width(
            area.width,
            img_actual_w,
            img_height,
            r.saturating_sub(area.y),
        ) as usize
    };

    let ser_meta = series_meta_line(item);
    // Row the overview starts on: title (0/1) + meta (0/1) + spacer (1,
    // unconditional -- see `unconditional_spacer_after_meta`).
    let overview_start_row = area.y + title_rows + (!ser_meta.is_empty()) as u16 + 1;
    let overview_lines = if !item.overview.is_empty() {
        let lines = wrap_overview_lines(&item.overview, |line_idx| {
            text_dims_pre(overview_start_row + line_idx as u16)
        });
        // Cap at available rows minus space for the season row and episode list --
        // shares SERIES_DETAIL_* constants with `series_inline_detail_rows`
        // so the reserved space and what's actually drawn stay in sync.
        // Narrow renders no season/episode block below, so nothing is
        // reserved for it there.
        let reserved_for_below = 0;
        let available_rows = (max_y
            .saturating_sub(overview_start_row)
            .saturating_sub(reserved_for_below)) as usize;
        lines.into_iter().take(available_rows).collect()
    } else {
        Vec::new()
    };
    let hero_lines: Vec<HeroLine> = overview_lines.into_iter().map(HeroLine::Plain).collect();
    let title = item.display_name();

    let hero_content = HeroContent {
        title: show_title.then_some(title.as_str()),
        meta_line: (!ser_meta.is_empty()).then_some(ser_meta.as_str()),
        meta_color: palette::TEXT_DETAIL_META,
        show_playing: false,
        unconditional_spacer_after_meta: true,
        lines: &hero_lines,
        image: (img_height > 0).then_some(HeroImage {
            actual_w: img_actual_w,
            height: img_height,
        }),
    };
    let result =
        crate::app::render::components::hero::paint_hero_content(f, area, &hero_content, focused);
    // ── Render series image last so it layers over text ───────────────
    if let Some(img_rect) = result.img_rect {
        if img_is_placeholder {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::BORDER_UNFOCUSED)),
                img_rect,
            );
        }
    }
    result.img_rect.map(|image_area| HomeImagePaint::Series {
        area: image_area,
        item: Box::new(item.clone()),
        show_placeholder: img_is_placeholder || img_loading,
        image_types: &["Primary"],
    })
}
