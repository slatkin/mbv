use crate::app::palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

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
/// space. In single-column mode the title gets a `##` prefix; in
/// two-column mode (`cols > 1`) the selected cell instead carries a
/// `palette::PLAYBACK_PANEL_BG` background. Column selection markers
/// (`▌` / `▐`) are drawn separately by `draw_column_selection_markers`.
pub(super) fn build_list_row_spans(
    title: String,
    dur_str: String,
    selected: bool,
    focused: bool,
    fg: Color,
    cols: usize,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span> = if selected {
        let title_style = if focused {
            Style::default().fg(fg).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(fg)
        };
        if cols > 1 {
            let bg = palette::PLAYBACK_PANEL_BG;
            vec![
                Span::styled(" ", Style::default().bg(bg)),
                Span::styled(title, title_style.bg(bg)),
            ]
        } else {
            let marker_style = Style::default().fg(palette::AQUA);
            vec![
                Span::styled("\u{258c}", marker_style),
                Span::styled(format!("##{title}"), title_style),
            ]
        }
    } else {
        vec![Span::raw(" "), Span::styled(title, Style::default().fg(fg))]
    };
    if !dur_str.is_empty() {
        let dur_style = if selected && cols > 1 {
            Style::default().fg(palette::MUTED).bg(palette::PLAYBACK_PANEL_BG)
        } else {
            Style::default().fg(palette::MUTED)
        };
        spans.push(Span::styled(dur_str, dur_style));
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
    cols: usize,
) -> Vec<Span<'static>> {
    let mut spans = build_list_row_spans(title, dur_str, selected, focused, fg, cols);
    let used: usize = spans.iter().map(|s| s.width()).sum();
    let pad = pad_to.saturating_sub(used);
    if pad > 0 {
        spans.push(Span::raw(" ".repeat(pad)));
    }
    spans
}

/// Draws the column selection marker after the list has rendered.
/// In two-column mode, the selected cell's marker is drawn at the panel
/// edge: `▌` at the left edge for a left-column selection, `▐` at the
/// right edge for a right-column selection (symmetric).
pub(super) fn draw_column_selection_markers(
    f: &mut Frame,
    content_area: Rect,
    cursor: usize,
    cols: usize,
    item_rows: &[Vec<usize>],
) {
    if cols <= 1 {
        return;
    }
    let cursor_row = item_rows
        .iter()
        .position(|row| row.contains(&cursor));
    let Some(row_idx) = cursor_row else {
        return;
    };
    let col_in_row = item_rows[row_idx]
        .iter()
        .position(|&idx| idx == cursor)
        .unwrap_or(0);

    let marker_style = Style::default().fg(palette::AQUA);
    if col_in_row == 0 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("\u{258c}", marker_style))),
            Rect {
                x: content_area.x.saturating_sub(1),
                y: content_area.y + row_idx as u16,
                width: 1,
                height: 1,
            },
        );
    } else {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("\u{2590}", marker_style))),
            Rect {
                x: content_area.x + content_area.width,
                y: content_area.y + row_idx as u16,
                width: 1,
                height: 1,
            },
        );
    }
}
