use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::Frame;

/// Rows the compact movie banner occupies inline in the library list. The
/// selected movie row + the banner's own content (meta/overview/poster,
/// rendered by `render_power_compact_detail`, directly below the selected row)
/// are wrapped in a colored block — `palette::MEDIA_SELECTED_BG` when focused,
/// `palette::PLAYBACK_PANEL_BG` when unfocused — a dark (#282828 / #151515)
/// background visually similar to the home tab's Keep Watching
/// list — instead of horizontal rules. The two
/// constants below reserve one row above the selected item (the block's top
/// padding, replacing the previous opening `─` rule) and one row after the
/// banner content (the block's bottom padding, replacing the previous closing
/// `─` rule), and `COMPACT_BANNER_INDENT` reserves that many columns of
/// external side padding on each side of the colored block (matched one-for-
/// one by `render_power_compact_detail`'s own internal padding, so the
/// visible side padding is `INDENT + 1` columns on each side).
pub(super) const COMPACT_BANNER_RULE_ROWS: usize = 1;
pub(super) const COMPACT_BANNER_GAP_ROWS: usize = 1;
/// Standard inset for every selected detail block.
pub(super) const SELECTED_BLOCK_SIDE_PADDING: u16 = 2;
/// External side padding for the selected movie block.
pub(super) const COMPACT_MOVIE_BANNER_INDENT: u16 = SELECTED_BLOCK_SIDE_PADDING;
/// External side padding for inline series detail.
pub(super) const COMPACT_BANNER_INDENT: u16 = 1;

/// Returns `palette::WHITE` when `focused`, `palette::SUBTLE` otherwise.
pub(super) fn focused_or_subtle(focused: bool) -> Color {
    if focused {
        palette::WHITE
    } else {
        palette::SUBTLE
    }
}

/// Returns `palette::YELLOW` when `focused`, `palette::MUTED` otherwise.
pub(super) fn focused_or_muted(focused: bool) -> Color {
    if focused {
        palette::YELLOW
    } else {
        palette::MUTED
    }
}

/// Returns `palette::AQUA` when `focused`, `palette::MUTED` otherwise.
pub(super) fn focused_aqua_or_muted(focused: bool) -> Color {
    if focused {
        palette::AQUA
    } else {
        palette::MUTED
    }
}

/// Returns `palette::SOFT_WHITE` when `focused`, `palette::MUTED` otherwise.
pub(super) fn focused_or_muted_soft_white(focused: bool) -> Color {
    if focused {
        palette::SOFT_WHITE
    } else {
        palette::MUTED
    }
}

pub(super) enum DisplayRow {
    Spacer,
    LetterHeader(String),
    Item(usize),
    BannerFiller,
    SeriesDetailFiller,
}

/// Shared inputs to the per-kind row-rendering bodies of `render_power_list`
/// (`render_power_letter_grouped_rows`, `render_power_plain_rows`): the
/// prelude values both kinds' bodies read, factored out so each callee takes
/// one struct instead of the same eight-plus positional arguments.
/// `area` and `content_area` differ only when a search input box has shifted
/// `content_area` down (see `render_power_list`'s prelude); both are read
/// independently by the bodies, so both are carried.
pub(super) struct ListRenderCtx<'a> {
    pub(super) area: Rect,
    pub(super) content_area: Rect,
    pub(super) items: &'a [mbv_core::api::MediaItem],
    pub(super) cursor: usize,
    pub(super) stored_scroll: usize,
    pub(super) banner_rows: usize,
    pub(super) banner_content_rows: usize,
    pub(super) series_detail_rows: usize,
    pub(super) focused: bool,
}

pub(super) fn push_selected_detail_fillers_before(
    rows: &mut Vec<DisplayRow>,
    item_idx: usize,
    cursor: usize,
    banner_rows: usize,
    series_detail_rows: usize,
) {
    if banner_rows > 0 && item_idx == cursor {
        rows.push(DisplayRow::BannerFiller);
        rows.push(DisplayRow::BannerFiller);
    }
    if series_detail_rows > 0 && item_idx == cursor {
        rows.push(DisplayRow::SeriesDetailFiller);
        rows.push(DisplayRow::SeriesDetailFiller);
    }
}

pub(super) fn push_selected_detail_fillers_after(
    rows: &mut Vec<DisplayRow>,
    item_idx: usize,
    cursor: usize,
    banner_rows: usize,
    series_detail_rows: usize,
) {
    if banner_rows > 0 && item_idx == cursor {
        for _ in 0..banner_rows.saturating_sub(2) {
            rows.push(DisplayRow::BannerFiller);
        }
        rows.push(DisplayRow::BannerFiller);
        rows.push(DisplayRow::BannerFiller);
    }
    if series_detail_rows > 0 && item_idx == cursor {
        for _ in 0..series_detail_rows {
            rows.push(DisplayRow::SeriesDetailFiller);
        }
    }
}

pub(super) fn selected_detail_lower_bound(
    display_cursor: usize,
    banner_rows: usize,
    series_detail_rows: usize,
    visible: usize,
) -> usize {
    let rows_below_cursor = banner_rows.max(series_detail_rows);
    (display_cursor + rows_below_cursor)
        .saturating_sub(visible.saturating_sub(1))
        .min(display_cursor)
}

/// Builds the title (+ optional duration) spans for one list row, shared by
/// both the letter-grouped and plain-list rendering branches (identical
/// styling logic, only how `title`/`dur_str`/`avail` are computed differs
/// between the two call sites).
pub(super) fn build_list_row_spans(
    title: String,
    dur_str: String,
    selected: bool,
    selected_has_banner: bool,
    is_series: bool,
    focused: bool,
    fg: Color,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = if selected {
        if selected_has_banner {
            // Colored-block look: 2-col leading pad inside the
            // MEDIA_SELECTED_BG block, no green `▌` gutter. Title is Emby
            // green (BOLD when focused) and the row omits the duration --
            // it lives in the banner's metadata row below.
            let title_style = if focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            vec![Span::raw("  "), Span::styled(title, title_style)]
        } else if is_series {
            // Series inline detail: title is yellow when selected.
            let title_style = if focused {
                Style::default()
                    .fg(palette::YELLOW)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::YELLOW)
            };
            vec![
                Span::raw(" ".repeat(SELECTED_BLOCK_SIDE_PADDING as usize)),
                Span::styled(title, title_style),
            ]
        } else {
            // Keep standard alignment without an inline banner.
            let title_style = if focused {
                Style::default()
                    .fg(palette::IRIS)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(fg)
            };
            vec![
                super::selection_marker(true),
                Span::styled(title, title_style),
            ]
        }
    } else {
        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
    };
    if !selected_has_banner && !dur_str.is_empty() {
        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
    }
    spans
}

/// Paints the series inline detail block's colored background, shared by
/// both the letter-grouped and plain-list rendering branches of
/// `render_power_list` (identical treatment, only how `display_cursor` /
/// `offset` / `visible` are computed differs between the two call sites).
/// The colored block starts at the spacer row above the selected item and runs
/// through the spacer row below the episode list; the SeriesDetailFiller top
/// border (▁) and the bottom border (▔, drawn inside `render_series_inline_detail`)
/// are left uncolored so they blend into the existing background.
pub(super) fn render_series_detail_background(
    f: &mut Frame,
    content_area: Rect,
    offset: usize,
    visible: usize,
    display_cursor: usize,
    series_detail_rows: usize,
    focused: bool,
) {
    if series_detail_rows == 0 {
        return;
    }
    let series_rule_top = display_cursor.saturating_sub(1);
    let series_rule_bottom = display_cursor + series_detail_rows.saturating_sub(1);
    let bg = if focused {
        palette::MEDIA_SELECTED_BG
    } else {
        palette::PLAYBACK_PANEL_BG
    };
    super::render_selected_block_background(
        f,
        content_area,
        offset,
        visible,
        series_rule_top,
        series_rule_bottom,
        bg,
    );
}
