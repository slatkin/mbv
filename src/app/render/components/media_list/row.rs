use crate::app::components::media_list::{
    ActiveProgress, MediaKind, MediaListRow, MediaListTitleReveal, MediaListTrailing,
    MediaSemanticState,
};
use crate::app::palette;
use crate::app::render::components::marquee::marquee_spans;
use crate::app::ui_util::trunc_str;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::ListItem;
use std::time::Instant;
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
/// row's appearance. Canonical Iris bars use the selected-row Ink foreground,
/// so the bar introduces no bold title or ordinary title-role colour; a
/// now-playing marker retains its live-playback accent.
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
/// both panel edges. Canonical selected bars use the selected-row foreground
/// role for their text; a now-playing marker retains its live-playback accent.
pub(in crate::app) fn media_list_row<Target>(
    row: &MediaListRow<Target>,
    selected: bool,
    focused: bool,
    selected_bg: Color,
    alternate_bg: Option<Color>,
    inner_width: usize,
    has_scrollbar: bool,
    title_reveal: MediaListTitleReveal,
    marquee: Option<(&str, &mut String, &mut Instant)>,
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
                    text.to_uppercase(),
                    Style::default().fg(palette::GROUP_HEADING_FG),
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
            let item = ItemRow {
                primary: primary.as_str(),
                secondary: secondary.as_deref(),
                trailing,
                duration: duration.as_deref(),
                kind: *kind,
                semantic_state,
            };
            let paint = RowPaint {
                selected,
                focused,
                selected_bg,
                alternate_bg,
                inner_width,
                has_scrollbar,
                title_reveal,
                marquee,
            };
            paint_item_row(&item, paint)
        }
    }
}

/// The provider-neutral content of one `Item` row, borrowed from the row.
struct ItemRow<'a> {
    primary: &'a str,
    secondary: Option<&'a str>,
    trailing: &'a Option<MediaListTrailing>,
    duration: Option<&'a str>,
    kind: MediaKind,
    semantic_state: &'a MediaSemanticState,
}

/// The caller-resolved paint parameters for one `Item` row.
struct RowPaint<'a> {
    selected: bool,
    focused: bool,
    selected_bg: Color,
    alternate_bg: Option<Color>,
    inner_width: usize,
    has_scrollbar: bool,
    title_reveal: MediaListTitleReveal,
    marquee: Option<(&'a str, &'a mut String, &'a mut Instant)>,
}

/// Canonical row geometry:
/// `[2-col indent][title…]  [orange progress]  [green gutter]  [gold duration]`
/// with the title at column 2 and quiet gaps before the right-aligned
/// metadata columns.
const LEFT_INSET: usize = 2;
const QUIET_GAP: usize = 2;

/// Orchestrates one `Item` row: resolve style, parts, and budget, then
/// assemble spans left to right. Each stage lives in its own helper, so this
/// stays straight-line.
fn paint_item_row(item: &ItemRow<'_>, mut paint: RowPaint<'_>) -> ListItem<'static> {
    let style = semantic_paint(item.semantic_state);
    let gutter = gutter_text(item.trailing.as_ref());
    // `Collection` rows never show a duration, even if one is
    // projected — one enforcement point so parents can't re-diverge.
    let duration = effective_duration(item.duration, item.kind);
    let trailing = inline_trailing(style.progress.as_ref());
    // The row's own selection bit, before the focus folding below:
    // the title reveal follows the selection, while the selected-row
    // bar and the marquee follow the focused selection.
    let row_selected = paint.selected;
    let focused_selected = paint.selected && paint.focused;
    let trailing_w: usize = trailing.iter().map(|(text, _)| 1 + text.width()).sum();
    let slot_reserve = duration.map_or(0, |dur| QUIET_GAP + dur.width());
    // The metadata gutter is a fixed-width column, so it reserves
    // its full width whatever the string's own length is.
    let date_reserve = usize::from(gutter.is_some()) * (QUIET_GAP + DATE_GUTTER_W);
    let separator_reserve = usize::from(item.secondary.is_some_and(|text| !text.is_empty()));
    let icon_reserve = style.live_icon.map_or(0, UnicodeWidthStr::width);
    let widths = title_budget(&BudgetInputs {
        inner_width: paint.inner_width,
        has_scrollbar: paint.has_scrollbar,
        trailing_w,
        slot_reserve,
        date_reserve,
        separator_reserve,
        icon_reserve,
    });
    // Split rows paint the two-tone palette: primary is the
    // container/context in gold, secondary the item's own name in the
    // soft-white emphasis role, with one separating space. A played
    // row mutes only the secondary; no other state moves the palette (a `NowPlaying` row
    // never carries secondary). Single-part rows keep their semantic
    // title role. (`parts` feeds the selected-row marquee path;
    // unselected split rows paint the same roles below.)
    let parts = title_parts(
        item.primary,
        item.secondary,
        style.fg,
        secondary_title_color(item.semantic_state),
    );
    // A row outside the selection of a reveal-on-selection list paints
    // only its primary (context) text: the secondary title is the
    // item's own name, shown where the user is looking. The row keeps
    // its title slot, so the resting context text uses the columns the
    // whole title would have taken and is cut by the ordinary
    // truncation rule.
    let painted = revealed_parts(&parts, paint.title_reveal, row_selected);
    let title = title_spans(
        paint.marquee.take(),
        focused_selected,
        &parts,
        painted,
        widths.title_width,
        paint.title_reveal,
    );
    let mut spans = leading_spans(style.live_icon, title);
    // The inline metadata pieces contain only the active row's
    // percentage, which paints the progress role. Gutter metadata
    // uses the fixed right-aligned green column below.
    let progress_index = push_inline_trailing(&mut spans, &trailing, style.progress.as_deref());
    push_gutter_column(&mut spans, gutter, duration, widths.content_w);
    push_duration_column(&mut spans, duration, widths.content_w);
    // Every selected row paints the opaque bar edge to edge.
    let paint_selected = focused_selected;
    finish_row_bar(
        &mut spans,
        &BarFinish {
            inner_width: paint.inner_width,
            content_w: widths.content_w,
            paint_selected,
            selected_bg: paint.selected_bg,
            alternate_bg: paint.alternate_bg,
            progress_index,
        },
    );
    // Gutter-accent selection keeps the row background unchanged.
    ListItem::new(Line::from(spans)).style(if paint_selected {
        Style::default().bg(paint.selected_bg)
    } else {
        Style::default()
    })
}

/// The title colour, inline progress text, and now-playing marker derived
/// from one semantic state.
struct SemanticPaint {
    fg: Color,
    progress: Option<String>,
    live_icon: Option<&'static str>,
}

fn progress_text(progress: Option<ActiveProgress>) -> Option<String> {
    progress.map(|value| format!("{}%", value.percent()))
}

fn semantic_paint(semantic_state: &MediaSemanticState) -> SemanticPaint {
    // Now-playing rows lose the accent colour: the aqua play marker
    // before the title marks them instead, so the row text keeps
    // the ordinary role. Active rows retain resume progress inline
    // but do not receive the play marker. Both states append their
    // progress percentage to the trailing text in the same style.
    // A now-playing row never carries secondary (Home, the only
    // split-row producer, emits `Ordinary` rows), so the palette
    // below never applies to one.
    match semantic_state {
        MediaSemanticState::Ordinary => SemanticPaint {
            fg: palette::TEXT_EMPHASIS,
            progress: None,
            live_icon: None,
        },
        MediaSemanticState::Played => SemanticPaint {
            fg: palette::TEXT_MUTED,
            progress: None,
            live_icon: None,
        },
        MediaSemanticState::Active { progress } => SemanticPaint {
            fg: palette::TEXT_EMPHASIS,
            progress: progress_text(*progress),
            live_icon: None,
        },
        MediaSemanticState::NowPlaying { progress } => SemanticPaint {
            fg: palette::TEXT_EMPHASIS,
            progress: progress_text(*progress),
            live_icon: Some("▶ "),
        },
    }
}

fn gutter_text(trailing: Option<&MediaListTrailing>) -> Option<&str> {
    match trailing {
        Some(MediaListTrailing::Gutter(text)) if !text.is_empty() => Some(text.as_str()),
        _ => None,
    }
}

fn effective_duration(duration: Option<&str>, kind: MediaKind) -> Option<&str> {
    duration
        .filter(|dur| !dur.is_empty())
        .filter(|_| !matches!(kind, MediaKind::Collection))
}

fn inline_trailing(progress: Option<&String>) -> Vec<(String, Color)> {
    let mut pieces = Vec::new();
    if let Some(pct) = progress {
        pieces.push((pct.clone(), palette::PROGRESS_PERCENT));
    }
    pieces
}

/// The reserved-slot inputs to the title-width budget.
struct BudgetInputs {
    inner_width: usize,
    has_scrollbar: bool,
    trailing_w: usize,
    slot_reserve: usize,
    date_reserve: usize,
    separator_reserve: usize,
    icon_reserve: usize,
}

struct Budget {
    content_w: usize,
    title_width: usize,
}

fn title_budget(inputs: &BudgetInputs) -> Budget {
    let content_w = row_content_w(inputs.inner_width, inputs.has_scrollbar);
    let title_width = content_w.saturating_sub(
        LEFT_INSET
            + inputs.trailing_w
            + inputs.slot_reserve
            + inputs.date_reserve
            + inputs.separator_reserve
            + inputs.icon_reserve,
    );
    Budget {
        content_w,
        title_width,
    }
}

fn secondary_title_color(semantic_state: &MediaSemanticState) -> Color {
    match semantic_state {
        MediaSemanticState::Played => palette::TEXT_MUTED,
        _ => palette::SPLIT_ROW_TITLE_FG,
    }
}

fn title_parts(
    primary: &str,
    secondary: Option<&str>,
    title_color: Color,
    secondary_color: Color,
) -> Vec<(String, Color)> {
    match secondary.filter(|sec| !sec.is_empty()) {
        Some(sec) => vec![
            (primary.to_owned(), palette::SPLIT_ROW_CONTEXT_FG),
            (" ".into(), palette::SPLIT_ROW_CONTEXT_FG),
            (sec.to_owned(), secondary_color),
        ],
        None => vec![(primary.to_owned(), title_color)],
    }
}

fn revealed_parts(
    parts: &[(String, Color)],
    title_reveal: MediaListTitleReveal,
    row_selected: bool,
) -> &[(String, Color)] {
    let hidden = title_reveal == MediaListTitleReveal::OnSelection && !row_selected;
    if hidden {
        &parts[..1]
    } else {
        parts
    }
}

fn title_spans(
    marquee: Option<(&str, &mut String, &mut Instant)>,
    focused_selected: bool,
    parts: &[(String, Color)],
    painted: &[(String, Color)],
    title_width: usize,
    title_reveal: MediaListTitleReveal,
) -> Vec<Span<'static>> {
    let parts_width: usize = parts.iter().map(|(text, _)| text.width()).sum();
    let marquee_active = focused_selected
        && (parts_width > title_width || title_reveal == MediaListTitleReveal::OnSelection);
    match marquee.filter(|_| marquee_active) {
        Some((key, text, started_at)) => marquee_spans_for(
            key,
            parts,
            title_width,
            text,
            started_at,
            title_reveal == MediaListTitleReveal::OnSelection,
        ),
        None => truncated_title_spans(painted, title_width),
    }
}

fn marquee_spans_for(
    key: &str,
    parts: &[(String, Color)],
    title_width: usize,
    text: &mut String,
    started_at: &mut Instant,
    reveal_on_selection: bool,
) -> Vec<Span<'static>> {
    let borrowed: Vec<(&str, Color)> = parts.iter().map(|(t, c)| (t.as_str(), *c)).collect();
    marquee_spans(
        key,
        &borrowed,
        title_width,
        text,
        started_at,
        reveal_on_selection,
    )
}

fn truncated_title_spans(painted: &[(String, Color)], title_width: usize) -> Vec<Span<'static>> {
    // Whole-row truncation: the title parts (context,
    // separator, item title) are one string cut as a unit —
    // full parts while they fit, then the part that crosses
    // the budget takes the single trailing ellipsis and
    // everything after it is dropped. The context part keeps
    // its full width; the item title absorbs the cut.
    let mut spans = Vec::new();
    let mut used = 0usize;
    for (text, color) in painted {
        if used >= title_width {
            break;
        }
        let budget = title_width - used;
        if text.width() <= budget {
            spans.push(Span::styled(text.clone(), Style::default().fg(*color)));
            used += text.width();
        } else {
            spans.push(Span::styled(
                trunc_str(text, budget),
                Style::default().fg(*color),
            ));
            break;
        }
    }
    spans
}

fn leading_spans(live_icon: Option<&'static str>, title: Vec<Span<'static>>) -> Vec<Span<'static>> {
    let mut spans = vec![Span::raw("  ")];
    if let Some(icon) = live_icon {
        spans.push(Span::styled(icon, Style::default().fg(palette::ACCENT)));
    }
    spans.extend(title);
    spans
}

fn push_inline_trailing(
    spans: &mut Vec<Span<'static>>,
    trailing: &[(String, Color)],
    progress: Option<&str>,
) -> Option<usize> {
    let mut progress_index = None;
    for (text, color) in trailing {
        spans.push(Span::raw(" "));
        let index = spans.len();
        spans.push(Span::styled(text.clone(), Style::default().fg(*color)));
        if progress == Some(text.as_str()) {
            progress_index = Some(index);
        }
    }
    progress_index
}

fn spans_width(spans: &[Span<'static>]) -> usize {
    spans.iter().map(|span| span.content.width()).sum()
}

fn push_gutter_column(
    spans: &mut Vec<Span<'static>>,
    gutter: Option<&str>,
    duration: Option<&str>,
    content_w: usize,
) {
    let Some(gutter) = gutter else {
        return;
    };
    let used = spans_width(spans);
    let tail = DATE_GUTTER_W + duration.map_or(0, |dur| QUIET_GAP + dur.width());
    let pad = content_w.saturating_sub(used + tail);
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(
        format!(
            "{:>width$}",
            trunc_str(gutter, DATE_GUTTER_W),
            width = DATE_GUTTER_W
        ),
        Style::default().fg(palette::STATUS_AVAILABLE),
    ));
}

fn push_duration_column(spans: &mut Vec<Span<'static>>, duration: Option<&str>, content_w: usize) {
    let Some(dur) = duration else {
        return;
    };
    let used = spans_width(spans);
    let pad = content_w.saturating_sub(used + dur.width());
    spans.push(Span::raw(" ".repeat(pad)));
    spans.push(Span::styled(
        dur.to_owned(),
        Style::default().fg(palette::DURATION),
    ));
}

/// The trailing bar treatment: the selected row pads out to the full row
/// width and recolors to the Iris bar; every other row takes its zebra
/// stripe.
struct BarFinish {
    inner_width: usize,
    content_w: usize,
    paint_selected: bool,
    selected_bg: Color,
    alternate_bg: Option<Color>,
    progress_index: Option<usize>,
}

fn finish_row_bar(spans: &mut Vec<Span<'static>>, finish: &BarFinish) {
    if finish.paint_selected {
        // Pad the selected row's spans out to the full row width (up to
        // the scrollbar column) so the highlighted background bar spans
        // the whole panel regardless of whether a duration string is
        // present — never just the width of the row text.
        let used = spans_width(spans);
        spans.push(Span::raw(
            " ".repeat(finish.inner_width.saturating_sub(used)),
        ));
        recolor_selected_bar(spans, finish.selected_bg, finish.progress_index);
    } else {
        *spans = stripe_spans(std::mem::take(spans), finish.alternate_bg, finish.content_w);
    }
}

fn recolor_selected_bar(
    spans: &mut [Span<'static>],
    selected_bg: Color,
    progress_index: Option<usize>,
) {
    // Canonical selected bars are Iris: use the dedicated Ink
    // foreground rather than allowing ordinary title/metadata roles
    // to compete with the selection. The progress percentage keeps a
    // role of its own, but the bar's own variant of it — the orange
    // does not read on the light bar.
    let Some(fg) = selected_row_foreground(selected_bg) else {
        return;
    };
    for (index, span) in spans.iter_mut().enumerate() {
        span.style.fg = Some(if progress_index == Some(index) {
            palette::SELECTED_ROW_PROGRESS_FG
        } else {
            fg
        });
    }
}

fn selected_row_foreground(selected_bg: Color) -> Option<Color> {
    let on_bar = selected_bg == palette::SELECTED_ROW_BG;
    on_bar.then(|| palette::bar_role_fg(palette::SELECTED_ROW_FG, on_bar))
}

/// Columns the row's right inset reserves inside `inner_width`.
const RIGHT_INSET: usize = 2;

/// The fixed width of the right-aligned date-metadata gutter (the podcast
/// browser's `17 Sep` column): the column is the same width for every row
/// that carries a year, date, or runtime, whatever the string's own length is.
const DATE_GUTTER_W: usize = 6;

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
