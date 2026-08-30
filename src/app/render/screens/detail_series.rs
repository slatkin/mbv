use crate::app::render::components::detail_series_view::{
    series_meta_line, SERIES_DETAIL_TRAILING_BLANK_ROWS, SERIES_IMAGE_ROWS,
};
use crate::app::render::components::hero::wrap_overview_lines;
use crate::app::App;

/// Rows reserved below the overview text in the Series inline detail pane
/// for the divider/season-pills row and the (roughly estimated) episode
/// list, shared by `series_inline_detail_rows` (which reserves this many
/// filler rows in the list) and `render_series_inline_detail` (which caps
/// how many overview lines it draws so this many rows remain below them) --
/// keeping both in sync prevents the overview from eating into space the
/// layout pass assumed was reserved for the divider/pills/episodes.
const SERIES_DETAIL_OVERVIEW_MAX_LINES: usize = 4;
/// Row budget for the inline series detail block's *content* (title
/// through the trailing blank row) -- the caller adds its own block
/// framing (border/padding rows) on top, mirroring the movie hero's
/// `hero_height_for_width` + `HERO_BLOCK_EXTRA_ROWS` split. `show_title`
/// reserves the yellow title row used in two-column lists (see
/// `render_series_inline_detail`). Narrow never shows the season/episode
/// block (see `render_series_inline_detail`), so no space is reserved
/// for it here.
pub(in crate::app::render) fn series_inline_detail_rows(
    app: &App,
    item: &mbv_core::api::EmbyItem,
    panel_width: u16,
    show_title: bool,
) -> usize {
    let inner_w = (panel_width as usize).saturating_sub(2);

    let mut rows = 0usize;

    if show_title {
        rows += 1;
    }

    // Series metadata row (year range + genre)
    if !series_meta_line(item).is_empty() {
        rows += 1;
    }

    // Blank spacer
    rows += 1;

    // Overview (word-wrapped, capped to leave room for pills + episodes)
    if !item.overview.is_empty() {
        let lines = wrap_overview_lines(&item.overview, |_| inner_w);
        // Cap at SERIES_DETAIL_OVERVIEW_MAX_LINES rows to leave room for pills + episodes
        rows += lines.len().min(SERIES_DETAIL_OVERVIEW_MAX_LINES);
        if !lines.is_empty() {
            rows += 1; // spacer after overview
        }
    }

    // Keep the block tall enough for the image and its bottom gutter.
    let img_height = if app.images_enabled() {
        SERIES_IMAGE_ROWS as usize
    } else {
        0
    };
    let image_end_row = img_height.saturating_add((img_height > 0) as usize);
    rows = rows.max(image_end_row);

    // Blank spacer below the episode list
    rows += SERIES_DETAIL_TRAILING_BLANK_ROWS;

    rows
}
