use super::super::ui_util::{build_queue_rows, QueueRow};
use super::chrome::thin_vertical_thumb;
use super::parse_album_folder_name;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use mbv_core::api::MediaItem;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Paragraph};
use ratatui::Frame;
use tui_scrollbar::{GlyphSet, ScrollBar, ScrollLengths};
use unicode_width::UnicodeWidthStr;

// Power View re-renders frequently while scrolling; prefer a cheaper filter in
// these hot paths to reduce terminal image preparation stalls.
pub(super) const POWER_RENDER_FILTER: ratatui_image::FilterType =
    ratatui_image::FilterType::Triangle;

// Configured music albums need the image worker's child-audio lookup; their
// album containers do not reliably expose usable Primary images.
pub(super) const MUSIC_ALBUM_IMAGE_TYPES: &[&str] = &["AudioChild"];

/// Columns of empty space between the left and right panels in power view.
pub(super) const POWER_VIEW_GAP: u16 = 0;

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

pub(super) fn power_right_panel_content_area(area: Rect, left_collapsed: bool) -> Rect {
    if left_collapsed {
        Rect {
            width: area.width.saturating_sub(1),
            ..area
        }
    } else {
        Rect {
            x: area.x + POWER_TAB_LEFT_PAD,
            width: area
                .width
                .saturating_sub(POWER_TAB_LEFT_PAD.saturating_mul(2)),
            ..area
        }
    }
}

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

pub(super) fn render_power_scrollbar_with_viewport(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) {
    render_power_scrollbar_with_viewport_at(
        f,
        area,
        content_length,
        viewport_content_length,
        offset,
        area.x + area.width.saturating_sub(1),
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SCROLLBAR,
    );
}

pub(super) fn render_power_right_scrollbar(
    f: &mut Frame,
    area: Rect,
    max_offset: usize,
    offset: usize,
) {
    let visible = area.height as usize;
    let x = if area.right() < f.area().right() {
        area.right()
    } else {
        area.x + area.width.saturating_sub(1)
    };
    render_power_scrollbar_with_viewport_at(
        f,
        area,
        max_offset.saturating_add(visible),
        visible,
        offset,
        x,
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SCROLLBAR,
    );
}

pub(super) fn render_power_right_scrollbar_with_viewport(
    f: &mut Frame,
    area: Rect,
    content_length: usize,
    viewport_content_length: usize,
    offset: usize,
) {
    let x = if area.right() < f.area().right() {
        area.right()
    } else {
        area.x + area.width.saturating_sub(1)
    };
    render_power_scrollbar_with_viewport_at(
        f,
        area,
        content_length,
        viewport_content_length,
        offset,
        x,
        thin_vertical_thumb(GlyphSet::minimal()),
        palette::SCROLLBAR,
    );
}

fn render_power_scrollbar_with_viewport_at(
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

/// Paints the ▁/▔ border rows on the reserved rows one position outside
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
    let border_style = Style::default().fg(palette::SEEK_TRACK);
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

pub(super) fn render_power_queue_panel_frame(f: &mut Frame, area: Rect, focused: bool) -> Rect {
    if area.width == 0 || area.height == 0 {
        return Rect::default();
    }

    let bg = if focused {
        palette::MEDIA_SELECTED_BG
    } else {
        palette::LIBRARY_SIDE_BG
    };
    f.render_widget(Block::default().style(Style::default().bg(bg)), area);

    let border_style = Style::default().fg(palette::SEEK_TRACK);
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

    Rect {
        y: area.y + 1,
        height: area.height.saturating_sub(border_rows),
        ..area
    }
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
            .bg(palette::LIBRARY_SIDE_BG)
    }
}

/// Draws the shared " {count} items" header (SUBTLE) on the first row of
/// `area` and returns `area` shrunk by that one row, so callers can render
/// their list into the remaining space. Used by the home-video tab to keep
/// the label styling and the one-row consumption identical to other tabs
/// that once shared it (movies/tv show library lists no longer show this
/// row; see `render_power_list`).
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

impl App {
    pub(super) fn render_power_library(
        &mut self,
        f: &mut Frame,
        area: Rect,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        // If a music-group library's nav_stack was truncated to just the group
        // level (e.g., stale breadcrumb click), immediately re-push the album level.
        if self.library_tab > 0 {
            self.ensure_music_group_album_level(self.library_tab - 1);
            self.ensure_feed_home_video_group_level(self.library_tab - 1);
        }

        if self.library_tab == 0 {
            self.render_power_home_list(f, area, focused, layout);
            return;
        }
        let lib_idx = self.library_tab.saturating_sub(1);
        let is_feed_group = self.library_tab > 0 && self.is_feed_home_video_group_view(lib_idx);
        let is_music_group = self.library_tab > 0 && self.is_music_group_view(lib_idx);
        let is_album_folders = self.library_tab > 0 && self.is_viewing_album_folders(lib_idx);
        let is_home_video = self.library_tab > 0 && self.is_home_video_view(lib_idx);
        if is_feed_group {
            self.render_power_feed_home_video_group_view(f, area, lib_idx, focused, layout);
        } else if is_album_folders && is_music_group {
            self.render_power_music_group_view(f, area, lib_idx, focused, layout);
        } else if is_album_folders {
            self.render_power_list(f, area, focused, layout);
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
