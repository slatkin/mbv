mod album;
mod card;
mod detail;
mod home;
mod list;
mod music;
mod pills;
mod queue;

use super::super::layout::LayoutPower;
use super::super::ui_util::{build_queue_rows, natural_sort_key, QueueRow};
use super::super::{palette, App, PowerFocus};
use mbv_core::api::MediaItem;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;
use textwrap::wrap;
use unicode_width::UnicodeWidthStr;

// Power View re-renders frequently while scrolling; prefer a cheaper filter in
// these hot paths to reduce terminal image preparation stalls.
pub(super) const POWER_RENDER_FILTER: ratatui_image::FilterType =
    ratatui_image::FilterType::Triangle;

// Configured music albums need the image worker's child-audio lookup; their
// album containers do not reliably expose usable Primary images.
const MUSIC_ALBUM_IMAGE_TYPES: &[&str] = &["AudioChild"];

/// Columns of empty space between the left and right panels in power view.
const POWER_VIEW_GAP: u16 = 0;

/// Left-edge padding applied once to every power-view tab's content area
/// (Home, library lists, music groups, albums, series, home-video, feed
/// groups) plus the music-group pills row, so all tabs share a consistent
/// gutter. Applied at the single dispatch chokepoint in the main render
/// fn; individual tab renderers add only their own content-level gutters
/// (marker columns, banner indents) relative to this padded edge.
///
/// Detail surfaces that need additional internal alignment can add their own
/// indentation relative to this padded edge.
pub(super) const POWER_TAB_LEFT_PAD: u16 = 2;

pub(super) fn render_power_scrollbar(f: &mut Frame, area: Rect, max_offset: usize, offset: usize) {
    let visible = area.height as usize;
    render_power_scrollbar_with_viewport(
        f,
        area,
        max_offset.saturating_add(visible),
        visible,
        offset,
    );
}

pub(super) fn right_panel_scrollbar_area(area: Rect) -> Rect {
    Rect {
        width: area.width.saturating_add(1),
        ..area
    }
}

pub(super) fn render_power_scrollbar_with_viewport(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) {
    if area.height == 0 || viewport_content_length == 0 || content_length <= viewport_content_length
    {
        return;
    }
    let max_offset = content_length.saturating_sub(viewport_content_length);
    let mut state = ScrollbarState::new(max_offset + 1)
        .position(offset.min(max_offset))
        .viewport_content_length(viewport_content_length);
    f.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .thumb_symbol("▐")
            .track_symbol(Some(" "))
            .style(Style::default().fg(palette::SUBTLE))
            .begin_symbol(None)
            .end_symbol(None),
        area,
        &mut state,
    );
}

/// Paints a colored background block spanning display rows `[top_pad_abs, bottom_pad_abs]`
/// (absolute/unscrolled indices into the complete display row sequence), clamped to the
/// visible scroll window `[offset, offset+visible)`. The block fills the full row width
/// supplied by `area.x` and `area.width` (interior content can indent itself further).
/// Call before rendering list/row content so the background shows through.
pub(super) fn render_selected_block_background(
    f: &mut Frame,
    area: Rect,
    offset: usize,
    visible: usize,
    top_pad_abs: usize,
    bottom_pad_abs: usize,
    bg: Color,
) {
    let vis_top = top_pad_abs.max(offset);
    let vis_bot = bottom_pad_abs.min(offset + visible.saturating_sub(1));
    if vis_top <= vis_bot {
        let block_y = area.y + (vis_top - offset) as u16;
        let block_h = (vis_bot - vis_top + 1) as u16;
        f.render_widget(
            Block::default().style(Style::default().bg(bg)),
            Rect {
                x: area.x,
                y: block_y,
                width: area.width,
                height: block_h,
            },
        );
    }
}

/// Paints the ▁/▔ SOFT_WHITE border rows on the reserved rows one position outside
/// the colored block's padding rows `[top_pad_abs, bottom_pad_abs]`.
/// The padding rows are inserted with extra detail rule rows for border space.
/// Call *after* the block's own content and scrollbar render, so borders paint on top.
pub(super) fn render_selected_block_borders(
    f: &mut Frame,
    area: Rect,
    offset: usize,
    visible: usize,
    top_pad_abs: usize,
    bottom_pad_abs: usize,
) {
    let border_style = Style::default().fg(palette::SOFT_WHITE);
    // Top border: paint one row before the colored block padding
    if let Some(top_border) = top_pad_abs.checked_sub(1) {
        if top_border >= offset && top_border < offset + visible {
            let top_y = area.y + (top_border - offset) as u16;
            f.render_widget(
                Paragraph::new(Line::from(Span::styled(
                    "\u{2581}".repeat(area.width as usize),
                    border_style,
                ))),
                Rect {
                    x: area.x,
                    y: top_y,
                    width: area.width,
                    height: 1,
                },
            );
        }
    }
    // Bottom border: paint one row after the colored block padding
    let bot_border = bottom_pad_abs + 1;
    if bot_border >= offset && bot_border < offset + visible {
        let bot_y = area.y + (bot_border - offset) as u16;
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "\u{2594}".repeat(area.width as usize),
                border_style,
            ))),
            Rect {
                x: area.x,
                y: bot_y,
                width: area.width,
                height: 1,
            },
        );
    }
}

fn render_power_queue_panel_frame(f: &mut Frame, area: Rect, desired_rows: u16) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    f.render_widget(
        Block::default().style(Style::default().bg(palette::MEDIA_SELECTED_BG)),
        area,
    );

    let border_style = Style::default().fg(palette::SOFT_WHITE);
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "\u{2594}".repeat(area.width as usize),
            border_style,
        ))),
        Rect { height: 1, ..area },
    );
    if area.height > 1 {
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "\u{2581}".repeat(area.width as usize),
                border_style,
            ))),
            Rect {
                y: area.y + area.height - 1,
                height: 1,
                ..area
            },
        );
    }

    let border_rows = area.height.min(2);
    let use_padding = area.height >= desired_rows.saturating_add(4);
    let top_decoration = 1 + u16::from(use_padding);
    let height = area
        .height
        .saturating_sub(border_rows)
        .saturating_sub(if use_padding { 2 } else { 0 });

    Rect {
        y: area.y + top_decoration,
        height,
        ..area
    }
}

fn rendered_power_queue_rows_for_padding(items: &[MediaItem], panel_area: Rect) -> u16 {
    if items.is_empty() {
        return 1;
    }

    let (display, group_for_header) = build_power_queue_rows(items);
    let padded_visible = panel_area.height.saturating_sub(4) as usize;
    let has_sb = display.len() > padded_visible;
    let render_w = panel_area.width.saturating_sub(u16::from(has_sb)) as usize;
    let wrap_w = render_w.saturating_sub(1).max(1);
    let mut header_idx = 0;
    let mut rows = 0u16;

    for entry in display {
        match entry {
            QueueRow::Header => {
                let group = group_for_header
                    .get(header_idx)
                    .map(|s| s.as_str())
                    .unwrap_or("");
                header_idx += 1;
                rows = rows.saturating_add(wrap(group, wrap_w).len().max(1) as u16);
            }
            QueueRow::Spacer | QueueRow::Track { .. } => rows = rows.saturating_add(1),
        }
    }

    rows
}

pub(super) fn build_power_queue_rows(items: &[MediaItem]) -> (Vec<QueueRow>, Vec<String>) {
    let (display, group_for_header) = build_queue_rows(items, true);
    let mut rows = Vec::with_capacity(display.len().saturating_add(group_for_header.len()));

    for row in display {
        rows.push(row.clone());
        if matches!(row, QueueRow::Header) {
            rows.push(QueueRow::Spacer);
        }
    }

    (rows, group_for_header)
}

/// Style for a selector pill (group/section/artist tab row): dark active text
/// on YELLOW, yellow inactive text on the dark pill background. Shared by
/// every power-view pill row (home's group/section pills, music's group
/// pills) so they can't drift apart on the selected-vs-unselected look.
pub(super) fn selector_pill_style(selected: bool) -> Style {
    if selected {
        Style::default().fg(palette::PILL_DARK).bg(palette::YELLOW)
    } else {
        Style::default()
            .fg(palette::YELLOW)
            .bg(palette::POWER_RIGHT_BG)
    }
}

/// Draws the shared " {count} items" header (SUBTLE) on the first row of
/// `area` and returns `area` shrunk by that one row, so callers can render
/// their list into the remaining space. Used by every tab that shows an
/// item count above its list (home-video, library list) to keep the label
/// styling and the one-row consumption identical.
pub(super) fn render_power_count_label(f: &mut Frame, area: Rect, count: usize) -> Rect {
    if area.width == 0 || area.height == 0 {
        return area;
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" {} items", count),
            Style::default().fg(palette::SUBTLE),
        )),
        Rect { height: 1, ..area },
    );
    Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(1),
        ..area
    }
}

/// The shared left cursor marker span used by every power-view list row.
/// `active` (row is both selected and focused) renders the AQUA half-block
/// `▌`; otherwise a single blank space so unselected rows stay aligned.
/// Only the marker glyph is unified here -- each renderer keeps its own
/// row *text* coloring, which varies by tab.
pub(super) fn selection_marker(active: bool) -> Span<'static> {
    if active {
        Span::styled("\u{258c}", Style::default().fg(palette::AQUA))
    } else {
        Span::raw(" ")
    }
}

/// Width in columns reserved for a power-view list's scrollbar gutter.
pub(super) const POWER_SCROLLBAR_GUTTER: u16 = 1;

/// Usable text width of a list column of the given `width` once the
/// scrollbar gutter is reserved (when `needs_scrollbar`). Centralizes the
/// `width - gutter` arithmetic every scrolling list repeats.
pub(super) fn power_content_width(width: u16, needs_scrollbar: bool) -> usize {
    let gutter = if needs_scrollbar {
        POWER_SCROLLBAR_GUTTER
    } else {
        0
    };
    width.saturating_sub(gutter) as usize
}

/// Renders a single-row horizontal rule (`─` repeated to fill `area`'s width)
/// in `color` -- e.g. a divider between list rows, or the "tail" line under a
/// section header. Shared so separators stay visually identical.
pub(super) fn render_horizontal_rule(f: &mut Frame, area: Rect, color: Color) {
    f.render_widget(
        Paragraph::new(Span::styled(
            "\u{2500}".repeat(area.width as usize),
            Style::default().fg(color),
        )),
        area,
    );
}

/// What to draw behind a pill bar before the pills are overlaid.
pub(super) enum PillUnderlay {
    /// No divider. `fill` clears the row's trailing cells with blanks so the
    /// pills float on the panel background (used by the music-group tabs);
    /// `fill: false` leaves the trailing cells untouched (feed-group tabs).
    Blank { fill: bool },
}

/// A horizontally-scrolling row of selector pills, shared by every power-view
/// pill bar (Home's "Newest" section pills, feed-group tabs, music-group
/// tabs) so their scroll/overflow/selection behavior can't drift apart.
/// Callers pre-truncate `labels`, supply the parallel `ids` recorded as click
/// targets, mark which position is `selected_pos`, and choose an optional
/// leading `prefix` label and the `underlay`.
pub(super) struct PillBar<'a> {
    pub labels: &'a [String],
    pub ids: &'a [usize],
    pub selected_pos: usize,
    pub prefix: Option<&'a str>,
    pub underlay: PillUnderlay,
}

/// Renders `bar` into `area`, scrolling the visible window so `selected_pos`
/// stays on screen with `‹`/`›` chevrons when the pills overflow, and returns
/// the on-screen hitboxes as `(rect, id)` pairs for `layout.selector_tabs`.
pub(super) fn render_pill_bar(f: &mut Frame, area: Rect, bar: PillBar) -> Vec<(Rect, usize)> {
    // `ids` runs parallel to `labels`; a mismatch would panic on the slice
    // below, so assert the contract up front rather than fail cryptically.
    debug_assert_eq!(
        bar.labels.len(),
        bar.ids.len(),
        "render_pill_bar: labels and ids must be parallel"
    );
    let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();
    if area.width == 0 || area.height == 0 || bar.labels.is_empty() {
        return selector_tabs;
    }
    let n = bar.labels.len();
    let bar_w = area.width as usize;
    let prefix_w = bar.prefix.map(|p| p.width()).unwrap_or(0);
    // Display width of each pill is " label " = label width + 2.
    let pill_widths: Vec<usize> = bar.labels.iter().map(|l| l.width() + 2).collect();

    // Greedy: how many pills fit starting at `start` within `avail` columns
    // (1-column gap between consecutive pills).
    let count_fitting = |start: usize, avail: usize| -> usize {
        let mut used = 0usize;
        let mut count = 0usize;
        for width in pill_widths.iter().skip(start) {
            let need = if count == 0 { *width } else { 1 + *width };
            if used + need > avail {
                break;
            }
            used += need;
            count += 1;
        }
        count
    };

    // Advance the scroll window until the selected pill is visible.
    let mut scroll_start = 0usize;
    loop {
        let avail = bar_w
            .saturating_sub(prefix_w)
            .saturating_sub(if scroll_start > 0 { 2 } else { 0 }) // "‹ "
            .saturating_sub(2); // reserve for " ›"
        let cnt = count_fitting(scroll_start, avail);
        if cnt == 0 || scroll_start + cnt > bar.selected_pos {
            break;
        }
        scroll_start += 1;
    }

    let has_left = scroll_start > 0;
    let avail_pills = bar_w
        .saturating_sub(prefix_w)
        .saturating_sub(if has_left { 2 } else { 0 })
        .saturating_sub(2); // reserve for " ›"
    let cnt = count_fitting(scroll_start, avail_pills);
    let scroll_end = (scroll_start + cnt).min(n);
    let has_right = scroll_end < n;

    let mut spans: Vec<Span> = Vec::new();
    let mut x_cursor = area.x;
    if let Some(prefix) = bar.prefix {
        // White label, no background, so an underlay rule shows around it.
        spans.push(Span::styled(
            prefix.to_string(),
            Style::default().fg(Color::White),
        ));
        x_cursor += prefix_w as u16;
    }
    if has_left {
        let chunk = "\u{2039} ";
        spans.push(Span::styled(chunk, Style::default().fg(palette::GREEN)));
        x_cursor += chunk.width() as u16;
    }
    for (offset, (label, &id)) in bar.labels[scroll_start..scroll_end]
        .iter()
        .zip(bar.ids[scroll_start..scroll_end].iter())
        .enumerate()
    {
        if offset > 0 {
            // Single blank gap so the pills float free rather than sitting on
            // a continuous divider.
            spans.push(Span::raw(" "));
            x_cursor += 1;
        }
        let abs_idx = scroll_start + offset;
        let style = selector_pill_style(abs_idx == bar.selected_pos);
        let pill = format!(" {} ", label);
        let pill_w = pill.width() as u16;
        selector_tabs.push((
            Rect {
                x: x_cursor,
                y: area.y,
                width: pill_w,
                height: 1,
            },
            id,
        ));
        spans.push(Span::styled(pill, style));
        x_cursor += pill_w;
    }
    if has_right {
        let chunk = " \u{203a}";
        spans.push(Span::styled(chunk, Style::default().fg(palette::GREEN)));
        x_cursor += chunk.width() as u16;
    }

    // With no rule underlay, optionally clear the rest of the row with blanks.
    if let PillUnderlay::Blank { fill: true } = bar.underlay {
        let used_w = x_cursor.saturating_sub(area.x) as usize;
        let remaining = bar_w.saturating_sub(used_w);
        if remaining > 0 {
            spans.push(Span::raw(" ".repeat(remaining)));
        }
    }

    f.render_widget(Paragraph::new(Line::from(spans)), area);
    selector_tabs
}

/// Draws a shared empty/loading placeholder message (MUTED) at `area`.
/// Callers pass the exact text (`" (empty)"`, `" Loading…"`, or a
/// context-specific string like `"Indexing music library..."`) so the
/// wording stays local, but the placeholder styling is defined once.
pub(super) fn render_power_placeholder(f: &mut Frame, area: Rect, msg: &str) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    f.render_widget(
        Paragraph::new(Span::styled(
            msg.to_string(),
            Style::default().fg(palette::MUTED),
        )),
        area,
    );
}

/// For folder-based music libraries where albums are stored as directories named
/// "Artist (YYYY) Album Title", parse out the three components.
/// Returns `(artist, year, album_title)` on success.
pub(super) fn parse_album_folder_name(name: &str) -> Option<(String, u32, String)> {
    let mut search_from = 0;
    while let Some(rel) = name[search_from..].find(" (") {
        let sp_pos = search_from + rel; // position of the space before '('
        let after_open = sp_pos + 2; // position of first char after '('
        if let Some(close_rel) = name[after_open..].find(')') {
            let year_str = &name[after_open..after_open + close_rel];
            if year_str.len() == 4 {
                if let Ok(year) = year_str.parse::<u32>() {
                    let close_pos = after_open + close_rel; // position of ')'
                    if name[close_pos..].starts_with(") ") {
                        let artist = name[..sp_pos].to_string();
                        let album = name[close_pos + 2..].to_string();
                        return Some((artist, year, album));
                    }
                }
            }
        }
        search_from = sp_pos + 2;
    }
    None
}

/// Strips a leading article ("The ", "A ", "An ") from `s` (case-insensitive).
/// Returns a slice of the original string starting after the article.
fn strip_article(s: &str) -> &str {
    for prefix in &["the ", "a ", "an "] {
        // `s.get(..prefix.len())` returns `None` (rather than panicking, as a
        // byte-index slice would) when `prefix.len()` doesn't land on a UTF-8
        // char boundary — e.g. an accented artist name where the boundary
        // falls inside a multi-byte character.
        if let Some(head) = s.get(..prefix.len()) {
            if head.eq_ignore_ascii_case(prefix) {
                return &s[prefix.len()..];
            }
        }
    }
    s
}

/// Best-effort natural sort key for an album's display artist, computed
/// synchronously (Emby tag or folder-name heuristic only — no network fetch,
/// no cache lookup). Used to pick a sane initial cursor position when a
/// music-group album level first loads (see `handle_lib_event`'s
/// `LibEvent::Loaded` arm in `actions.rs`), before
/// `App::resolve_group_album_artist`'s async fetch has had a chance to run.
/// Mirrors that method's synchronous fallback chain (Emby tag →
/// folder-name-parsed artist → literal "Unknown Artist"), minus the
/// cache/fetch steps, since nothing is cached yet at initial load.
pub(crate) fn initial_group_artist_sort_key(item: &mbv_core::api::MediaItem) -> String {
    let artist = if !item.artist.is_empty() {
        item.artist.clone()
    } else if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
        artist
    } else {
        "Unknown Artist".to_string()
    };
    natural_sort_key(strip_article(&artist))
}

/// Returns the effective sort key for an item: `sort_name` when Emby provides it,
/// otherwise the item's display name with any leading article stripped.
pub(super) fn effective_sort_str(item: &mbv_core::api::MediaItem) -> &str {
    if !item.sort_name.is_empty() {
        &item.sort_name
    } else {
        strip_article(&item.name)
    }
}

/// Returns the letter-group bucket label for `item` given `total` items in the list.
/// Uses `sort_name` when available (so "The Wire" → 'W'), otherwise the article-stripped
/// name. "#" for titles starting with a digit or non-letter; ranges for 50–999 items;
/// individual letters for 250+ items.
pub(super) fn letter_bucket(item: &mbv_core::api::MediaItem, total: usize) -> String {
    let key = effective_sort_str(item);
    let first = key
        .chars()
        .next()
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('\0');
    // KNOWN LIMITATION: any non-ASCII-alphabetic first character (accented
    // letters like "Æon"/"Élan" included, codepoint > 'Z') buckets here as
    // "#". But the "#" *pill*'s Emby fetch bounds are `NameLessThan("A")`
    // -- only titles that SORT BEFORE "A" -- so an accented title with a
    // codepoint after 'Z' is actually fetched by the `V–Z` pill
    // (`name_ge = "V"`, no upper bound) yet renders under this "#" header,
    // making it unreachable from the "#" pill's scoped fetch. Fixing this
    // would mean either teaching the "#" pill to also request `V–Z`-range
    // items with a non-ASCII-alphabetic first char (an Emby-side filter
    // that doesn't exist), or bucketing accented letters under their
    // unaccented equivalent instead of "#" (a bigger behavior change than
    // this pass intends). Left as-is; flagged for a follow-up.
    if !first.is_ascii_alphabetic() {
        return "#".to_string();
    }
    if total >= 250 {
        return first.to_string();
    }
    match first {
        'A'..='C' => "A\u{2013}C",
        'D'..='F' => "D\u{2013}F",
        'G'..='I' => "G\u{2013}I",
        'J'..='L' => "J\u{2013}L",
        'M'..='O' => "M\u{2013}O",
        'P'..='R' => "P\u{2013}R",
        'S'..='U' => "S\u{2013}U",
        _ => "V\u{2013}Z",
    }
    .to_string()
}

/// Library size above which the Power View library list shows the
/// letter-range pill row (see `LetterFilter`), scoping the server fetch to
/// one range at a time. Unrelated to the 50-item in-list header threshold
/// used by `use_letter_groups` in `list.rs`.
pub(crate) const LIBRARY_PILL_THRESHOLD: usize = 300;

/// The letter-range pill buckets, in display order. Single source of truth
/// for both the pill labels and the Emby `NameStartsWithOrGreater` /
/// `NameLessThan` fetch bounds, so they can't drift apart. Mirrors the range
/// boundaries used by `letter_bucket` above.
///
/// KNOWN LIMITATION (see `letter_bucket`'s doc comment): the `"#"` pill's
/// bounds (`NameLessThan("A")`) only reach titles that sort *before* "A".
/// An accented title whose SortName starts with a codepoint after 'Z'
/// (e.g. "Æon Flux") is fetched by the `V–Z` pill but rendered under a
/// `"#"` in-list header, and so is unreachable from the `"#"` pill itself.
const LETTER_FILTER_BUCKETS: &[(&str, Option<&str>, Option<&str>)] = &[
    ("A\u{2013}C", Some("A"), Some("D")),
    ("D\u{2013}F", Some("D"), Some("G")),
    ("G\u{2013}I", Some("G"), Some("J")),
    ("J\u{2013}L", Some("J"), Some("M")),
    ("M\u{2013}O", Some("M"), Some("P")),
    ("P\u{2013}R", Some("P"), Some("S")),
    ("S\u{2013}U", Some("S"), Some("V")),
    ("V\u{2013}Z", Some("V"), None),
    ("#", None, Some("A")),
];

/// A selected letter-range pill: which bucket, its display label, and the
/// Emby name-range bounds to fetch. Constructed only via `for_index`/`default`
/// so it always matches a row in `LETTER_FILTER_BUCKETS`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct LetterFilter {
    pub index: usize,
    pub label: &'static str,
    pub name_ge: Option<&'static str>,
    pub name_lt: Option<&'static str>,
}

impl LetterFilter {
    /// Number of pill buckets (`A–C` … `V–Z`, `#`).
    pub(crate) fn count() -> usize {
        LETTER_FILTER_BUCKETS.len()
    }

    /// Builds the `LetterFilter` for bucket `index`, or `None` if out of range.
    pub(crate) fn for_index(index: usize) -> Option<Self> {
        LETTER_FILTER_BUCKETS
            .get(index)
            .map(|&(label, name_ge, name_lt)| LetterFilter {
                index,
                label,
                name_ge,
                name_lt,
            })
    }

    /// The default pill selected when a large library is first opened: the
    /// first range, `A–C`.
    pub(crate) fn default_filter() -> Self {
        Self::for_index(0).expect("LETTER_FILTER_BUCKETS is non-empty")
    }

    /// All pill labels in bucket order, for building a `PillBar`.
    pub(crate) fn labels() -> Vec<String> {
        LETTER_FILTER_BUCKETS
            .iter()
            .map(|&(label, _, _)| label.to_string())
            .collect()
    }
}

impl App {
    pub(super) fn render_power_view(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutPower,
        playback: &mut super::super::layout::LayoutPlayback,
        tabs_area_out: &mut Rect,
        tabbar_vol_area_out: &mut Rect,
        player_h: u16,
        show_controls: bool,
        now_playing_title: &Option<(String, ratatui::style::Color)>,
    ) {
        if area.height < 4 {
            return;
        }
        // Apply the tab saved from the previous session once libs have loaded.
        if self.power_left_tab_pending > 0 && !self.libs.is_empty() {
            self.power_left_tab = self.power_left_tab_pending.min(self.libs.len());
            self.power_left_tab_pending = 0;
        }
        // Safety clamp -- power_left_tab should already be valid, but guard against
        // any edge case where libs haven't populated yet.
        if self.power_left_tab > self.libs.len() {
            self.power_left_tab = 0;
        }

        // Left panel (card + queue) | Right panel (library, remaining).
        let left_w = if self.power_left_collapsed {
            0
        } else {
            self.power_left_width
        };
        let right_w = area.width.saturating_sub(left_w);

        // Header row removed — the tab bar above indicates current location.
        layout.breadcrumbs = Vec::new();
        layout.selector_tabs = Vec::new();
        let content_h = area.height;
        let left_area = if self.power_left_collapsed {
            Rect::default()
        } else {
            Rect {
                x: area.x,
                y: area.y,
                width: left_w,
                height: content_h,
            }
        };

        // Full-column background behind the card image and queue list.
        if !self.power_left_collapsed {
            f.render_widget(
                Block::default().style(Style::default().bg(palette::CONTINUE_BG)),
                left_area,
            );
        }

        // Full-column background for the right panel (tabs, player, library, queue, status).
        let right_full_area = Rect {
            x: area.x + left_w + POWER_VIEW_GAP,
            y: area.y,
            width: right_w.saturating_sub(POWER_VIEW_GAP),
            height: area.height,
        };
        f.render_widget(
            Block::default().style(Style::default().bg(palette::POWER_RIGHT_BG)),
            right_full_area,
        );

        // Inner content area with padding inside the colored box (queue uses this).
        let left_content = Rect {
            x: left_area.x + 2,
            y: left_area.y + 3,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(4),
        };
        // Blank row, queue title row, then card image.
        if !self.power_left_collapsed {
            self.render_power_queue_title(
                f,
                Rect {
                    x: left_area.x + 2,
                    y: left_area.y + 1,
                    width: left_area.width.saturating_sub(4),
                    height: 1,
                },
                layout,
            );
        }
        let card_area = Rect {
            x: left_area.x + 2,
            y: left_area.y + 3,
            width: left_area.width.saturating_sub(4),
            height: left_area.height.saturating_sub(4),
        };

        let tab_h: u16 = super::TAB_BAR_BOX_HEIGHT;
        let right_area = Rect {
            x: area.x + left_w + POWER_VIEW_GAP,
            y: area.y + tab_h + player_h,
            width: right_w.saturating_sub(POWER_VIEW_GAP),
            height: content_h
                .saturating_sub(1)
                .saturating_sub(tab_h)
                .saturating_sub(player_h),
        };

        // Tab bar at the very top of the right column.
        let tab_area = Rect {
            x: right_area.x,
            y: area.y,
            width: right_area.width,
            height: tab_h,
        };
        self.render_tabs(f, tab_area, tabs_area_out, tabbar_vol_area_out, true);

        // Player panel below the tab bar.
        if player_h > 0 {
            let player_area = Rect {
                x: right_area.x,
                y: area.y + tab_h,
                width: right_area.width,
                height: player_h,
            };
            self.render_player_panel(
                f,
                player_area,
                playback,
                player_h,
                show_controls,
                now_playing_title,
            );
        }

        // Status bar sits at the bottom of the right panel only.
        let status_area = Rect {
            x: right_area.x,
            y: right_area.y + right_area.height,
            width: right_area.width,
            height: 1,
        };

        let queue_focused = matches!(self.power_focus, PowerFocus::Queue);
        let left_focused = !queue_focused;

        let (lib_area, queue_area) = if self.power_left_collapsed {
            (right_area, Rect::default())
        } else {
            // The card fills the top of the left column; the queue list takes
            // the rows below it. Short terminals keep that same structure.
            let (card_h, _) = self.render_power_card(f, card_area);
            let left_remaining = left_content.height.saturating_sub(card_h);
            (
                right_area,
                Rect {
                    y: left_content.y + card_h,
                    height: left_remaining,
                    ..left_content
                },
            )
        };

        // Apply the shared horizontal padding once here, at the single point
        // where the tab content area is finalized, so every tab kind (and the
        // music-group pills row below) inherits consistent left/right gutters
        // instead of each renderer inventing its own. When the left column is
        // collapsed the user has asked to reclaim maximum width, so the gutters
        // are dropped and the library spans the panel edge-to-edge.
        let lib_area = if self.power_left_collapsed {
            lib_area
        } else {
            Rect {
                x: lib_area.x + POWER_TAB_LEFT_PAD,
                width: lib_area
                    .width
                    .saturating_sub(POWER_TAB_LEFT_PAD.saturating_mul(2)),
                ..lib_area
            }
        };

        let mut render_lib_area = lib_area;
        if self.power_left_tab > 0 && self.is_music_group_view(self.power_left_tab - 1) {
            let lib_idx = self.power_left_tab - 1;
            if lib_area.height > 0 {
                let pills_area = Rect {
                    x: lib_area.x,
                    y: lib_area.y,
                    width: lib_area.width,
                    height: 1,
                };
                self.render_power_music_group_pills_row(f, pills_area, lib_idx, layout);
                render_lib_area = Rect {
                    y: lib_area.y + 2,
                    height: lib_area.height.saturating_sub(2),
                    ..lib_area
                };
            } else {
                layout.selector_tabs = Vec::new();
            }
        } else if self.power_left_tab > 0 && self.should_show_letter_pills(self.power_left_tab - 1)
        {
            let lib_idx = self.power_left_tab - 1;
            if lib_area.height > 0 {
                let pills_area = Rect {
                    x: lib_area.x,
                    y: lib_area.y,
                    width: lib_area.width,
                    height: 1,
                };
                self.render_power_letter_pills_row(f, pills_area, lib_idx, layout);
                render_lib_area = Rect {
                    y: lib_area.y + 2,
                    height: lib_area.height.saturating_sub(2),
                    ..lib_area
                };
            } else {
                layout.selector_tabs = Vec::new();
            }
        }

        if !self.power_left_collapsed {
            let desired_queue_rows = {
                let queue = self.displayed_queue();
                rendered_power_queue_rows_for_padding(&queue.items, queue_area)
            };
            let queue_list_area = render_power_queue_panel_frame(f, queue_area, desired_queue_rows);
            self.render_power_queue(f, queue_list_area, queue_focused, layout);
        }
        self.render_power_library(f, render_lib_area, left_focused, layout);

        // Status bar + toast overlay at the bottom of the right panel.
        if status_area.width > 0 {
            self.render_status_bar(f, status_area, playback, false, true);
            let show_toast =
                !self.status.is_empty() && (!self.system_notifications || self.notif_failed);
            if show_toast {
                f.render_widget(Clear, status_area);
                f.render_widget(
                    Paragraph::new(Self::toast_line(&self.status))
                        .alignment(Alignment::Center)
                        .style(Style::default().fg(palette::TEXT).bg(palette::IRIS)),
                    status_area,
                );
            }
        }
    }

    fn render_power_library(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        // If a music-group library's nav_stack was truncated to just the group
        // level (e.g., stale breadcrumb click), immediately re-push the album level.
        if self.power_left_tab > 0 {
            self.ensure_music_group_album_level(self.power_left_tab - 1);
            self.ensure_feed_home_video_group_level(self.power_left_tab - 1);
        }

        if self.power_left_tab == 0 {
            self.render_power_home_list(f, area, focused, layout);
            return;
        }
        let lib_idx = self.power_left_tab.saturating_sub(1);
        let is_feed_group = self.power_left_tab > 0 && self.is_feed_home_video_group_view(lib_idx);
        let is_music_group = self.power_left_tab > 0 && self.is_music_group_view(lib_idx);
        let is_album_folders = self.power_left_tab > 0 && self.is_viewing_album_folders(lib_idx);
        let is_album = self.power_left_tab > 0 && self.is_album_level(lib_idx);
        let is_home_video = self.power_left_tab > 0 && self.is_home_video_view(lib_idx);
        if is_feed_group {
            self.render_power_feed_home_video_group_view(f, area, lib_idx, focused, layout);
        } else if is_album_folders && is_music_group {
            self.render_power_music_group_view(f, area, lib_idx, focused, layout);
        } else if is_album_folders {
            self.render_power_list(f, area, focused, layout);
        } else if is_album {
            let (items, cursor) = {
                let lvl = self.libs[lib_idx].nav_stack.last();
                match lvl {
                    Some(l) => (l.items.clone(), l.cursor),
                    None => (Vec::new(), 0),
                }
            };
            self.render_power_album_detail(f, area, &items, cursor, focused, true, false, layout);
        } else if is_home_video {
            self.render_power_home_video_list(f, area, lib_idx, focused, layout);
        } else {
            self.render_power_list(f, area, focused, layout);
        }
    }

    /// Returns the currently cursor-selected item at the album-folder-listing
    /// nav_stack level (i.e. the level where `is_viewing_album_folders`
    /// holds), if any. The cursor field always indexes into the raw
    /// `items` array in the order it was fetched (SortName-by-album-title)
    /// -- *not* the artist-grouped display order that
    /// `render_power_music_group_view` builds for rendering -- so a plain
    /// `items.get(cursor)` is correct even for the grouped music view.
    pub(in crate::app) fn selected_album_item(
        &self,
        lib_idx: usize,
    ) -> Option<mbv_core::api::MediaItem> {
        let lvl = self.libs[lib_idx].nav_stack.last()?;
        lvl.items.get(lvl.cursor).cloned()
    }

    /// Resolves the display artist for an album item in the grouped power
    /// music views. Priority order:
    /// 1. `item.artist` (Emby's Album-entity metadata) if non-empty.
    /// 2. `album_artist_cache` entry if non-empty (fetched from the album's
    ///    first few tracks — see `fetch_album_artist` in `images.rs`).
    /// 3. `parse_album_folder_name` heuristic as an interim guess — and if
    ///    the cache has neither a value nor an empty-tombstone yet, and no
    ///    fetch is already in flight, triggers `fetch_album_artist`.
    /// 4. Literal "Unknown Artist".
    pub(super) fn resolve_group_album_artist(&mut self, item: &mbv_core::api::MediaItem) -> String {
        if !item.artist.is_empty() {
            return item.artist.clone();
        }
        if let Some(cached) = self.album_artist_cache.get(&item.id) {
            if !cached.is_empty() {
                return cached.clone();
            }
        } else if !self.album_artist_loading.contains(&item.id) {
            self.fetch_album_artist(item.id.clone());
        }
        if let Some((artist, _, _)) = parse_album_folder_name(&item.name) {
            return artist;
        }
        "Unknown Artist".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::layout::{LayoutPlayback, LayoutPower, PowerLeftRowTarget};
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{BrowseLevel, LibraryTab, QueueScope};
    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    use mbv_core::api::MediaItem;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn power_view_uses_triangle_resampling() {
        assert_eq!(POWER_RENDER_FILTER, ratatui_image::FilterType::Triangle);
    }

    fn render_power_scrollbar_column(height: u16, max_offset: usize, offset: usize) -> String {
        let backend = TestBackend::new(1, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_power_scrollbar(f, Rect::new(0, 0, 1, height), max_offset, offset);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    fn render_power_scrollbar_column_with_viewport(
        height: u16,
        content_length: usize,
        viewport_content_length: usize,
        offset: usize,
    ) -> String {
        let backend = TestBackend::new(1, height);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            render_power_scrollbar_with_viewport(
                f,
                Rect::new(0, 0, 1, height),
                content_length,
                viewport_content_length,
                offset,
            );
        })
        .unwrap();
        buffer_to_string(&term)
    }

    #[test]
    fn power_scrollbar_is_proportional_and_reaches_both_ends() {
        let top = render_power_scrollbar_column(7, 3, 0);
        let bottom = render_power_scrollbar_column(7, 3, 3);

        assert_eq!(top.lines().next(), Some("▐"));
        assert_eq!(bottom.lines().last(), Some("▐"));
        assert!(
            top.matches('▐').count() > 2,
            "expected a proportional thumb:\n{top}"
        );
    }

    #[test]
    fn power_scrollbar_respects_custom_viewport_units() {
        let top = render_power_scrollbar_column_with_viewport(7, 10, 2, 0);
        let bottom = render_power_scrollbar_column_with_viewport(7, 10, 2, 8);

        assert_eq!(top.matches('▐').count(), 1);
        assert_eq!(top.lines().next(), Some("▐"));
        assert_eq!(bottom.lines().last(), Some("▐"));
    }

    fn buffer_to_string(term: &Terminal<TestBackend>) -> String {
        let buf = term.backend().buffer();
        let area = *buf.area();
        let mut out = String::new();
        for y in 0..area.height {
            for x in 0..area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    /// Renders a pill bar of the given labels/ids into a `width`-wide row and
    /// returns the resulting `(rect, id)` hitboxes.
    fn render_pill_bar_hitboxes(
        labels: &[String],
        ids: &[usize],
        selected_pos: usize,
        width: u16,
    ) -> Vec<(Rect, usize)> {
        let backend = TestBackend::new(width, 1);
        let mut term = Terminal::new(backend).unwrap();
        let mut tabs = Vec::new();
        term.draw(|f| {
            tabs = render_pill_bar(
                f,
                Rect::new(0, 0, width, 1),
                PillBar {
                    labels,
                    ids,
                    selected_pos,
                    prefix: None,
                    underlay: PillUnderlay::Blank { fill: true },
                },
            );
        })
        .unwrap();
        tabs
    }

    #[test]
    fn pill_bar_hitboxes_carry_caller_ids_not_display_positions() {
        // ids are deliberately offset from positions (mirroring Home's
        // section_idx = position + 10 here) so a regression that returned the
        // display offset instead of the id would be caught.
        let labels: Vec<String> = ["Alpha", "Beta", "Gamma"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let ids = vec![10usize, 11, 12];

        // Wide enough to show every pill: all ids returned, in order.
        let tabs = render_pill_bar_hitboxes(&labels, &ids, 0, 60);
        assert_eq!(
            tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
            vec![10, 11, 12],
        );
        // Hitboxes are left-to-right and non-overlapping.
        for pair in tabs.windows(2) {
            assert!(pair[0].0.x + pair[0].0.width <= pair[1].0.x);
        }
    }

    #[test]
    fn pill_bar_scrolls_to_keep_selected_visible_and_maps_its_id() {
        // Six pills in a narrow row force horizontal scrolling; selecting the
        // last one must scroll it into view and report its caller id (25).
        let labels: Vec<String> = (0..6).map(|i| format!("Group{i}")).collect();
        let ids: Vec<usize> = (0..6).map(|i| 20 + i).collect();

        let tabs = render_pill_bar_hitboxes(&labels, &ids, 5, 18);

        assert!(!tabs.is_empty(), "expected at least one visible pill");
        // The selected pill (id 25) must be among the visible hitboxes.
        assert!(
            tabs.iter().any(|(_, id)| *id == 25),
            "selected pill's id should be visible after scrolling, got {:?}",
            tabs.iter().map(|(_, id)| *id).collect::<Vec<_>>(),
        );
        // Every visible id is one we supplied (never a bare display offset).
        assert!(tabs.iter().all(|(_, id)| (20..=25).contains(id)));
        // Overflow occurred, so scrolling dropped at least one leading pill.
        assert!(
            tabs.len() < labels.len(),
            "narrow row should not fit all six pills"
        );
    }

    fn render_power_library_to_terminal(
        app: &mut App,
        layout: &mut LayoutPower,
    ) -> Terminal<TestBackend> {
        render_power_library_to_terminal_focused(app, layout, true)
    }

    fn render_power_library_to_terminal_focused(
        app: &mut App,
        layout: &mut LayoutPower,
        focused: bool,
    ) -> Terminal<TestBackend> {
        // 20 rows is comfortably enough for the " N items" header row (that
        // `render_power_list` draws unconditionally for a focused library
        // panel) plus the selected row and the compact banner's
        // content-dependent height (#263) for the short test overviews used
        // by callers of this helper.
        let backend = TestBackend::new(60, 20);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_library(f, Rect::new(0, 0, 60, 20), focused, layout);
        })
        .unwrap();
        term
    }

    fn render_power_library_to_string(app: &mut App, layout: &mut LayoutPower) -> String {
        let term = render_power_library_to_terminal(app, layout);
        buffer_to_string(&term)
    }

    fn render_power_view_to_terminal(
        app: &mut App,
        width: u16,
        height: u16,
    ) -> (Terminal<TestBackend>, LayoutPower) {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_view(
                f,
                Rect::new(0, 0, width, height),
                &mut layout,
                &mut LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        (term, layout)
    }

    fn render_power_view(app: &mut App, width: u16, height: u16) -> LayoutPower {
        render_power_view_to_terminal(app, width, height).1
    }

    #[test]
    fn expanded_power_view_tab_panel_has_two_column_side_gutters() {
        let mut app = make_app_stub();
        app.power_left_width = 40;

        let layout = render_power_view(&mut app, 80, 24);

        assert_eq!(layout.left_area.x, 40 + POWER_TAB_LEFT_PAD);
        assert_eq!(layout.left_area.width, 40 - 2 * POWER_TAB_LEFT_PAD);
    }

    #[test]
    fn right_panel_scrollbar_uses_one_column_right_padding() {
        let content_area = Rect::new(42, 3, 36, 10);

        let scrollbar_area = right_panel_scrollbar_area(content_area);

        assert_eq!(scrollbar_area.x, content_area.x);
        assert_eq!(scrollbar_area.width, content_area.width + 1);
    }

    fn make_power_movie_app() -> App {
        let mut app = make_app_stub();
        app.power_left_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        let mut focused = make_item("Focused Movie", "Movie");
        focused.id = "movie-focused".into();
        focused.overview = "This overview should appear in the compact movie banner while the list remains visible underneath.".into();
        focused.director = "Director Hidden".into();
        focused.production_year = 1988;
        focused.genre = "Action".into();

        let mut second = make_item("Second Movie", "Movie");
        second.id = "movie-second".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: vec![focused, second],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    fn make_power_queue_app(item_count: usize) -> App {
        let mut app = make_power_movie_app();
        app.power_focus = PowerFocus::Queue;
        app.player_tab.set_items(
            (0..item_count)
                .map(|i| make_item(&format!("Queue Item {i}"), "Movie"))
                .collect(),
            0,
        );
        app
    }

    fn make_power_remote_queue_app() -> App {
        let local_items = vec![make_item("Local Queue Item", "Movie")];
        let remote_items = vec![make_item("Remote Queue Item", "Movie")];
        let (remote, player_rx) = mbv_core::remote_player::RemotePlayer::stub(remote_items, 0);
        let mut app = App::new_remote(EmbyClient::new(Config::default()), remote, player_rx, false);
        app.power_left_tab = 1;
        app.power_focus = PowerFocus::Queue;
        app.queue_scope = QueueScope::Remote;
        app.player_tab.set_items(local_items, 0);
        app
    }

    #[test]
    fn movie_library_unfocused_selected_banner_keeps_text_right_of_indicator() {
        let mut app = make_power_movie_app();
        let mut layout = LayoutPower::default();

        let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.lines().collect();

        // The colored-block look removes the green `▌` indicator entirely
        // (both focused and unfocused); the selected title sits inside the
        // MEDIA_SELECTED_BG block with a 2-col leading pad instead.
        let selected_line = lines
            .iter()
            .find(|line| line.contains("Focused Movie"))
            .expect("expected selected movie row");
        assert_eq!(
            selected_line.find('▌'),
            None,
            "expected no green selected-row indicator inside the colored block while unfocused:\n{out}"
        );

        let overview_line = lines
            .iter()
            .find(|line| line.contains("compact movie banner"))
            .expect("expected compact overview line");
        assert_eq!(
            overview_line.find('▌'),
            None,
            "expected no green banner bar inside the colored block while unfocused:\n{out}"
        );
    }

    #[test]
    fn power_view_uses_configured_left_column_width() {
        let mut app = make_power_movie_app();
        app.power_left_width = 55;

        let layout = render_power_view(&mut app, 100, 28);

        assert_eq!(layout.queue_area.width, 51);
    }

    #[test]
    fn collapsed_power_left_column_gives_library_full_width() {
        let mut app = make_power_movie_app();
        app.power_left_width = 55;
        app.power_left_collapsed = true;

        let layout = render_power_view(&mut app, 100, 28);

        assert_eq!(layout.queue_area, Rect::default());
        assert_eq!(layout.left_area.x, 0);
        assert_eq!(layout.left_area.width, 100);
    }

    #[test]
    fn short_power_view_keeps_queue_in_left_column() {
        let mut app = make_power_movie_app();
        app.power_left_width = 40;

        let layout = render_power_view(&mut app, 100, 12);

        assert!(
            layout.queue_area.x < app.power_left_width,
            "expected short-height queue to stay in the left column, got {:?}",
            layout.queue_area
        );
        assert!(
            layout.left_area.x >= app.power_left_width,
            "expected library area to remain in the right column, got {:?}",
            layout.left_area
        );
    }

    #[test]
    fn power_queue_panel_uses_selected_media_frame_and_background() {
        let mut app = make_power_queue_app(2);

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let buf = term.backend().buffer();
        let top_y = layout.queue_area.y - 2;
        let bottom_y = layout.queue_area.y + layout.queue_area.height + 1;
        let x = layout.queue_area.x;

        assert_eq!(buf[(x, top_y)].symbol(), "\u{2594}");
        assert_eq!(buf[(x, top_y)].fg, palette::SOFT_WHITE);
        assert_eq!(buf[(x, bottom_y)].symbol(), "\u{2581}");
        assert_eq!(buf[(x, bottom_y)].fg, palette::SOFT_WHITE);
        assert_eq!(buf[(x, layout.queue_area.y)].bg, palette::MEDIA_SELECTED_BG);
        assert_eq!(
            buf[(x, layout.queue_area.y - 1)].bg,
            palette::MEDIA_SELECTED_BG
        );
        assert_eq!(
            buf[(x, layout.queue_area.y + layout.queue_area.height)].bg,
            palette::MEDIA_SELECTED_BG
        );
    }

    #[test]
    fn power_queue_panel_fills_remaining_left_column_with_short_queue() {
        let mut app = make_power_queue_app(1);

        let (_term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let bottom_y = layout.queue_area.y + layout.queue_area.height + 1;

        assert_eq!(bottom_y, 26);
        assert!(
            layout.queue_area.height > 1,
            "expected queue viewport inside full-height panel, got {:?}",
            layout.queue_area
        );
    }

    #[test]
    fn power_queue_panel_empty_state_is_inside_panel() {
        let mut app = make_power_queue_app(0);

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let out = buffer_to_string(&term);
        let empty_y = out
            .lines()
            .position(|line| line.contains("Add items with p"))
            .expect("expected queue empty-state message");

        assert_eq!(empty_y as u16, layout.queue_area.y);
        assert_eq!(
            term.backend().buffer()[(layout.queue_area.x, empty_y as u16)].bg,
            palette::MEDIA_SELECTED_BG
        );
    }

    #[test]
    fn power_queue_panel_remains_visible_when_unfocused() {
        let mut app = make_power_queue_app(1);
        app.power_focus = PowerFocus::Left;

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let buf = term.backend().buffer();
        let top_y = layout.queue_area.y - 2;
        let bottom_y = layout.queue_area.y + layout.queue_area.height + 1;

        assert_eq!(buf[(layout.queue_area.x, top_y)].symbol(), "\u{2594}");
        assert_eq!(buf[(layout.queue_area.x, bottom_y)].symbol(), "\u{2581}");
        assert_eq!(
            buf[(layout.queue_area.x, layout.queue_area.y)].bg,
            palette::MEDIA_SELECTED_BG
        );
    }

    #[test]
    fn power_queue_title_and_scope_pills_stay_outside_panel() {
        let mut app = make_power_remote_queue_app();
        app.use_nerd_fonts = false;

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let top_y = layout.queue_area.y - 2;
        let out = buffer_to_string(&term);
        let header = out
            .lines()
            .nth(layout.queue_scope_local_area.y as usize)
            .expect("expected queue header row");
        let device_name = mbv_core::api::device_name();
        let upper_device_name = device_name.to_uppercase();

        assert!(layout.queue_scope_local_area.y < top_y);
        assert!(layout.queue_scope_remote_area.y < top_y);
        assert!(layout.queue_scope_remote_area.x > layout.queue_scope_local_area.x);
        assert_eq!(
            layout.queue_scope_local_area.width + layout.queue_scope_remote_area.width,
            layout.queue_area.width
        );
        assert!(
            header.matches(&upper_device_name).count() >= 2,
            "expected local and remote queue controls to use session-style hostname pills:\n{out}"
        );
        assert!(
            !header.contains('\u{F0AFE}'),
            "expected non-Nerd-Font queue header to avoid private-use glyphs:\n{out}"
        );
    }

    #[test]
    fn power_queue_title_does_not_render_playlist_pill() {
        let mut app = make_power_remote_queue_app();
        app.queue_source = crate::config::QueueSource::Playlist {
            id: None,
            name: "Road Mix".into(),
        };

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let out = buffer_to_string(&term);
        let header = out
            .lines()
            .nth(layout.queue_scope_local_area.y as usize)
            .expect("expected queue header row");
        let device_name = mbv_core::api::device_name();
        let upper_device_name = device_name.to_uppercase();

        assert!(
            header.contains(&upper_device_name),
            "expected session hostname pill in queue header:\n{out}"
        );
        assert!(
            !header.contains("Road Mix") && !header.contains("none"),
            "expected playlist pill to stay out of queue header:\n{out}"
        );
    }

    #[test]
    fn power_view_bottom_status_bar_shows_playlist_pill_when_queue_is_a_playlist() {
        let mut app = make_power_queue_app(2);
        app.queue_source = crate::config::QueueSource::Playlist {
            id: Some("pl1".into()),
            name: "Road Mix".into(),
        };

        let (term, _layout) = render_power_view_to_terminal(&mut app, 100, 28);
        let out = buffer_to_string(&term);
        let last_line = out.lines().last().unwrap_or_default();

        assert!(
            last_line.contains("Road Mix"),
            "expected the playlist pill to appear in the Power View status bar:\n{last_line}"
        );
        let text_x = last_line
            .find("Road Mix")
            .expect("expected playlist name position") as u16;
        assert_eq!(
            term.backend().buffer()[(text_x, 27)].fg,
            palette::YELLOW,
            "expected playlist pill text to be yellow, not green:\n{last_line}"
        );
    }

    #[test]
    fn short_power_queue_panel_drops_padding_before_rows() {
        let mut app = make_power_queue_app(20);

        let (term, layout) = render_power_view_to_terminal(&mut app, 100, 12);
        let buf = term.backend().buffer();
        let top_y = layout.queue_area.y - 1;
        let bottom_y = layout.queue_area.y + layout.queue_area.height;

        assert_eq!(buf[(layout.queue_area.x, top_y)].symbol(), "\u{2594}");
        assert_eq!(buf[(layout.queue_area.x, bottom_y)].symbol(), "\u{2581}");
        assert!(
            layout.queue_area.height >= 1,
            "expected at least one usable queue row on a short terminal, got {:?}",
            layout.queue_area
        );
    }

    #[test]
    fn power_queue_panel_counts_wrapped_group_headers_before_adding_padding() {
        let mut app = make_power_movie_app();
        app.power_focus = PowerFocus::Queue;
        let mut item = make_item("Track", "Audio");
        item.id = "boundary-track".into();
        item.album_id = "boundary-album".into();
        item.album = "Long Album Title".into();
        item.artist = "Very Long Artist".into();
        app.player_tab.set_items(vec![item], 0);

        let panel_area = Rect::new(0, 0, 20, 6);
        let desired_rows =
            rendered_power_queue_rows_for_padding(&app.displayed_queue().items, panel_area);
        assert_eq!(desired_rows, 4);

        let backend = TestBackend::new(panel_area.width, panel_area.height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            let queue_area = render_power_queue_panel_frame(f, panel_area, desired_rows);
            app.render_power_queue(f, queue_area, true, &mut layout);
        })
        .unwrap();
        let out = buffer_to_string(&term);

        assert_eq!(layout.queue_area.y, 1);
        assert_eq!(layout.queue_area.height, 4);
        assert!(
            layout.queue_row_map.contains(&Some(0)),
            "expected selected track row to be mapped as visible after wrapped header: {:?}",
            layout.queue_row_map
        );
        assert!(
            out.contains("1. Track"),
            "expected selected track to remain visible below the wrapped group header:\n{out}"
        );
    }

    #[test]
    fn power_queue_panel_preserves_group_aware_scrolling() {
        let mut app = make_power_movie_app();
        app.power_focus = PowerFocus::Queue;

        let mut items = Vec::new();
        for i in 0..4 {
            let mut item = make_item(&format!("A{i}"), "Audio");
            item.id = format!("a-{i}");
            item.album_id = "album-a".into();
            item.album = "Album A".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        for i in 0..4 {
            let mut item = make_item(&format!("B{i}"), "Audio");
            item.id = format!("b-{i}");
            item.album_id = "album-b".into();
            item.album = "Album B".into();
            item.artist = "Artist".into();
            items.push(item);
        }
        app.player_tab.set_items(items, 4);
        app.power_queue_scroll = 9;

        let (_term, _layout) = render_power_view_to_terminal(&mut app, 100, 20);

        assert_eq!(app.power_queue_scroll, 9);
    }

    fn make_power_music_group_app() -> App {
        let mut app = make_app_stub();
        app.power_left_tab = 1;
        app.music_levels = vec!["group".into(), "album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        // Six groups is enough to force horizontal scrolling in a narrow test terminal.
        let group_names = ["Alpha", "Beta", "Gamma", "Delta", "Epsilon", "Zeta"];
        let groups: Vec<MediaItem> = group_names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let mut it = make_item(n, "MusicArtist");
                it.id = format!("group-{i}");
                it.is_folder = true;
                it
            })
            .collect();

        let mut album = make_item("First Album", "MusicAlbum");
        album.id = "album-1".into();
        album.artist = "Alpha".into();
        album.production_year = 2001;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-music".into(),
                    title: "Music".into(),
                    items: groups,
                    total_count: group_names.len(),
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                },
                BrowseLevel {
                    parent_id: "group-0".into(),
                    title: "Alpha".into(),
                    items: vec![album],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                },
            ],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    #[test]
    fn selectable_artist_headers_are_typed_row_targets() {
        let mut app = make_power_music_group_app();
        // Headers for groups with only one child are not selectable, so give
        // Alpha a second album to keep it eligible as a typed row target.
        let mut alpha_album2 = make_item("Second Alpha Album", "MusicAlbum");
        alpha_album2.id = "album-1b".into();
        alpha_album2.artist = "Alpha".into();
        alpha_album2.is_folder = true;
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(alpha_album2);
        let mut beta_album = make_item("Beta Album", "MusicAlbum");
        beta_album.id = "album-2".into();
        beta_album.artist = "Beta".into();
        beta_album.is_folder = true;
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(beta_album);

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);

        assert!(
            out.contains("Alpha") && out.contains("Beta"),
            "expected both artist headers to render:\n{out}"
        );
        assert!(
            matches!(
                layout.left_row_targets.first().and_then(Option::as_ref),
                Some(PowerLeftRowTarget::ArtistHeader(selection))
                    if selection.artist_label == "Alpha"
                        && selection.first_album_id == "album-1"
            ),
            "expected the first custom artist header to be a typed row target"
        );
        assert_eq!(
            layout.left_row_map.first(),
            Some(&None),
            "legacy row map must keep headers non-album rows"
        );
    }

    #[test]
    fn selectable_artist_header_renders_focused() {
        let mut app = make_power_music_group_app();
        app.libs[0].artist_header_focus = Some(crate::app::ArtistHeaderSelection {
            first_album_id: "album-1".into(),
            artist_label: "Alpha".into(),
        });

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();
        let header_row = lines
            .iter()
            .position(|line| line.contains("Alpha"))
            .expect("expected Alpha header");
        let header = lines[header_row];

        assert!(
            !header.contains('\u{258c}'),
            "selected artist header should no longer render the left focus gutter:\n{out}"
        );
        assert!(
            !header.contains('\u{f037b}'),
            "selected artist header should no longer render the trailing focus icon \
             (the selection block now carries the focus signal):\n{out}"
        );

        // The header should now be wrapped in the same colored-block frame
        // as a selected album: a `▁` border row (with a blank colored-bg
        // padding row directly beneath it) above the header, an action-hint
        // row directly below the header (no `ENTER` clause, unlike the
        // album hint), then a colored-bg padding row and a `▔` border row.
        assert!(
            header_row >= 2 && lines[header_row - 2].contains('\u{2581}'),
            "expected a top border row two rows above the selected header:\n{out}"
        );
        let hint_row = header_row + 1;
        assert!(
            lines[hint_row].contains("^P: Play | ^A: Enqueue | ^S: Shuffle"),
            "expected the artist action-hint row directly below the header:\n{out}"
        );
        assert!(
            !lines[hint_row].contains("ENTER"),
            "artist action hint should not include the album's ENTER clause:\n{out}"
        );
        assert!(
            lines[hint_row + 1..]
                .iter()
                .take(4)
                .any(|line| line.contains('\u{2594}')),
            "expected a bottom border row below the selected header block:\n{out}"
        );

        assert_eq!(
            layout.cursor_screen_y,
            Some(header_row as u16),
            "selected header should own the screen cursor row"
        );
    }

    #[test]
    fn music_group_pills_render_on_row_below_title_marker() {
        let mut app = make_power_music_group_app();
        app.power_left_width = 20;
        let width = 100u16;
        let height = 20u16;
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_view(
                f,
                Rect::new(0, 0, width, height),
                &mut layout,
                &mut LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let row0 = out.lines().next().unwrap();
        let _row1 = out.lines().nth(1).unwrap();

        let row3 = out.lines().nth(3).unwrap();

        assert!(
            !row0.contains("Alpha") && !row0.contains("Beta"),
            "expected pills not on the first row:\n{out}"
        );
        assert!(
            row3.contains("Alpha") && row3.contains("Beta"),
            "expected group pills below the tab bar (no header row):\n{out}"
        );

        let _rchar_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.rfind(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };
        let char_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.find(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };

        let right_col_x = app.power_left_width + POWER_VIEW_GAP;
        let buf = term.backend().buffer();
        assert!(
            row3.chars().take(right_col_x as usize).all(|c| c == ' '),
            "expected the pill row to be confined to the right library column:\n{out}"
        );

        let alpha_x = char_x(row3, "Alpha");
        assert!(
            alpha_x >= right_col_x,
            "expected pills confined to the right column"
        );
        assert_eq!(buf[(alpha_x, 3)].bg, palette::YELLOW);
        assert_eq!(
            buf[(alpha_x, 3)].fg,
            palette::PILL_DARK,
            "expected the selected group pill to use dark text"
        );
        let beta_x = char_x(row3, "Beta");
        assert_eq!(buf[(beta_x, 3)].bg, palette::POWER_RIGHT_BG);
        assert_eq!(
            buf[(beta_x, 3)].fg,
            palette::YELLOW,
            "expected a non-selected group pill to use yellow text"
        );

        let (gap_start, gap_end) = (alpha_x.min(beta_x), alpha_x.max(beta_x));
        let between: String = row3
            .chars()
            .skip(gap_start as usize)
            .take((gap_end - gap_start) as usize)
            .collect();
        assert!(
            !between.contains('\u{2501}'),
            "expected a blank gap between adjacent pills, not a dash rule:\n{between:?}"
        );

        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 3, "expected selector hitboxes on the pills row");
            assert!(
                rect.x >= right_col_x,
                "expected selector hitboxes confined to the right column"
            );
        }

        // Row 4 is a blank spacer between the pill row and the album list.
        let spacer_row = out.lines().nth(4).unwrap();
        assert!(
            spacer_row.trim().is_empty(),
            "expected a blank spacer row between the pills and the album list:\n{out}"
        );
        let album_row = out.lines().nth(5).unwrap();
        assert!(
            album_row.contains("Alpha") || album_row.contains("First Album"),
            "expected album list content to start below the pill/spacer rows:\n{out}"
        );
    }

    #[test]
    fn music_group_pills_scroll_within_reserved_space_when_overflowing() {
        let mut app = make_power_music_group_app();
        app.power_left_width = 20;
        let width = 40u16;
        let height = 20u16;
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_view(
                f,
                Rect::new(0, 0, width, height),
                &mut layout,
                &mut LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let _row0 = out.lines().next().unwrap();

        let row3 = out.lines().nth(3).unwrap();
        let _row4 = out.lines().nth(4).unwrap();

        assert!(
            row3.contains('\u{203a}'),
            "expected a right scroll indicator on the pills row (no header gap):\n{out}"
        );

        let rchar_x = |line: &str, needle: &str| -> u16 {
            let byte_idx = line.rfind(needle).expect("needle not found");
            line[..byte_idx].chars().count() as u16
        };

        let right_indicator_x = rchar_x(row3, "\u{203a}");
        assert!(
            right_indicator_x < width,
            "expected the right scroll indicator to stay inside the pill row:\n{out}"
        );

        let right_col_x = (app.power_left_width + POWER_VIEW_GAP) as usize;
        assert!(
            row3.chars().take(right_col_x).all(|c| c == ' '),
            "expected the pill row to be confined to the right library column:\n{out}"
        );

        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 3, "expected pill hitboxes on the pills row");
            assert!(
                rect.x as usize >= right_col_x,
                "expected pill hitboxes confined to the right column"
            );
            assert!(
                rect.x + rect.width <= width,
                "expected pill hitboxes confined to the visible pill row"
            );
        }
    }

    // ── render_power_album_detail refactor (#145) ──────────────────────────
    // `render_power_album_detail` used to read `items`/`cursor` from
    // `nav_stack` internally; it now takes them as explicit parameters so a
    // future inline-detail render path (not wired up yet) can feed it
    // proactively-fetched data instead of a drilled-in nav_stack level. This
    // locks in that the existing drilldown call site (`is_album` branch in
    // `render_power_library`) still renders identically after the refactor.
    #[test]
    fn album_detail_still_renders_from_drilled_in_nav_stack_level() {
        let mut app = make_power_music_group_app();

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.production_year = 2001;
        track.index_number = 1;
        track.runtime_ticks = 200 * mbv_core::api::TICKS_PER_SECOND;

        app.libs[0].nav_stack.push(BrowseLevel {
            parent_id: "album-1".into(),
            title: "First Album".into(),
            items: vec![track],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
        });

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);

        assert!(
            out.contains("First Album"),
            "expected the drilled-in album title to still render via the \
             refactored explicit items/cursor signature:\n{out}"
        );
        assert!(
            out.contains("Opening Track"),
            "expected the drilled-in track list to still render via the \
             refactored explicit items/cursor signature:\n{out}"
        );
    }

    // ── inline album detail at the album-folder-listing level (#145, task 2) ──

    #[test]
    fn album_folder_listing_renders_list_and_inline_detail_together() {
        let mut app = make_power_music_group_app();
        // Sitting at the album-folder-listing level already (no drilldown push).
        assert_eq!(app.libs[0].nav_stack.len(), 2);

        let mut second_album = make_item("Second Album", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Alpha".into();
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(second_album);

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        // In the music-group (pill selector) view, inline tracks only render
        // once track-selection mode has been entered (Enter pressed).
        app.libs[0].album_track_focus = Some(0);

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        assert!(
            out.contains("Alpha"),
            "expected the album list (grouped by artist) to still render:\n{out}"
        );
        assert!(
            out.contains("Opening Track"),
            "expected the selected album's cached tracks to render inline, \
             without any drilldown:\n{out}"
        );

        // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
        // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
        // `─` rule + `▌` gutter.
        let title_y = lines
            .iter()
            .position(|l| l.contains("First Album"))
            .expect("expected selected album row");
        assert!(
            lines[title_y - 2].contains("\u{2581}"),
            "expected a top border two rows above the title (border row, then colored padding row):\n{out}"
        );
        assert!(
            lines[title_y - 1].trim().is_empty(),
            "expected the colored top-padding row directly above the title to be blank:\n{out}"
        );

        let track_y = lines
            .iter()
            .position(|l| l.contains("Opening Track"))
            .expect("expected inline track row");
        assert!(
            track_y > title_y,
            "expected the track row to render below the selected album title:\n{out}"
        );

        let second_album_y = lines
            .iter()
            .position(|l| l.contains("Second Album"))
            .expect("expected the following album row");
        assert!(
            lines[second_album_y - 1].contains("\u{2594}"),
            "expected a bottom border directly above the following album row:\n{out}"
        );
        assert!(
            second_album_y > track_y,
            "expected the following album to render after the inline track detail:\n{out}"
        );

        let title_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(0))
            .expect("expected the selected album (index 0) in the row map");
        let second_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected the following album (index 1) in the row map");
        assert!(
            second_row_idx > title_row_idx,
            "expected the following album's row-map entry after the selected album's"
        );
        assert!(
            layout.left_row_map[title_row_idx + 1..second_row_idx]
                .iter()
                .all(Option::is_none),
            "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
            layout.left_row_map
        );
        assert_eq!(
            app.libs[0].nav_stack.len(),
            2,
            "rendering the inline preview must not push a nav_stack level"
        );
    }

    #[test]
    fn flat_album_folder_listing_renders_inline_detail_under_selected_album() {
        let mut app = make_app_stub();
        app.power_left_tab = 1;
        app.music_levels = vec!["album".into()];

        let mut library = make_item("Music", "CollectionFolder");
        library.id = "lib-music".into();
        library.is_folder = true;
        library.collection_type = "music".into();

        let mut album = make_item("First Album", "MusicAlbum");
        album.id = "album-1".into();
        album.artist = "Alpha".into();
        album.is_folder = true;
        let mut second_album = make_item("Second Album", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Alpha".into();
        second_album.is_folder = true;

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-music".into(),
                title: "Music".into(),
                items: vec![album, second_album],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
        // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
        // `─` rule + `▌` gutter. Structure per block:
        //   [border ▁] [colored padding] [album title] [tracks...] [colored padding] [border ▔]
        let title_y = lines
            .iter()
            .position(|l| l.contains("First Album"))
            .expect("expected selected album title row");
        assert!(
            lines[title_y - 2].contains("\u{2581}"),
            "expected a top border two rows above the title (border row, then colored padding row):\n{out}"
        );
        assert!(
            lines[title_y - 1].trim().is_empty(),
            "expected the colored top-padding row directly above the title to be blank:\n{out}"
        );

        let track_y = lines
            .iter()
            .position(|l| l.contains("Opening Track"))
            .expect("expected inline track row");
        assert!(
            track_y > title_y,
            "expected the track row to render below the selected album title:\n{out}"
        );

        let second_album_y = lines
            .iter()
            .position(|l| l.contains("Second Album"))
            .expect("expected the following album row");
        assert!(
            lines[second_album_y - 1].contains("\u{2594}"),
            "expected a bottom border directly above the following album row:\n{out}"
        );
        assert!(
            second_album_y > track_y,
            "expected the following album to render after the inline track detail:\n{out}"
        );

        // Row-map: only the Album() rows (title + following album) map to a
        // selectable index; every border/padding/track-detail row is `None`.
        let title_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(0))
            .expect("expected the selected album (index 0) in the row map");
        let second_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected the following album (index 1) in the row map");
        assert!(
            second_row_idx > title_row_idx,
            "expected the following album's row-map entry after the selected album's"
        );
        assert!(
            layout.left_row_map[title_row_idx + 1..second_row_idx]
                .iter()
                .all(Option::is_none),
            "expected every row between the two albums (borders, padding, track detail) to be non-selectable:\n{:?}",
            layout.left_row_map
        );
        assert!(
            layout
                .left_row_targets
                .iter()
                .all(|target| !matches!(target, Some(PowerLeftRowTarget::ArtistHeader(_)))),
            "flat/non-custom grouped album headers must remain non-selectable"
        );
    }

    #[test]
    fn inline_album_track_selection_block_hides_its_own_scrollbar() {
        let mut app = make_app_stub();
        let mut tracks = Vec::new();
        for i in 0..20 {
            let mut track = make_item(&format!("Track {i}"), "Audio");
            track.id = format!("track-{i}");
            track.album = "Selected Album".into();
            track.index_number = i + 1;
            tracks.push(track);
        }

        let backend = TestBackend::new(30, 8);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_album_detail(
                f,
                Rect::new(0, 0, 30, 8),
                &tracks,
                12,
                true,
                true,
                true,
                &mut layout,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);

        assert!(
            !out.contains('\u{2590}'),
            "inline track-selection block must not draw its own scrollbar:\n{out}"
        );
    }

    #[test]
    fn album_folder_listing_fetches_and_shows_loading_on_cache_miss() {
        let mut app = make_power_music_group_app();
        let mut second_album = make_item("Second Album", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Alpha".into();
        app.libs[0]
            .nav_stack
            .last_mut()
            .unwrap()
            .items
            .push(second_album);
        assert!(!app.album_tracks_cache.contains_key("album-1"));
        assert!(!app.album_tracks_loading.contains("album-1"));

        // In the music-group (pill selector) view, inline tracks (and the
        // fetch that populates them) only happen once track-selection mode
        // has been entered (Enter pressed).
        app.libs[0].album_track_focus = Some(0);

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        assert!(
            app.album_tracks_loading.contains("album-1"),
            "expected a cache miss to trigger fetch_album_tracks for the \
             selected album:\n{out}"
        );
        assert!(
            out.to_lowercase().contains("loading"),
            "expected a loading indicator in the detail pane while the \
             fetch is in flight:\n{out}"
        );
        // Selection now reads via a colored MEDIA_SELECTED_BG block framed by
        // ▁/▔ unicode borders (movie-tab colored-block style), not the legacy
        // `─` rule + `▌` gutter.
        let title_y = lines
            .iter()
            .position(|l| l.contains("First Album"))
            .expect("expected selected album row");
        assert!(
            lines[title_y - 2].contains("\u{2581}"),
            "expected a top border two rows above the title (border row, then colored padding row):\n{out}"
        );
        assert!(
            lines[title_y - 1].trim().is_empty(),
            "expected the colored top-padding row directly above the title to be blank:\n{out}"
        );

        let loading_y = lines
            .iter()
            .position(|l| l.to_lowercase().contains("loading"))
            .expect("expected an inline loading row");
        assert!(
            loading_y > title_y,
            "expected the loading row to render below the selected album title:\n{out}"
        );

        let second_album_y = lines
            .iter()
            .position(|l| l.contains("Second Album"))
            .expect("expected the following album row");
        assert!(
            lines[second_album_y - 1].contains("\u{2594}"),
            "expected a bottom border directly above the following album row:\n{out}"
        );
        assert!(
            second_album_y > loading_y,
            "expected the following album to render after the inline loading row:\n{out}"
        );

        let title_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(0))
            .expect("expected the selected album (index 0) in the row map");
        let second_row_idx = layout
            .left_row_map
            .iter()
            .position(|r| *r == Some(1))
            .expect("expected the following album (index 1) in the row map");
        assert!(
            second_row_idx > title_row_idx,
            "expected the following album's row-map entry after the selected album's"
        );
        assert!(
            layout.left_row_map[title_row_idx + 1..second_row_idx]
                .iter()
                .all(Option::is_none),
            "expected every row between the two albums (borders, padding, loading row) to be non-selectable:\n{:?}",
            layout.left_row_map
        );
    }

    #[test]
    fn album_folder_inline_detail_is_hidden_until_track_selection_mode() {
        let mut app = make_power_music_group_app();

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutPower::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let lines: Vec<&str> = out.lines().collect();
        let buf = term.backend().buffer();

        assert_eq!(
            lines
                .iter()
                .filter(|line| line.contains("First Album"))
                .count(),
            1,
            "expected no duplicate inline album title row:\n{out}"
        );

        assert!(
            !out.contains("Opening Track"),
            "expected inline tracks to stay hidden until track-selection mode is entered \
             (Enter pressed):\n{out}"
        );

        let hint_y = lines
            .iter()
            .position(|line| line.contains("^P: Play"))
            .expect("expected inline action hint row");
        assert!(
            // The full hint text is wider than this fixture's terminal, so
            // it's truncated with an ellipsis -- just check for the
            // still-visible prefix.
            lines[hint_y].contains("ENTER: Show"),
            "expected the collapsed hint row to prompt Enter to show tracks:\n{out}"
        );
        let hint_x = lines[hint_y]
            .find("^P: Play")
            .expect("expected hint x position");
        assert!(
            lines[hint_y].starts_with("    ^P: Play"),
            "expected collapsed hint content to align with the selected block title indent:\n{out}"
        );
        assert_eq!(
            buf[(hint_x as u16, hint_y as u16)].fg,
            palette::SOFT_WHITE,
            "expected inline action hints to render soft white:\n{out}"
        );
    }

    #[test]
    fn selected_music_group_album_shows_right_aligned_art_before_track_mode() {
        let mut app = make_power_music_group_app();
        app.image_protocol_enabled = true;

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutPower::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let art_rect = layout
            .inline_image_rect
            .expect("expected selected album art rect before track mode");

        assert!(
            !out.contains("Opening Track"),
            "tracks should stay hidden until track-selection mode:\n{out}"
        );
        assert_eq!(
            art_rect.x + art_rect.width,
            58,
            "album art should have two columns of right padding"
        );
        assert_eq!((art_rect.width, art_rect.height), (24, 12));
        assert!(app.card_image_loading.contains("album-1:P"));
        assert!(!app.card_image_loading.contains("track-1:P"));
        assert_eq!(
            term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
            palette::OVERLAY,
            "loading album art should reserve a right-aligned placeholder:\n{out}"
        );
    }

    #[test]
    fn selected_music_group_album_keeps_right_aligned_art_in_track_mode() {
        let mut app = make_power_music_group_app();
        app.image_protocol_enabled = true;
        app.libs[0].album_track_focus = Some(0);

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutPower::default();
        let term = render_power_library_to_terminal(&mut app, &mut layout);
        let out = buffer_to_string(&term);
        let art_rect = layout
            .inline_image_rect
            .expect("expected selected album art rect in track mode");

        assert!(
            out.contains("Opening Track"),
            "expected inline track row:\n{out}"
        );
        let lines: Vec<&str> = out.lines().collect();
        let hint_y = lines
            .iter()
            .position(|line| line.contains("^P: Play"))
            .expect("expected track-mode action hint row");
        let track_y = lines
            .iter()
            .position(|line| line.contains("Opening Track"))
            .expect("expected inline track row");
        assert!(
            lines[hint_y].starts_with("    ^P: Play"),
            "expected track-mode hint content to keep the selected block title indent:\n{out}"
        );
        assert!(
            lines[hint_y + 1].trim().is_empty(),
            "expected a blank row between the track-mode hint and tracks:\n{out}"
        );
        assert_eq!(
            track_y,
            hint_y + 2,
            "expected the track list to start after the hint and blank separator row:\n{out}"
        );
        let hint_x = lines[hint_y]
            .find("^P: Play")
            .expect("expected track-mode hint x position");
        assert_eq!(
            term.backend().buffer()[(hint_x as u16, hint_y as u16)].fg,
            palette::SOFT_WHITE,
            "expected track-mode action hints to render soft white:\n{out}"
        );
        assert_eq!(
            art_rect.x + art_rect.width,
            58,
            "album art should have two columns of right padding"
        );
        assert_eq!((art_rect.width, art_rect.height), (24, 12));
        assert!(app.card_image_loading.contains("album-1:P"));
        assert!(!app.card_image_loading.contains("track-1:P"));
        assert_eq!(
            term.backend().buffer()[(art_rect.x, art_rect.y)].bg,
            palette::OVERLAY,
            "loading album art should reserve a right-aligned placeholder:\n{out}"
        );
    }

    #[test]
    fn album_folder_inline_detail_keeps_title_gutter_when_library_pane_unfocused() {
        // Selection now reads via the colored MEDIA_SELECTED_BG block + YELLOW
        // title text (the movie-tab colored-block style), not the legacy `▌`
        // marker -- confirm that styling survives losing pane focus.
        let mut app = make_power_music_group_app();

        let mut track = make_item("Opening Track", "Audio");
        track.id = "track-1".into();
        track.album = "First Album".into();
        track.artist = "Alpha".into();
        track.index_number = 1;
        app.album_tracks_cache.insert("album-1".into(), vec![track]);

        let mut layout = LayoutPower::default();
        let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
        let out = buffer_to_string(&term);
        let title_y = out
            .lines()
            .position(|line| line.contains("First Album"))
            .expect("expected selected album title row");
        let title_line = out.lines().nth(title_y).unwrap();
        let title_x = title_line
            .find("First Album")
            .expect("expected title text position") as u16;

        let buf = term.backend().buffer();
        assert_eq!(
            buf[(title_x, title_y as u16)].bg,
            palette::MEDIA_SELECTED_BG,
            "selected album title row should keep the colored block background while unfocused:\n{out}"
        );
        assert_eq!(
            buf[(title_x, title_y as u16)].fg,
            palette::YELLOW,
            "selected album title should keep its YELLOW text while unfocused:\n{out}"
        );
    }

    #[test]
    fn album_folder_listing_preserves_inline_track_focus_cursor() {
        let mut app = make_power_music_group_app();
        app.libs[0].album_track_focus = Some(1);

        let mut first = make_item("Opening Track", "Audio");
        first.id = "track-1".into();
        first.album = "First Album".into();
        first.artist = "Alpha".into();
        first.index_number = 1;

        let mut second = make_item("Focused Track", "Audio");
        second.id = "track-2".into();
        second.album = "First Album".into();
        second.artist = "Alpha".into();
        second.index_number = 2;

        app.album_tracks_cache
            .insert("album-1".into(), vec![first, second]);

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);
        let focused_line = out
            .lines()
            .find(|line| line.contains("Focused Track"))
            .expect("expected focused track to render inline");
        let focused_y = out
            .lines()
            .position(|line| line.contains("Focused Track"))
            .expect("expected focused track row");
        let lines: Vec<&str> = out.lines().collect();
        let hint_y = lines
            .iter()
            .position(|line| line.contains("^P: Play"))
            .expect("expected track-mode action hint row");
        assert!(
            lines[hint_y].contains("BACK: Exit"),
            "expected track-mode hint row to show the exit hint:\n{out}"
        );
        assert!(
            lines[hint_y + 1].trim().is_empty(),
            "expected a blank row between the track-mode hint and tracks:\n{out}"
        );
        assert_eq!(
            focused_y,
            hint_y + 3,
            "expected second track after hint, blank separator, and first track:\n{out}"
        );

        assert!(
            // Track rows are indented two extra spaces inside the selected block;
            // the AQUA `▌` cursor marker still sits directly against the track
            // number, no trailing space.
            focused_line.starts_with("    \u{258c}2. Focused Track"),
            "expected focused track row to show the AQUA cursor marker in track-selection mode:\n{out}"
        );
        assert_eq!(
            layout.cursor_screen_y,
            Some(focused_y as u16),
            "expected layout cursor to follow the focused inline track row"
        );
    }

    #[test]
    fn album_folder_track_focus_cursor_renders_when_library_pane_unfocused() {
        let mut app = make_power_music_group_app();
        app.libs[0].album_track_focus = Some(1);

        let mut first = make_item("Opening Track", "Audio");
        first.id = "track-1".into();
        first.album = "First Album".into();
        first.artist = "Alpha".into();
        first.index_number = 1;

        let mut second = make_item("Focused Track", "Audio");
        second.id = "track-2".into();
        second.album = "First Album".into();
        second.artist = "Alpha".into();
        second.index_number = 2;

        app.album_tracks_cache
            .insert("album-1".into(), vec![first, second]);

        let mut layout = LayoutPower::default();
        let term = render_power_library_to_terminal_focused(&mut app, &mut layout, false);
        let out = buffer_to_string(&term);
        let focused_line = out
            .lines()
            .find(|line| line.contains("Focused Track"))
            .expect("expected focused track to render inline");

        assert!(
            // Track rows are indented two extra spaces inside the selected block;
            // the AQUA `▌` cursor marker still sits directly against the track
            // number, no trailing space.
            focused_line.starts_with("    \u{258c}2. Focused Track"),
            "expected track-selection row to show the AQUA cursor marker while pane is unfocused:\n{out}"
        );
    }

    #[test]
    fn selected_album_item_follows_raw_cursor_not_display_order() {
        let mut app = make_power_music_group_app();

        // A second album whose artist sorts before "Alpha" -- if the cursor
        // were (mis)interpreted against the artist-grouped display order
        // instead of the raw `items` array, moving the cursor to 1 would
        // resolve to the wrong album here.
        let mut second_album = make_item("Zero Day", "MusicAlbum");
        second_album.id = "album-2".into();
        second_album.artist = "Aaardvark".into();

        {
            let lvl = app.libs[0].nav_stack.last_mut().unwrap();
            lvl.items.push(second_album);
            lvl.cursor = 1;
        }

        let selected = app
            .selected_album_item(0)
            .expect("expected a selected album at cursor 1");
        assert_eq!(
            selected.id, "album-2",
            "expected the raw items[cursor] entry, not a sorted/display-order lookup"
        );

        // In the music-group (pill selector) view, the inline-detail fetch
        // (and thus this test's target assertion) only happens once
        // track-selection mode has been entered.
        app.libs[0].album_track_focus = Some(0);

        let mut layout = LayoutPower::default();
        let _ = render_power_library_to_string(&mut app, &mut layout);
        assert!(
            app.album_tracks_loading.contains("album-2"),
            "expected the fetch triggered by rendering to target the cursor-selected \
             album (album-2), not album-1"
        );
        assert!(
            !app.album_tracks_loading.contains("album-1"),
            "album-1 is no longer selected, so it should not be (re)fetched"
        );
    }

    // ── #145 task 5: regression coverage for non-music Power View surfaces ──
    // `is_viewing_album_folders`/`is_album_level` both gate on
    // `collection_type == "music"`, so these are provably unreachable for
    // series/home-video libraries; the tests below additionally prove the
    // *render* path (`render_power_library`) still picks the original
    // single-pane series/home-video renderer and never touches the new
    // album-tracks cache/track-focus machinery added in tasks 1-4.

    fn make_power_series_app() -> App {
        let mut app = make_app_stub();
        app.power_left_tab = 1;

        let mut library = make_item("Shows", "CollectionFolder");
        library.id = "lib-shows".into();
        library.is_folder = true;
        library.collection_type = "tvshows".into();

        let mut season = make_item("Season 1", "Season");
        season.id = "season-1".into();

        let mut ep1 = make_item("Pilot", "Episode");
        ep1.id = "ep-1".into();
        let mut ep2 = make_item("Second Episode", "Episode");
        ep2.id = "ep-2".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![
                BrowseLevel {
                    parent_id: "lib-shows".into(),
                    title: "Seasons".into(),
                    items: vec![season],
                    total_count: 1,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                },
                BrowseLevel {
                    parent_id: "season-1".into(),
                    title: "Episodes".into(),
                    items: vec![ep1, ep2],
                    total_count: 2,
                    cursor: 0,
                    scroll: 0,
                    item_types: None,
                    unplayed_only: false,
                    sort_by: "SortName".into(),
                    sort_order: "Ascending".into(),
                    loading: false,
                    all_items: None,
                    letter_filter: None,
                },
            ],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    fn make_power_home_video_app() -> App {
        let mut app = make_app_stub();
        app.power_left_tab = 1;

        let mut library = make_item("Home Videos", "CollectionFolder");
        library.id = "lib-homevideos".into();
        library.is_folder = true;
        library.collection_type = "homevideos".into();

        let mut first = make_item("Birthday Clip", "Video");
        first.id = "video-1".into();
        let mut second = make_item("Vacation Clip", "Video");
        second.id = "video-2".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-homevideos".into(),
                title: "Home Videos".into(),
                items: vec![first, second],
                total_count: 2,
                cursor: 0,
                scroll: 0,
                item_types: None,
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: None,
        });

        app
    }

    #[test]
    fn home_video_library_is_never_album_folders_and_renders_via_original_list_path() {
        let mut app = make_power_home_video_app();
        let lib_idx = 0;

        assert!(
            !app.is_viewing_album_folders(lib_idx),
            "a homevideos library must never satisfy is_viewing_album_folders"
        );
        assert!(!app.is_album_level(lib_idx));
        assert!(app.is_home_video_view(lib_idx));
        assert!(app.libs[lib_idx].album_track_focus.is_none());

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);

        assert!(
            out.contains("Birthday Clip"),
            "expected the original single-pane home-video list renderer to fire \
             unchanged:\n{out}"
        );
        assert!(
            app.album_tracks_cache.is_empty(),
            "home-video rendering must never touch the album-tracks cache added by #145"
        );
        assert!(
            app.libs[lib_idx].album_track_focus.is_none(),
            "home-video rendering must never set track-selection mode"
        );
    }

    #[test]
    fn letter_filter_buckets_match_emby_name_range_bounds() {
        // Verified empirically against a live Emby server (2026-07-22) that
        // NameStartsWithOrGreater/NameLessThan filter on SortName -- these
        // bounds must stay in lockstep with `letter_bucket`'s range labels.
        let ac = LetterFilter::for_index(0).unwrap();
        assert_eq!(ac.label, "A\u{2013}C");
        assert_eq!(ac.name_ge, Some("A"));
        assert_eq!(ac.name_lt, Some("D"));

        let vz = LetterFilter::for_index(7).unwrap();
        assert_eq!(vz.label, "V\u{2013}Z");
        assert_eq!(vz.name_ge, Some("V"));
        assert_eq!(vz.name_lt, None, "V–Z has no upper bound");

        let hash = LetterFilter::for_index(8).unwrap();
        assert_eq!(hash.label, "#");
        assert_eq!(hash.name_ge, None, "# has no lower bound");
        assert_eq!(hash.name_lt, Some("A"));

        assert!(LetterFilter::for_index(9).is_none());
        assert_eq!(LetterFilter::count(), 9);
        assert_eq!(LetterFilter::labels().len(), 9);
    }

    #[test]
    fn letter_filter_default_is_the_first_bucket() {
        assert_eq!(
            LetterFilter::default_filter(),
            LetterFilter::for_index(0).unwrap()
        );
    }

    fn make_power_large_movie_library_app(library_total: usize) -> App {
        let mut app = make_app_stub();
        app.power_left_tab = 1;

        let mut library = make_item("Movies", "CollectionFolder");
        library.id = "lib-movies".into();
        library.is_folder = true;
        library.collection_type = "movies".into();

        app.libs.push(LibraryTab {
            library,
            nav_stack: vec![BrowseLevel {
                parent_id: "lib-movies".into(),
                title: "Movies".into(),
                items: Vec::new(),
                total_count: 0,
                cursor: 0,
                scroll: 0,
                item_types: Some("Movie".into()),
                unplayed_only: false,
                sort_by: "SortName".into(),
                sort_order: "Ascending".into(),
                loading: false,
                all_items: None,
                letter_filter: None,
            }],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            series_selection: None,
            series_season_cursor: 0,
            library_total: Some(library_total),
        });

        app
    }

    #[test]
    fn letter_pills_show_only_when_library_total_exceeds_threshold() {
        let mut small = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD);
        assert!(
            !small.should_show_letter_pills(0),
            "exactly the threshold must not qualify"
        );

        let mut large = make_power_large_movie_library_app(LIBRARY_PILL_THRESHOLD + 1);
        assert!(large.should_show_letter_pills(0));

        // `render_power_library_to_string` calls `render_power_library`
        // directly, which is *below* the pill-row layout carve-out (that
        // lives in `render_power_view`, mirroring the music-group pills
        // row) -- go through the full view so the carve-out fires.
        let backend = TestBackend::new(60, 20);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            large.render_power_view(
                f,
                Rect::new(0, 0, 60, 20),
                &mut layout,
                &mut crate::app::layout::LayoutPlayback::default(),
                &mut Rect::default(),
                &mut Rect::default(),
                0,
                false,
                &None,
            );
        })
        .unwrap();
        let out = buffer_to_string(&term);
        assert!(
            out.contains("A\u{2013}C"),
            "expected the default A–C pill to render:\n{out}"
        );
        assert!(
            !layout.selector_tabs.is_empty(),
            "expected pill hitboxes to be recorded for click dispatch"
        );

        // Rendering the small (non-qualifying) library must not show pills.
        let backend2 = TestBackend::new(60, 20);
        let mut term2 = Terminal::new(backend2).unwrap();
        let mut layout2 = LayoutPower::default();
        term2
            .draw(|f| {
                small.render_power_view(
                    f,
                    Rect::new(0, 0, 60, 20),
                    &mut layout2,
                    &mut crate::app::layout::LayoutPlayback::default(),
                    &mut Rect::default(),
                    &mut Rect::default(),
                    0,
                    false,
                    &None,
                );
            })
            .unwrap();
        let out2 = buffer_to_string(&term2);
        assert!(!out2.contains("A\u{2013}C"));
    }
}
