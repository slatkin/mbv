use crate::app::App;
use unicode_width::UnicodeWidthStr;

/// Rows reserved below the overview text in the Series inline detail pane
/// for the divider/season-pills row and the (roughly estimated) episode
/// list, shared by `series_inline_detail_rows` (which reserves this many
/// filler rows in the list) and `render_series_inline_detail` (which caps
/// how many overview lines it draws so this many rows remain below them) --
/// keeping both in sync prevents the overview from eating into space the
/// layout pass assumed was reserved for the divider/pills/episodes.
pub(super) const SERIES_DETAIL_DIVIDER_ROWS: usize = 1; // season summary/pills row
pub(super) const SERIES_DETAIL_EPISODE_ROWS_ESTIMATE: usize = 8; // estimated visible episode rows
const SERIES_DETAIL_OVERVIEW_MAX_LINES: usize = 4;
/// Blank row below the episode list. Shared by `series_inline_detail_rows`
/// and `render_series_inline_detail` for the same reason as the constants
/// above -- one row budget, two call sites.
pub(super) const SERIES_DETAIL_TRAILING_BLANK_ROWS: usize = 1;
pub(super) const SERIES_IMAGE_COLS: u16 = 18;
pub(super) const SERIES_IMAGE_ROWS: u16 = 12;
pub(super) const SERIES_IMAGE_PLACEHOLDER_ROWS: u16 = 10;

/// Builds the "YYYY-YYYY  GENRE" (or any subset present) metadata line for a
/// Series item, shared by `series_inline_detail_rows` (row-count estimate)
/// and `render_series_inline_detail` (actual render) so the two can't drift.
pub(super) fn series_meta_line(item: &mbv_core::api::MediaItem) -> String {
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

/// Word-wraps `text` into lines, asking `width_for_line` for the width
/// available to the line currently being built (given the number of lines
/// already completed) -- callers that need per-row width (e.g. narrowing
/// around an inline image) can vary the result per call, while callers that
/// just want a flat-width estimate can ignore the argument. Shared by
/// `series_inline_detail_rows` and `render_series_inline_detail` so the
/// wrapping logic can't drift between the two.
pub(super) fn wrap_overview_lines(
    text: &str,
    mut width_for_line: impl FnMut(usize) -> usize,
) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    let mut cur_line = String::new();
    for word in text.split_whitespace() {
        let word_w = word.width();
        let avail = width_for_line(lines.len());
        if cur_line.is_empty() {
            cur_line.push_str(word);
        } else if cur_line.width() + 1 + word_w <= avail {
            cur_line.push(' ');
            cur_line.push_str(word);
        } else {
            lines.push(std::mem::take(&mut cur_line));
            cur_line.push_str(word);
        }
    }
    if !cur_line.is_empty() {
        lines.push(cur_line);
    }
    lines
}

impl App {
    pub(super) fn series_selection_state(
        &self,
        lib_idx: usize,
        series_id: &str,
    ) -> (bool, Option<usize>) {
        let in_selection = self.libs[lib_idx].series_selection.is_some();
        let episode_count = self.libs[lib_idx]
            .series_selection
            .and_then(|_| self.series_detail_cache.get(series_id))
            .and_then(|detail| {
                detail
                    .seasons
                    .get(self.libs[lib_idx].series_season_cursor)
                    .and_then(|season| detail.episodes.get(&season.id))
            })
            .map(Vec::len);
        (in_selection, episode_count)
    }

    /// Row budget for the inline series detail block's *content* (title
    /// through the trailing blank row) -- the caller adds its own block
    /// framing (border/padding rows) on top, mirroring the movie hero's
    /// `hero_height_for_width` + `HERO_BLOCK_EXTRA_ROWS` split. `show_title`
    /// reserves the yellow title row used in two-column lists (see
    /// `render_series_inline_detail`).
    pub(super) fn series_inline_detail_rows(
        &mut self,
        item: &mbv_core::api::MediaItem,
        panel_width: u16,
        show_title: bool,
        in_selection: bool,
        episode_count: Option<usize>,
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

        // Season summary row and episodes when selection mode is active.
        rows += SERIES_DETAIL_DIVIDER_ROWS;
        if in_selection {
            rows += episode_count.unwrap_or(SERIES_DETAIL_EPISODE_ROWS_ESTIMATE);
        }

        // Keep the block tall enough for the image (which starts below the
        // title row), if it extends below the text and episode content.
        let img_height = if self.images_enabled() {
            SERIES_IMAGE_ROWS as usize
        } else {
            0
        };
        let image_end_row = img_height + show_title as usize;
        rows = rows.max(image_end_row);

        // Blank spacer below the episode list
        rows += SERIES_DETAIL_TRAILING_BLANK_ROWS;

        rows
    }
}
