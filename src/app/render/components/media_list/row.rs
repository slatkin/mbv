use crate::app::components::media_list::{
    MediaKind, MediaListRow, MediaListTrailing, MediaSemanticState,
};
use crate::app::palette;
use crate::app::render::components::marquee::marquee_spans;
use crate::app::ui_util::trunc_str;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::ListItem;
use unicode_width::UnicodeWidthStr;

/// One painted row of a media-list flow, shared by the Wide and Inline
/// Presentations. Semantic state drives the row
/// colour and, for active rows, an appended progress percentage; `primary`
/// is truncated with an ellipsis to fit; `duration` is a distinct
/// right-aligned green element ending at the panel text-flow content edge
/// (`inner_width` already excludes the scrollbar column). The now-playing
/// row paints its total duration like every other row — no throbber slot.
///
/// `selected_bg` is the selected-row bar fill. Every selected row on a
/// focused list paints it across the whole row, overriding its zebra stripe
/// and the two-column gutters; callers resolve it to the `SELECTED_ROW_BG`
/// role, and the surface table's owning-surface identity no longer changes the
/// row's appearance. Every span keeps the ordinary unselected foreground role,
/// so the bar introduces no bold title and no accent title colour. The
/// `gutter_accent` argument is retained for the Wide callers but no longer
/// changes painting.
///
/// `alternate_bg` is the row's position in the list's zebra alternation. It
/// paints every row type — selectable items, group headings and the blank
/// spacers — within the same text-flow range, so the stripe bands run unbroken
/// through a group boundary and the two-column gutters keep the parent
/// background.
///
/// Row geometry: the title text is indented 2 columns in — a 2-column quiet
/// indent — so the title lands at column 2 of the panel; the selected row's
/// background fills the whole row via `List`'s row-style fill and bleeds to
/// both panel edges.
pub(in crate::app) fn media_list_row<Target>(
    row: &MediaListRow<Target>,
    selected: bool,
    focused: bool,
    selected_bg: Color,
    alternate_bg: Option<Color>,
    _gutter_accent: bool,
    inner_width: usize,
    has_scrollbar: bool,
    mut marquee: Option<(&mut String, &mut std::time::Instant)>,
) -> ListItem<'static> {
    match row {
        MediaListRow::Spacer => ListItem::new(Line::from(stripe_spans(
            vec![Span::raw("  ")],
            alternate_bg,
            row_content_w(inner_width, has_scrollbar),
        ))),
        MediaListRow::Heading { text } => ListItem::new(Line::from(stripe_spans(
            vec![
                Span::raw("  "),
                Span::styled(
                    text.clone(),
                    Style::default()
                        .fg(palette::TEXT_METADATA)
                        .add_modifier(Modifier::BOLD),
                ),
            ],
            alternate_bg,
            row_content_w(inner_width, has_scrollbar),
        ))),
        MediaListRow::Item {
            primary,
            secondary,
            trailing,
            duration,
            kind,
            semantic_state,
            ..
        } => {
            // Canonical row geometry:
            // `[2-col indent][title…]  [FOAM trailing]  [gold duration]`
            // with the title at column 2 and a quiet gap before the right-aligned
            // duration.

            let (fg, progress, live_icon) = match semantic_state {
                // Now-playing rows lose the accent colour: the aqua play marker
                // before the title marks them instead, so the row text keeps
                // the ordinary role. Active rows retain resume progress inline
                // but do not receive the play marker. Both states append their
                // progress percentage to the trailing text in the same style.
                MediaSemanticState::Ordinary => (palette::TEXT_EMPHASIS, None, None),
                MediaSemanticState::Played => (palette::TEXT_MUTED, None, None),
                MediaSemanticState::Active { progress } => (
                    palette::TEXT_EMPHASIS,
                    (*progress).map(|value| format!("{}%", value.percent())),
                    None,
                ),
                MediaSemanticState::NowPlaying { progress } => (
                    palette::TEXT_EMPHASIS,
                    (*progress).map(|value| format!("{}%", value.percent())),
                    Some("▶ "),
                ),
            };
            const LEFT_INSET: usize = 2;
            const QUIET_GAP: usize = 2;
            // The left-aligned metadata pieces, in paint order: the trailing
            // slot's own role (a year is green, a progress badge FOAM), then
            // the active row's percentage as its own FOAM piece.
            let mut trailing_pieces: Vec<(String, Color)> = Vec::new();
            match trailing {
                Some(MediaListTrailing::Year(text)) if !text.is_empty() => {
                    trailing_pieces.push((text.clone(), palette::STATUS_AVAILABLE));
                }
                Some(MediaListTrailing::Progress(text)) if !text.is_empty() => {
                    trailing_pieces.push((text.clone(), palette::TEXT_METADATA));
                }
                _ => {}
            }
            if let Some(pct) = progress {
                trailing_pieces.push((pct, palette::TEXT_METADATA));
            }
            // `Collection` rows never show a duration, even if one is
            // projected — one enforcement point so parents can't re-diverge.
            let duration = duration
                .as_deref()
                .filter(|dur| !dur.is_empty())
                .filter(|_| !matches!(kind, MediaKind::Collection));

            let content_w = row_content_w(inner_width, has_scrollbar);
            let trailing_w: usize = trailing_pieces
                .iter()
                .map(|(text, _)| 1 + text.width())
                .sum();
            let slot_reserve = duration.map_or(0, |dur| QUIET_GAP + dur.width());
            let selected = selected && focused;
            // Every selected row paints the opaque bar edge to edge; the
            // `gutter_accent` callers no longer keep their background.
            let paint_selected = selected;
            let secondary_separator_reserve =
                usize::from(secondary.as_deref().is_some_and(|text| !text.is_empty()));
            let icon_reserve = live_icon.map_or(0, UnicodeWidthStr::width);
            let title_width = content_w.saturating_sub(
                LEFT_INSET + trailing_w + slot_reserve + secondary_separator_reserve + icon_reserve,
            );
            let title_color = fg;
            // Split rows (a secondary title after the primary text) paint the
            // now-playing two-tone palette: the primary text is the
            // container/context name in the playback-context gold role and
            // the secondary title (the item's own name) in the playback-title
            // aqua role, with one separating space. The mute follows the
            // title role, not the position: a played split row mutes the
            // item title while the context keeps gold; no other semantic
            // state moves the split-row palette. Single-part rows keep the
            // ordinary title role for their semantic state. (`parts` feeds
            // the marquee path, which only paints selected rows; unselected
            // split rows paint the same roles via the truncation branch
            // below.)
            let secondary_color = match semantic_state {
                MediaSemanticState::Played => palette::TEXT_MUTED,
                _ => palette::PLAYBACK_TITLE_FG,
            };
            let parts: Vec<(String, Color)> =
                match secondary.as_deref().filter(|sec| !sec.is_empty()) {
                    Some(sec) => vec![
                        (primary.clone(), palette::PLAYBACK_CONTEXT_FG),
                        (" ".into(), palette::PLAYBACK_CONTEXT_FG),
                        (sec.to_owned(), secondary_color),
                    ],
                    None => vec![(primary.clone(), title_color)],
                };
            let parts_width: usize = parts.iter().map(|(text, _)| text.width()).sum();
            let title_spans = marquee
                .take()
                .filter(|_| selected && parts_width > title_width)
                .map(|(text, started_at)| {
                    marquee_spans(primary, &parts, title_width, text, started_at)
                })
                .unwrap_or_else(|| {
                    // Truncation priority: the secondary title keeps its width
                    // (up to the whole slot) and the primary title takes the
                    // rest, so a long series name ellipsises before the
                    // episode title is dropped.
                    let sec = secondary.as_deref().filter(|sec| !sec.is_empty());
                    let sec_w = sec.map_or(0, |text| text.width().min(title_width));
                    // A split row's primary text paints the playback-context
                    // gold role even when the secondary title is truncated
                    // away; a single-part row keeps the ordinary title role.
                    let primary_color = if sec.is_some() {
                        palette::PLAYBACK_CONTEXT_FG
                    } else {
                        title_color
                    };
                    let mut spans = vec![Span::styled(
                        trunc_str(primary, title_width - sec_w),
                        Style::default().fg(primary_color),
                    )];
                    if let Some(sec) = sec.filter(|_| sec_w > 0) {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            trunc_str(sec, sec_w),
                            Style::default().fg(secondary_color),
                        ));
                    }
                    spans
                });
            let mut spans = vec![Span::raw("  ")];
            if let Some(icon) = live_icon {
                spans.push(Span::styled(icon, Style::default().fg(palette::ACCENT)));
            }
            spans.extend(title_spans);
            if !trailing_pieces.is_empty() {
                for (text, color) in &trailing_pieces {
                    spans.push(Span::raw(" "));
                    spans.push(Span::styled(text.clone(), Style::default().fg(*color)));
                }
            }
            if let Some(dur) = duration {
                let used: usize = spans.iter().map(|span| span.content.width()).sum();
                let pad = content_w.saturating_sub(used + dur.width());
                spans.push(Span::raw(" ".repeat(pad)));
                spans.push(Span::styled(
                    dur.to_owned(),
                    Style::default().fg(palette::DURATION),
                ));
            }
            // Pad the selected row's spans out to the full row width (up to
            // the scrollbar column) so the highlighted background bar spans
            // the whole panel regardless of whether a duration string is
            // present — never just the width of the row text.
            if paint_selected {
                let used: usize = spans.iter().map(|span| span.content.width()).sum();
                spans.push(Span::raw(" ".repeat(inner_width.saturating_sub(used))));
            }
            if !paint_selected {
                spans = stripe_spans(spans, alternate_bg, content_w);
            }
            // Gutter-accent selection keeps the row background unchanged.
            ListItem::new(Line::from(spans)).style(if paint_selected {
                Style::default().bg(selected_bg)
            } else {
                Style::default()
            })
        }
    }
}

/// Columns the row's right inset reserves inside `inner_width`.
const RIGHT_INSET: usize = 2;

/// The row's text-flow content width: every row type (items, headings, the
/// blank spacers) stripes within this same range.
fn row_content_w(inner_width: usize, has_scrollbar: bool) -> usize {
    (inner_width + usize::from(has_scrollbar)).saturating_sub(RIGHT_INSET)
}

/// Carries `alternate_bg` on the row's text-flow spans only: the two-column
/// quiet indent and the right inset stay parent background (and the
/// scrollbar is painted separately), so the stripe bands of items, group
/// headings and blank spacers all cover the same columns. Ratatui fills a
/// ListItem's whole row allocation when its style has a background, so the
/// row style stays unstyled and the trailing padding span carries the stripe
/// out to the content edge.
fn stripe_spans(
    mut spans: Vec<Span<'static>>,
    alternate_bg: Option<Color>,
    content_w: usize,
) -> Vec<Span<'static>> {
    let Some(bg) = alternate_bg else {
        return spans;
    };
    for span in spans.iter_mut().skip(1) {
        span.style = span.style.bg(bg);
    }
    let used: usize = spans.iter().map(|span| span.content.width()).sum();
    if content_w > used {
        spans.push(Span::styled(
            " ".repeat(content_w - used),
            Style::default().bg(bg),
        ));
    }
    spans
}
