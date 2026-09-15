use crate::app::components::media_list::{MediaKind, MediaListRow, MediaSemanticState};
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
/// `selected_bg` is not a free per-caller choice: the focused selected row
/// "punches through" to the surface *containing* the panel that holds the
/// list, so it must be that parent container's background. The surface table
/// owns the mapping: library rails plus Home and Feeds pass the `SelectedRow`
/// row (the library backdrop, even while the list panel itself is
/// focus-green), while queue and nested workspace lists resolve their owning
/// column's selected-row identity (`SelectedRowOnQueueColumn` /
/// `SelectedRowOnLibraryPane`) so the row follows that column's focus. The
/// policy's optional selected-row style may override that surface mapping.
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
    selected_style: Option<crate::app::components::media_list::SelectedRowStyle>,
    inner_width: usize,
    has_scrollbar: bool,
    mut marquee: Option<(&mut String, &mut std::time::Instant)>,
) -> ListItem<'static> {
    match row {
        MediaListRow::Spacer => ListItem::new(Line::default()),
        MediaListRow::Heading { text } => ListItem::new(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                text.clone(),
                Style::default()
                    .fg(palette::TEXT_FOCUS_ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ])),
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
            // `[2-col indent][title…]  [FOAM trailing]  [green duration]`
            // with the title at column 2 and a quiet gap before the right-aligned
            // duration.

            let (fg, progress) = match semantic_state {
                MediaSemanticState::Ordinary => (palette::TEXT_EMPHASIS, None),
                MediaSemanticState::Played => (palette::TEXT_MUTED, None),
                // Active and now-playing rows append the live progress
                // percentage to the trailing text, in the same style; only
                // the throbber glyph is gone.
                MediaSemanticState::Active { progress }
                | MediaSemanticState::NowPlaying { progress } => (
                    palette::ACCENT,
                    (*progress).map(|value| format!("{}%", value.percent())),
                ),
                MediaSemanticState::Starting => (palette::ACCENT, Some("starting".into())),
                MediaSemanticState::Disabled => (palette::TEXT_MUTED, None),
            };
            const LEFT_INSET: usize = 2;
            const QUIET_GAP: usize = 2;
            const RIGHT_INSET: usize = 2;
            let trailing = match (
                trailing.as_deref().filter(|text| !text.is_empty()),
                progress,
            ) {
                (Some(text), Some(pct)) => format!("{text} {pct}"),
                (Some(text), None) => text.to_owned(),
                (None, Some(pct)) => pct,
                (None, None) => String::new(),
            };
            // `Collection` rows never show a duration, even if one is
            // projected — one enforcement point so parents can't re-diverge.
            let duration = duration
                .as_deref()
                .filter(|dur| !dur.is_empty())
                .filter(|_| !matches!(kind, MediaKind::Collection));

            let content_w = (inner_width + usize::from(has_scrollbar)).saturating_sub(RIGHT_INSET);
            let trailing_w = if trailing.is_empty() {
                0
            } else {
                1 + trailing.width()
            };
            let slot_reserve = duration.map_or(0, |dur| QUIET_GAP + dur.width());
            let selected = selected && focused;
            let selected_style = selected.then(|| selected_style).flatten();
            let secondary_separator_reserve =
                usize::from(secondary.as_deref().is_some_and(|text| !text.is_empty()));
            let title_width = content_w.saturating_sub(
                LEFT_INSET + trailing_w + slot_reserve + secondary_separator_reserve,
            );
            let title_color = if selected
                && !matches!(
                    semantic_state,
                    MediaSemanticState::Active { .. } | MediaSemanticState::NowPlaying { .. }
                ) {
                selected_style.map_or(palette::TEXT_EMPHASIS, |style| style.title_fg)
            } else {
                fg
            };
            // Two-tone episode rows: an optional secondary title (the
            // episode title) paints in the yellow focus-accent role after the
            // primary title, with one separating space.
            let parts: Vec<(String, Color)> =
                match secondary.as_deref().filter(|sec| !sec.is_empty()) {
                    Some(sec) => vec![
                        (primary.clone(), title_color),
                        (" ".into(), title_color),
                        (sec.to_owned(), palette::TEXT_FOCUS_ACCENT),
                    ],
                    None => vec![(primary.clone(), title_color)],
                };
            let parts_width: usize = parts.iter().map(|(text, _)| text.width()).sum();
            let mut title_spans = marquee
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
                    let mut spans = vec![Span::styled(
                        trunc_str(primary, title_width - sec_w),
                        Style::default().fg(title_color),
                    )];
                    if let Some(sec) = sec.filter(|_| sec_w > 0) {
                        spans.push(Span::raw(" "));
                        spans.push(Span::styled(
                            trunc_str(sec, sec_w),
                            Style::default().fg(palette::TEXT_FOCUS_ACCENT),
                        ));
                    }
                    spans
                });
            if let Some(style) = selected_style {
                for span in &mut title_spans {
                    span.style = span.style.fg(style.title_fg).bg(style.title_bg);
                }
            }

            let mut spans = vec![Span::raw("  ")];
            spans.extend(title_spans);
            if !trailing.is_empty() {
                spans.push(Span::raw(" "));
                spans.push(Span::styled(
                    trailing,
                    Style::default().fg(palette::TEXT_METADATA),
                ));
            }
            if let Some(dur) = duration {
                let used: usize = spans.iter().map(|span| span.content.width()).sum();
                let pad = content_w.saturating_sub(used + dur.width());
                spans.push(Span::raw(" ".repeat(pad)));
                spans.push(Span::styled(
                    dur.to_owned(),
                    Style::default()
                        .fg(selected_style
                            .map_or(palette::STATUS_AVAILABLE, |style| style.duration_fg)),
                ));
            }
            // Pad the selected row's spans out to the full row width (up to
            // the scrollbar column) so the highlighted background bar spans
            // the whole panel regardless of whether a duration string is
            // present — never just the width of the row text.
            if selected {
                let used: usize = spans.iter().map(|span| span.content.width()).sum();
                spans.push(Span::raw(" ".repeat(inner_width.saturating_sub(used))));
            }
            if !selected {
                if let Some(bg) = alternate_bg {
                    // Ratatui fills a ListItem's whole row allocation when its
                    // style has a background. Keep the row style unstyled and
                    // carry zebra paint only on the text-flow spans instead:
                    // the two-column indent and right inset remain parent
                    // background (and the scrollbar is painted separately).
                    for span in spans.iter_mut().skip(1) {
                        span.style = span.style.bg(bg);
                    }
                    let used = spans.iter().map(|span| span.content.width()).sum();
                    spans.push(Span::styled(
                        " ".repeat(content_w.saturating_sub(used)),
                        Style::default().bg(bg),
                    ));
                }
            }
            ListItem::new(Line::from(spans)).style(if selected {
                Style::default().bg(selected_style.map_or(selected_bg, |style| style.bg))
            } else {
                Style::default()
            })
        }
    }
}
