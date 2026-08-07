use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;

/// Standard inset for every selected detail block.
pub(super) const SELECTED_BLOCK_SIDE_PADDING: u16 = 2;

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
    /// One display row: the item indices occupying it, in column order. In
    /// one-column mode every such row carries exactly one index, so both
    /// modes share a single rendering path with no `cols == 1` branch.
    Item(Vec<usize>),
}

/// Shared inputs to the per-kind row-rendering bodies of `render_power_list`
/// (`render_power_letter_grouped_rows`, `render_power_plain_rows`): the
/// prelude values both kinds' bodies read, factored out so each callee takes
/// one struct instead of the same six-plus positional arguments.
pub(super) struct ListRenderCtx<'a> {
    /// The list's own area: `render_power_list` splits `content_area` into
    /// this (the top slice, above the hero) and a separate `hero_area` (the
    /// bottom slice) -- the row renderer only ever sees `list_area` and has
    /// no notion of the hero at all.
    pub(super) content_area: Rect,
    pub(super) items: &'a [mbv_core::api::MediaItem],
    pub(super) cursor: usize,
    pub(super) stored_scroll: usize,
    /// Column count for this frame's list pane width (1 or 2).
    pub(super) cols: usize,
    pub(super) focused: bool,
}

/// Builds the title (+ optional duration) spans for one list row, shared by
/// both the letter-grouped and plain-list rendering branches (identical
/// styling logic, only how `title`/`dur_str`/`avail` are computed differs
/// between the two call sites). Every cell starts with a 1-column leading
/// separator; for the selected cell that separator is a `▌` mark in
/// `palette::AQUA` and the title gets a `##` prefix, so the selected cell is
/// identifiable without a colored background -- the hero (not the row) now
/// owns `MEDIA_SELECTED_BG`, since the two are no longer adjacent.
pub(super) fn build_list_row_spans(
    title: String,
    dur_str: String,
    selected: bool,
    focused: bool,
    fg: Color,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = if selected {
        let marker_style = Style::default().fg(palette::AQUA);
        let title_style = if focused {
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };
        vec![
            Span::styled("\u{258c}", marker_style),
            Span::styled(format!("##{title}"), title_style),
        ]
    } else {
        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
    };
    if !dur_str.is_empty() {
        spans.push(Span::styled(dur_str, Style::default().fg(palette::MUTED)));
    }
    spans
}

/// Builds the padded spans for one item rendered into a `cell_width`-wide
/// cell: the existing marker/title/metadata/truncation logic operating
/// against the narrower cell width. Returns the cell's spans plus trailing
/// padding so the next cell starts at its own x offset; `pad_to` is the
/// total width to fill (cell width, plus the inter-column gap for every
/// cell except the last in its row).
pub(super) fn item_cell_spans(
    title: String,
    dur_str: String,
    selected: bool,
    focused: bool,
    fg: Color,
    pad_to: usize,
) -> Vec<Span<'static>> {
    let mut spans = build_list_row_spans(title, dur_str, selected, focused, fg);
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let pad = pad_to.saturating_sub(used);
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans
}
