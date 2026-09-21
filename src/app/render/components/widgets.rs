use super::chrome::thin_vertical_thumb;
use crate::app::components::media_list::queue_row_background;
#[cfg(test)]
use crate::app::TabSelection;
use crate::app::{palette, App};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

// The main UI re-renders frequently while scrolling; prefer a cheaper filter in
// these hot paths to reduce terminal image preparation stalls.
pub(in crate::app) const RENDER_FILTER: ratatui_image::FilterType =
    ratatui_image::FilterType::Triangle;

// Configured music albums need the image worker's child-audio lookup; their
// album containers do not reliably expose usable Primary images.
pub(in crate::app) const MUSIC_ALBUM_IMAGE_TYPES: &[&str] = &["AudioChild"];

/// Columns of empty space between the left and right panels.
pub(in crate::app) const COLUMN_GAP: u16 = 0;

/// Left-edge padding applied once to every tab's content area
/// (Home, library lists, music groups, albums, series, home-video, feed
/// groups) plus the music-group pills row, so all tabs share a consistent
/// gutter. Applied at the single dispatch chokepoint in the main render
/// fn; individual tab renderers add only their own content-level gutters
/// (marker columns, banner indents) relative to this padded edge.
///
/// Detail surfaces that need additional internal alignment can add their own
/// indentation relative to this padded edge.
pub(in crate::app) const TAB_LEFT_PAD: u16 = 2;

/// Rows of top padding between the right panel's content and whatever sits
/// above it (tab bar, or the top of the terminal in mini view): previously
/// supplied incidentally by the wide playback strip's reserved band, which
/// now mounts only in LibraryOnly and no longer covers Both.
pub(in crate::app) const RIGHT_PANEL_TOP_PAD: u16 = 1;

pub(in crate::app) fn right_panel_content_area(area: Rect, left_collapsed: bool) -> Rect {
    let area = Rect {
        y: area.y + RIGHT_PANEL_TOP_PAD,
        height: area.height.saturating_sub(RIGHT_PANEL_TOP_PAD),
        ..area
    };
    if left_collapsed {
        Rect {
            x: area.x + 1,
            width: area.width.saturating_sub(2),
            ..area
        }
    } else {
        Rect {
            x: area.x + TAB_LEFT_PAD,
            width: area.width.saturating_sub(TAB_LEFT_PAD.saturating_mul(2)),
            ..area
        }
    }
}

/// The single scrollbar entry point: takes a role (`color`), never
/// hardcodes one. Positions the thumb at the area's own right edge if the
/// area already reaches the frame's right edge, otherwise just outside the
/// area (so a scrollbar never overlaps a panel's own content column).
pub(in crate::app) fn render_right_scrollbar(
    f: &mut Frame,
    area: Rect,
    max_offset: usize,
    offset: usize,
    color: Color,
) {
    let visible = area.height as usize;
    render_right_scrollbar_with_viewport(
        f,
        area,
        max_offset.saturating_add(visible),
        visible,
        offset,
        color,
    );
}

pub(in crate::app) fn render_right_scrollbar_with_viewport(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
    color: Color,
) {
    let x = if area.right() < f.area().right() {
        area.right()
    } else {
        area.x + area.width.saturating_sub(1)
    };
    render_scrollbar_with_viewport_at(
        f,
        area,
        content_length,
        viewport_content_length,
        offset,
        x,
        thin_vertical_thumb(GlyphSet::minimal()),
        color,
    );
}

pub(in crate::app) fn render_scrollbar_with_viewport_at(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
    x: u16,
    glyph_set: GlyphSet,
    scrollbar_color: Color,
) {
    if area.height == 0 || viewport_content_length == 0 || content_length <= viewport_content_length
    {
        return;
    }
    let max_offset = content_length.saturating_sub(viewport_content_length);
    let scrollbar = ScrollBar::vertical(ScrollLengths {
        content_len: content_length,
        viewport_len: viewport_content_length,
    })
    .offset(offset.min(max_offset))
    .glyph_set(glyph_set)
    .track_style(Style::default().fg(scrollbar_color))
    .thumb_style(Style::default().fg(scrollbar_color));
    f.render_widget(
        &scrollbar,
        Rect {
            x,
            width: 1,
            ..area
        },
    );
}

/// The QueuePanel's recessed content box inside its QueueColumn placement:
/// two columns of horizontal padding, one row of vertical padding.
pub(in crate::app) fn queue_panel_inset(area: Rect) -> Rect {
    Rect {
        x: area.x + 2,
        y: area.y + 1,
        width: area.width.saturating_sub(4),
        height: area.height.saturating_sub(2),
    }
}

/// The one-row block separator the Wide Hero's Main content box uses between
/// its overview text and its cast and crew table, and that the Workspace and
/// Queue panel headers reuse between their title and their rows: `▁` block
/// characters spanning `area`'s full width in the sage-green separator role.
pub(in crate::app) fn render_block_separator(f: &mut Frame, area: Rect) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new("\u{2581}".repeat(area.width as usize))
            .style(Style::default().fg(palette::HERO_OVERVIEW_SEPARATOR)),
        Rect { height: 1, ..area },
    );
}

/// Fill `area` with `surface`'s resolved background, clearing it first.
pub(in crate::app) fn fill_surface(
    f: &mut Frame,
    area: Rect,
    surface: palette::Surface,
    focused: bool,
) {
    let bg = palette::surface_colors(surface, focused).fill;
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(Style::default().bg(bg)), area);
}

pub(in crate::app) fn render_queue_panel_frame(f: &mut Frame, area: Rect, focused: bool) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    // The Queue panel owns a recessed box inside the Queue column surface.
    // Paint the complete parent placement first (which covers the QueueColumn
    // footer band around the status bar), then the recessed box, which ends
    // above the footer band. The outer fill already clears the inset rect, so
    // only paint its background.
    fill_surface(f, area, palette::Surface::QueueColumn, focused);
    let box_area = crate::app::render::arrangements::queue::queue_list_box(area);
    let inner = queue_row_background(focused);
    f.render_widget(Block::default().style(Style::default().bg(inner)), box_area);

    area
}

/// Style for a pill-selector choice: soft-white text while hovered (chip
/// background unchanged), muted text while resting, and the dominant
/// selected treatment. This is the canonical appearance for every
/// interactive pill selector (Home sections, feed groups, music groups,
/// letter filters, and series seasons).
fn selector_pill_fg(selected: bool, hovered: bool) -> Color {
    if selected {
        palette::PILL_SELECTED_FG
    } else if hovered {
        palette::TEXT_EMPHASIS
    } else {
        palette::TEXT_MUTED
    }
}

/// The joined pill shell's display width: `◢ label[•] ◤` = label width plus
/// the optional marker, chip's inner pads, and edge glyphs it actually paints.
fn pill_shell_width(label: &str, marked: bool, inner_pad: usize, leading: bool) -> usize {
    label.width() + usize::from(marked) + inner_pad * 2 + usize::from(leading) + 1
}

/// Push one joined pill shell (`◢ label[•] ◤`) as spans. The edge glyphs take
/// the chip's own surface as their foreground and either the row's or the
/// neighbouring chip's surface behind them. `inner_pad` is the blank columns
/// between the label and each edge glyph (0 when the slanted border alone is
/// the separation). `leading` is `None` for a chip that continues an
/// already-drawn seam: its leading column is dropped rather than painted flat,
/// which would otherwise read as a blank column of the chip's own fill (shared
/// by the interactive pill bar and the display-only hint bar so their shell
/// cannot drift apart).
fn push_pill_shell(
    spans: &mut Vec<Span>,
    label: &str,
    marked: bool,
    fg: Color,
    fill: Color,
    inner_pad: usize,
    leading: Option<Color>,
    trailing_bg: Color,
) {
    if let Some(leading_bg) = leading {
        spans.push(Span::styled("◢", Style::default().fg(fill).bg(leading_bg)));
    }
    let pad = " ".repeat(inner_pad);
    spans.push(Span::styled(
        format!("{pad}{label}"),
        Style::default().fg(fg).bg(fill),
    ));
    if marked {
        // The bullet is a content-state marker, so its semantic colour is
        // Iris (not the selected chip fill); ACCENT_ACTIVE is the existing
        // Iris role until the generated palette-role catalogue can accept a
        // dedicated marker role.
        spans.push(Span::styled(
            "•",
            Style::default().fg(palette::ACCENT_ACTIVE).bg(fill),
        ));
    }
    spans.push(Span::styled(
        " ".repeat(inner_pad),
        Style::default().bg(fill),
    ));
    spans.push(Span::styled("◤", Style::default().fg(fill).bg(trailing_bg)));
}

/// A horizontally-scrolling row of selector pills, shared by every
/// pill selector (Home sections, feed groups, music groups, letter
/// filters, and series seasons) so their appearance,
/// scroll/overflow/selection behavior can't drift apart. Callers
/// pre-truncate `labels`, supply the parallel `ids` recorded as click
/// targets, mark which position is `selected_pos`, and may pass an
/// optional leading `prefix` inset (rendered without the pill shell; it
/// does not alter the pill visual).
pub(in crate::app) struct PillBar<'a> {
    pub labels: &'a [String],
    /// Parallel semantic marker flags; an empty slice leaves every pill unmarked.
    pub markers: &'a [bool],
    pub ids: &'a [usize],
    pub selected_pos: usize,
    pub hovered: Option<usize>,
    pub prefix: Option<&'a str>,
    /// The row's retained overflow window (its last painted one). `Default`
    /// on first paint or after a layout change: the window centers on the
    /// selection.
    pub window: PillBarWindow,
}

/// One pill row's retained overflow window: the painted window's first pill
/// index, kept by the owning component across frames like its hitboxes
/// (ADR 0024 retained paint-local geometry). While the selected pill stays
/// inside the retained window the window does not move — a pointer
/// selection of an already-painted pill never slides the bar under the
/// cursor — and a selection outside it scrolls the fewest pills that
/// reveal it, landing on the leading edge moving left and minimally past
/// the trailing edge moving right. Only first paint (no retained window),
/// an out-of-range retention, and a row that stopped overflowing center on
/// the selection.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::app) struct PillBarWindow {
    pub(in crate::app) start: Option<usize>,
}

/// Renders `bar` into `area`, painting the canonical pill-selector row
/// background, drawing joined angled pills with the selected choice kept on
/// screen (with `‹`/`›` chevrons when the pills overflow), and returning the
/// on-screen pill hitboxes as `(rect, id)` pairs for `layout.selector_tabs`.
/// This is the sole renderer for interactive pill selectors; callers do not
/// select appearance variants.
pub(in crate::app) fn render_pill_bar(
    f: &mut Frame,
    area: Rect,
    bar: PillBar,
) -> (Vec<(Rect, usize)>, PillBarWindow) {
    // `ids` runs parallel to `labels`; a mismatch would panic on the slice
    // below, so assert the contract up front rather than fail cryptically.
    debug_assert_eq!(
        bar.labels.len(),
        bar.ids.len(),
        "render_pill_bar: labels and ids must be parallel"
    );
    debug_assert!(
        bar.markers.is_empty() || bar.markers.len() == bar.labels.len(),
        "render_pill_bar: markers must be empty or parallel to labels"
    );
    let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();
    if area.width == 0 || area.height == 0 {
        return (selector_tabs, bar.window);
    }
    let area = Rect { height: 1, ..area };
    // The row surface is part of the canonical shell, painted even with no
    // pills to show (task 12.2): the row's place stays reserved (its own
    // doc comment above), so it must still repaint its own background
    // rather than leave whatever was underneath before this panel owned the
    // placement. `Clear` blanks every cell's symbol first -- a bare
    // `Block::style` only recolors a cell, it never overwrites a stale
    // glyph.
    f.render_widget(ratatui::widgets::Clear, area);
    f.render_widget(
        Block::default().style(
            Style::default().bg(palette::surface_colors(palette::Surface::PillRow, false).fill),
        ),
        area,
    );
    if bar.labels.is_empty() {
        // Nothing painted: the retained window passes through unchanged so
        // an empty-frame repaint (or an active Inline Search box in the
        // row's rect) does not drop the bar's sticky position.
        return (selector_tabs, bar.window);
    }
    let n = bar.labels.len();
    let bar_w = area.width as usize;
    let prefix_w = bar.prefix.map(|p| p.width()).unwrap_or(0);
    // Display width of each joined pill is "◢ label[•] ◤" = label width +
    // optional marker width + inner padding (2) + edge glyphs (2).
    let pill_widths: Vec<usize> = bar
        .labels
        .iter()
        .enumerate()
        .map(|(idx, l)| {
            pill_shell_width(l, bar.markers.get(idx).copied().unwrap_or(false), 1, true)
        })
        .collect();

    // Greedy: how many pills fit starting at `start` within `avail` columns.
    let count_fitting = |start: usize, avail: usize| -> usize {
        let mut used = 0usize;
        let mut count = 0usize;
        for width in pill_widths.iter().skip(start) {
            if used + *width > avail {
                break;
            }
            used += *width;
            count += 1;
        }
        count
    };

    // Only scroll when the pills overflow: when everything fits, paint
    // all of them from zero so moving selection never pushes visible
    // pills out. When it overflows, the window is sticky (policy on
    // `PillBarWindow`). Starting at zero always put the selection at the
    // trailing edge, so moving backward looked as though the final
    // visible pill stayed focused.
    // ponytail: O(n²) over a short selector row; use a sliding window only if
    // selector counts become large enough to measure.
    let total_w: usize = prefix_w + pill_widths.iter().sum::<usize>();
    let (scroll_start, scroll_end, has_left, has_right) = if total_w <= bar_w {
        (0, n, false, false)
    } else {
        // Columns a window starting at `start` may use: the "‹ " chevron is
        // reserved whenever the window starts past the first pill, and " ›"
        // is always reserved while the row overflows.
        let avail_from = |start: usize| {
            bar_w
                .saturating_sub(prefix_w)
                .saturating_sub(if start > 0 { 2 } else { 0 }) // "‹ "
                .saturating_sub(2) // reserve for " ›"
        };
        let window_end = |start: usize| (start + count_fitting(start, avail_from(start))).min(n);
        // Sticky window: keep the retained start (already painted) or find
        // the fewest-pills slide that reveals the selection. Each branch
        // resolves `(start, end)` together so `window_end` isn't recomputed
        // for the same `start` twice. A position past the end (no active
        // pill) anchors at zero, exactly as before the sticky window: every
        // painted pill renders unselected.
        let sticky = if bar.selected_pos < n {
            let selected = bar.selected_pos;
            match bar.window.start {
                Some(prev) if prev < n && selected >= prev => {
                    let end = window_end(prev);
                    if selected < end {
                        Some((prev, end))
                    } else {
                        // Minimal slide right: the first window past the
                        // retained start that reaches past the selection.
                        (prev + 1..=selected).find_map(|start| {
                            let end = window_end(start);
                            (selected < end).then_some((start, end))
                        })
                    }
                }
                Some(prev) if prev < n => (0..=selected).rev().find_map(|start| {
                    let end = window_end(start);
                    (selected < end).then_some((start, end))
                }),
                _ => None,
            }
        } else {
            None
        };
        let (scroll_start, scroll_end) = sticky.unwrap_or_else(|| {
            let start = (0..=bar.selected_pos.min(n - 1))
                .filter_map(|start| {
                    let end = window_end(start);
                    if end == start || bar.selected_pos >= end {
                        return None;
                    }
                    Some((
                        (bar.selected_pos - start).min(end - 1 - bar.selected_pos),
                        start,
                    ))
                })
                // Prefer the later window on a tie: with only two pills visible,
                // moving left must put the selection at the leading edge rather than
                // leaving it pinned to the trailing edge.
                .max_by_key(|(edge_distance, start)| (*edge_distance, *start))
                .map(|(_, start)| start)
                .unwrap_or(0);
            (start, window_end(start))
        });

        let has_left = scroll_start > 0;
        let has_right = scroll_end < n;
        (scroll_start, scroll_end, has_left, has_right)
    };
    let window = PillBarWindow {
        start: Some(scroll_start),
    };

    let mut spans: Vec<Span> = Vec::new();
    let mut x_cursor = area.x;
    if let Some(prefix) = bar.prefix {
        if prefix == "  " {
            spans.push(Span::styled(
                "  ",
                Style::default()
                    .fg(palette::STATUS_AVAILABLE)
                    .bg(palette::surface_colors(palette::Surface::PillRow, false).fill),
            ));
        } else {
            spans.push(Span::styled(
                prefix.to_string(),
                Style::default().fg(palette::TEXT_METADATA),
            ));
        }
        x_cursor += prefix_w as u16;
    }
    if has_left {
        let chunk = "\u{2039} ";
        spans.push(Span::styled(
            chunk,
            Style::default().fg(palette::PILL_OVERFLOW_FG),
        ));
        x_cursor += chunk.width() as u16;
    }
    for (offset, (label, &id)) in bar.labels[scroll_start..scroll_end]
        .iter()
        .zip(bar.ids[scroll_start..scroll_end].iter())
        .enumerate()
    {
        let abs_idx = scroll_start + offset;
        let selected = abs_idx == bar.selected_pos;
        let is_last_pill = abs_idx + 1 == n;
        let hovered = bar.hovered == Some(abs_idx);
        let marked = bar.markers.get(abs_idx).copied().unwrap_or(false);
        let chip = if selected {
            palette::Surface::PillChipSelected
        } else {
            palette::Surface::PillChip
        };
        let fill = palette::surface_colors(chip, selected).fill;
        let pill_w = pill_shell_width(label, marked, 1, true) as u16;
        selector_tabs.push((
            Rect {
                x: x_cursor,
                y: area.y,
                width: pill_w,
                height: 1,
            },
            id,
        ));
        let leading_bg = palette::surface_colors(
            if abs_idx == 0 {
                palette::Surface::PillRow
            } else {
                palette::Surface::PillChip
            },
            false,
        )
        .fill;
        let trailing_bg = palette::surface_colors(
            if is_last_pill {
                palette::Surface::PillRow
            } else {
                palette::Surface::PillChip
            },
            false,
        )
        .fill;
        push_pill_shell(
            &mut spans,
            label,
            marked,
            selector_pill_fg(selected, hovered),
            fill,
            1,
            Some(leading_bg),
            trailing_bg,
        );
        x_cursor += pill_w;
    }
    if has_right {
        let chunk = " \u{203a}";
        spans.push(Span::styled(
            chunk,
            Style::default().fg(palette::PILL_OVERFLOW_FG),
        ));
        x_cursor += chunk.width() as u16;
    }

    // Clear the rest of the row with the canonical row background so the
    // surface is continuous across the panel.
    let used_w = x_cursor.saturating_sub(area.x) as usize;
    let remaining = bar_w.saturating_sub(used_w);
    if remaining > 0 {
        spans.push(Span::styled(
            " ".repeat(remaining),
            Style::default().bg(palette::surface_colors(palette::Surface::PillRow, false).fill),
        ));
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
    (selector_tabs, window)
}

/// Paints the display-only hint row used by the Library Hero overlay: the
/// canonical pill bar's row surface and joined chip shell (`◢ label[•] ◤`), with
/// every chip filled from `palette::HINT_PILL_FILLS` in rotation (foam,
/// yellow, orange, repeating) and soft-white text over the fill. Nothing here
/// is interactive — no hitboxes, no sticky window, no chevrons, no selection —
/// so a hint never depends on cursor, focus, or pointer state. The chip shell
/// is shared with [`render_pill_bar`] so the two cannot drift apart.
pub(in crate::app) fn render_hint_pill_bar(f: &mut Frame, area: Rect, hints: &[&str]) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let area = Rect { height: 1, ..area };
    let row_bg = palette::surface_colors(palette::Surface::PillRow, false).fill;
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(Style::default().bg(row_bg)), area);
    if hints.is_empty() {
        return;
    }
    let fill_of = |idx: usize| palette::HINT_PILL_FILLS[idx % palette::HINT_PILL_FILLS.len()];
    let widths: Vec<usize> = hints
        .iter()
        .enumerate()
        .map(|(idx, hint)| pill_shell_width(hint, false, 0, idx == 0))
        .collect();
    // The chips are centered on the row: the group is a fixed set of hints, not
    // a scrollable selector, so it has no reason to hug the left edge. When the
    // group overflows the row it starts at the edge and clips.
    let total: usize = widths.iter().sum();
    let pad_left = (area.width as usize).saturating_sub(total) / 2;
    let mut spans: Vec<Span> = Vec::new();
    if pad_left > 0 {
        spans.push(Span::styled(
            " ".repeat(pad_left),
            Style::default().bg(row_bg),
        ));
    }
    for (idx, hint) in hints.iter().enumerate() {
        let fill = fill_of(idx);
        // Chip boundaries are ONE diagonal, not two: a chip's trailing edge
        // paints its own fill above-left with the next chip's fill behind it,
        // and the next chip emits no leading glyph at all, so the two chips sit
        // directly side by side. Painting a flat leading glyph instead would
        // read as a blank column of the chip's own fill, i.e. a second space
        // before the label. Only the row's first chip keeps an angled end (and
        // every chip's trailing end) against the row surface.
        let leading = (idx == 0).then_some(row_bg);
        let trailing_bg = if idx + 1 == hints.len() {
            row_bg
        } else {
            fill_of(idx + 1)
        };
        push_pill_shell(
            &mut spans,
            hint,
            false,
            palette::TEXT_ON_ACCENT,
            fill,
            0,
            leading,
            trailing_bg,
        );
    }
    // Clear the rest of the row with the row background so the hint bar reads
    // as the same surface as a pill bar.
    let remaining = (area.width as usize).saturating_sub(pad_left + total);
    if remaining > 0 {
        spans.push(Span::styled(
            " ".repeat(remaining),
            Style::default().bg(row_bg),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Draws a shared empty/loading placeholder message (MUTED) at `area`.
/// Callers pass the exact text (`" (empty)"`, `" Loading…"`, or a
/// context-specific string like `"Indexing music library..."`) so the
/// wording stays local, but the placeholder styling is defined once.
pub(in crate::app) fn render_placeholder(f: &mut Frame, area: Rect, msg: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            msg.to_string(),
            Style::default().fg(palette::TEXT_MUTED),
        )),
        area,
    );
}

impl App {
    #[cfg(test)]
    pub(in crate::app) fn reserve_library_area(
        &mut self,
        _f: &mut Frame,
        area: Rect,
        layout: &mut Rect,
        _cursor_scroll: Option<(usize, usize)>,
    ) {
        // If a music-group library's nav_stack was truncated to just the group
        // level (e.g., stale breadcrumb click), immediately re-push the album level.
        // (Emby-only; done inside the Emby match arm below.)
        // Exhaustive destination dispatch: each Service renders only its own
        // view; there is no default-to-Emby branch. The selected destination
        // was already normalized to a live index by `render_main`.
        match self.tab {
            TabSelection::Home => {
                // Home content is painted by the mounted `HomeComponent`.
                // The legacy frame only reserves the full Home destination
                // area here — it paints no Home rows, pills, hero, or image
                // (task 5.3d, Home legacy underpaint removal); nothing reads
                // the reservation back (task 12.4), so it is a no-op.
            }
            TabSelection::Feeds => {
                // Feeds is painted by its embedded owner inside the mounted
                // `LibraryPanel` (task 7.3); the legacy base frame reserves
                // nothing and paints no feed entry, selector pill or filter
                // pill.
            }
            TabSelection::AudiobookshelfLibrary(_) => {
                // Audiobookshelf destinations are painted by the embedded
                // LibraryPanel owner; the legacy frame only reserves the area.
            }
            TabSelection::EmbyLibrary(lib_idx) => {
                if self.is_feed_home_video_group_view(lib_idx) {
                    // EmbyLibraryContent owns feed group presentation at every
                    // width; publish only the full browser area.
                    *layout = area;
                    return;
                }
                {
                    // Wide TV's mounted `LibraryPanel` paints the whole Wide
                    // hero workspace through its embedded TV content owner
                    // panel's shared skeleton (task 8.2); the legacy base
                    // frame reserves only the destination area here and
                    // paints no workspace.
                    //
                    // The active Emby/TV content owner owns the browse body at every width;
                    // reserve only the destination area here.
                    *layout = area;
                }
            }
        }
    }
}
