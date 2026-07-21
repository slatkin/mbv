mod album;
mod card;
mod detail;
mod episode;
mod home;
mod list;
mod music;
mod queue;

use super::super::layout::LayoutPower;
use super::super::ui_util::natural_sort_key;
use super::super::{palette, App, PowerFocus};
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
use ratatui::Frame;

// Power View re-renders frequently while scrolling; prefer a cheaper filter in
// these hot paths to reduce terminal image preparation stalls.
pub(super) const POWER_RENDER_FILTER: ratatui_image::FilterType =
    ratatui_image::FilterType::Triangle;

/// Columns of empty space between the left and right panels in power view.
const POWER_VIEW_GAP: u16 = 0;

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

/// Style for a selector pill (group/section/artist tab row): IRIS text +
/// BOLD on GREEN when selected, PILL text on GREEN otherwise. Shared by
/// every power-view pill row (home's group/section pills, music's group
/// pills) so they can't drift apart on the selected-vs-unselected look.
pub(super) fn selector_pill_style(selected: bool) -> Style {
    if selected {
        Style::default()
            .fg(palette::IRIS)
            .bg(palette::GREEN)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(palette::PILL).bg(palette::GREEN)
    }
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
        let left_w = self.power_left_width;
        let right_w = area.width.saturating_sub(left_w);

        // Header row removed — the tab bar above indicates current location.
        layout.breadcrumbs = Vec::new();
        layout.selector_tabs = Vec::new();
        let content_h = area.height;
        let left_area = Rect {
            x: area.x,
            y: area.y,
            width: left_w,
            height: content_h,
        };

        // Full-column background behind the card image and queue list.
        f.render_widget(
            Block::default().style(Style::default().bg(palette::CONTINUE_BG)),
            left_area,
        );

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
        self.render_power_queue_title(
            f,
            Rect {
                x: left_area.x + 1,
                y: left_area.y + 1,
                width: left_area.width.saturating_sub(2),
                height: 1,
            },
            layout,
        );
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

        // The card fills the top of the left column; the queue list takes the rows
        // below it. At low heights the card can consume most of the column, so relocate
        // the queue under the library on the right instead of cramming it in.
        let (card_h, _) = self.render_power_card(f, card_area);
        let left_remaining = left_content.height.saturating_sub(card_h);

        const MIN_LIST_ROWS: u16 = 6;
        // Hysteresis band: the card's rendered height can shift by a handful of
        // rows from one frame to the next purely because a different image just
        // finished loading (e.g. a season poster vs. an episode thumbnail have
        // very different aspect ratios). Without a dead zone, that alone could
        // flip the relocation decision and reflow the entire right-panel text,
        // making it look like the whole screen redraws in time with the image.
        const HYSTERESIS_ROWS: u16 = 4;
        let relocate_threshold = if self.power_queue_relocated {
            MIN_LIST_ROWS + HYSTERESIS_ROWS
        } else {
            MIN_LIST_ROWS
        };
        self.power_queue_relocated = left_remaining < relocate_threshold;
        let (lib_area, queue_area) = if self.power_queue_relocated {
            // Not enough room for the queue in the left column -- split the right column:
            // library on top, relocated queue at the bottom.
            let h = right_area.height;
            let min_q = MIN_LIST_ROWS.min(h);
            let max_q = h.saturating_sub(MIN_LIST_ROWS).max(min_q);
            let queue_h = (h / 3).clamp(min_q, max_q);
            let lib_h = h.saturating_sub(queue_h);
            // Render title (with scope pills) at the top of the relocated queue area.
            self.render_power_queue_title(
                f,
                Rect {
                    height: 1,
                    ..right_area
                },
                layout,
            );
            (
                Rect {
                    height: lib_h,
                    ..right_area
                },
                Rect {
                    y: right_area.y + lib_h,
                    height: queue_h,
                    ..right_area
                },
            )
        } else {
            // Normal mode: queue in left column below card,
            // library fills the entire right column.
            (
                right_area,
                Rect {
                    y: left_content.y + card_h,
                    height: left_remaining,
                    ..left_content
                },
            )
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
        }

        // Queue list: title row at top, rest is the list.
        let queue_list_area = Rect {
            y: queue_area.y,
            height: queue_area.height,
            ..queue_area
        };
        self.render_power_queue(f, queue_list_area, queue_focused, layout);
        self.render_power_library(f, render_lib_area, left_focused, layout);

        // Status bar + toast overlay at the bottom of the right panel.
        if status_area.width > 0 {
            self.render_status_bar(f, status_area, playback);
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
        let is_series = self.power_left_tab > 0 && self.is_series_view(lib_idx);
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
            self.render_power_album_detail(f, area, &items, cursor, focused, false, layout);
        } else if is_series {
            self.render_power_episode_detail(f, area, lib_idx, focused, layout);
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
    use crate::app::{BrowseLevel, LibraryTab};
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

    fn render_power_view(app: &mut App, width: u16, height: u16) -> LayoutPower {
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
        layout
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
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
        });

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
                },
            ],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
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
        let header = out
            .lines()
            .find(|line| line.contains("Alpha"))
            .expect("expected Alpha header");

        assert!(
            !header.contains('\u{258c}'),
            "selected artist header should no longer render the left focus gutter:\n{out}"
        );
        assert!(
            header.contains('\u{f037b}'),
            "selected artist header should render the trailing focus icon:\n{out}"
        );
        assert_eq!(
            layout.cursor_screen_y,
            Some(0),
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
        assert_eq!(buf[(alpha_x, 3)].bg, palette::GREEN);
        assert_eq!(
            buf[(alpha_x, 3)].fg,
            palette::IRIS,
            "expected the selected group pill to use iris text"
        );
        let beta_x = char_x(row3, "Beta");
        assert_eq!(buf[(beta_x, 3)].bg, palette::GREEN);
        assert_eq!(
            buf[(beta_x, 3)].fg,
            palette::PILL,
            "expected a non-selected group pill to stay green with pill text"
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
        assert!(
            lines[1].contains("\u{2500}"),
            "expected top rule directly above selected album row:\n{out}"
        );
        assert!(
            lines[2].contains("First Album"),
            "expected selected album row to stay in the list flow:\n{out}"
        );
        assert!(
            lines[2].starts_with("  \u{258c} ") && !lines[2].starts_with("  \u{258c}   "),
            "expected selected album row to drop the grouped indent after the gutter:\n{out}"
        );
        assert!(
            lines[3].contains("^P: Play | ^A: Enqueue | ^S: Shuffle"),
            "expected action hints directly under selected album title:\n{out}"
        );
        assert!(
            lines[3].starts_with("  \u{258c} "),
            "expected action hints to share the selected-region gutter:\n{out}"
        );
        assert!(
            lines[5].contains("Opening Track"),
            "expected selected album tracks inside the inline detail region:\n{out}"
        );
        assert!(
            lines[5].starts_with("  \u{258c} 1. Opening Track"),
            "expected inline tracks to share the selected-region gutter:\n{out}"
        );
        assert!(
            lines[6].contains("\u{2500}"),
            "expected bottom rule directly after inline detail:\n{out}"
        );
        assert!(
            lines[7].contains("Second Album"),
            "expected following album row immediately after reserved inline detail rows:\n{out}"
        );
        assert_eq!(
            layout.left_row_map.get(1),
            Some(&None),
            "expected top rule row to be non-selectable"
        );
        assert_eq!(
            layout.left_row_map.get(2),
            Some(&Some(0)),
            "expected selected album row to map to its album index"
        );
        assert!(
            layout.left_row_map[3..7].iter().all(Option::is_none),
            "expected inline detail and bottom-rule rows to be non-selectable"
        );
        assert_eq!(
            layout.left_row_map.get(7),
            Some(&Some(1)),
            "expected album row after inline detail to remain selectable"
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
            }],
            search: None,
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
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

        assert!(
            lines[2].contains("\u{2500}"),
            "expected top rule directly above selected album row:\n{out}"
        );
        assert!(
            lines[3].contains("First Album"),
            "expected selected album row to stay in the flat list flow:\n{out}"
        );
        assert!(
            lines[3].starts_with("  \u{258c} First Album"),
            "expected flat selected album title to align after a one-column gutter gap:\n{out}"
        );
        assert!(
            lines[4].contains("^P: Play | ^A: Enqueue | ^S: Shuffle"),
            "expected action hints directly under selected album title:\n{out}"
        );
        assert!(
            lines[5].starts_with("  \u{258c}"),
            "expected the spacer row between hints and tracks to keep the selected-region gutter:\n{out}"
        );
        assert!(
            lines[6].contains("Opening Track"),
            "expected tracks inside the inline detail region:\n{out}"
        );
        assert!(
            lines[6].starts_with("  \u{258c} 1. Opening Track"),
            "expected inline tracks to share the selected-region gutter:\n{out}"
        );
        assert!(
            lines[7].contains("\u{2500}"),
            "expected bottom rule directly after inline detail:\n{out}"
        );
        assert!(
            lines[8].contains("Second Album"),
            "expected following album row immediately after inline detail:\n{out}"
        );
        assert_eq!(layout.left_row_map.get(1), Some(&None));
        assert_eq!(layout.left_row_map.get(2), Some(&Some(0)));
        assert!(layout.left_row_map[3..7].iter().all(Option::is_none));
        assert_eq!(layout.left_row_map.get(7), Some(&Some(1)));
        assert!(
            layout
                .left_row_targets
                .iter()
                .all(|target| !matches!(target, Some(PowerLeftRowTarget::ArtistHeader(_)))),
            "flat/non-custom grouped album headers must remain non-selectable"
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
        assert!(
            lines[1].contains("\u{2500}"),
            "expected top rule directly above selected album row:\n{out}"
        );
        assert!(
            lines[2].contains("First Album"),
            "expected selected album row to stay in the list flow:\n{out}"
        );
        assert!(
            lines[3].to_lowercase().contains("loading"),
            "expected loading row directly under the selected album row:\n{out}"
        );
        assert!(
            lines[3].starts_with("  \u{258c} Loading"),
            "expected inline loading row to share the selected-region gutter:\n{out}"
        );
        assert!(
            lines[4].contains("\u{2500}"),
            "expected bottom rule directly after inline loading row:\n{out}"
        );
        assert!(
            lines[5].contains("Second Album"),
            "expected following album row directly after inline loading row:\n{out}"
        );
        assert_eq!(layout.left_row_map.get(1), Some(&None));
        assert_eq!(layout.left_row_map.get(2), Some(&Some(0)));
        assert_eq!(layout.left_row_map.get(3), Some(&None));
        assert_eq!(layout.left_row_map.get(4), Some(&None));
        assert_eq!(layout.left_row_map.get(5), Some(&Some(1)));
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
        assert_eq!(
            buf[(hint_x as u16, hint_y as u16)].fg,
            palette::MUTED,
            "expected inline action hints to render muted:\n{out}"
        );
    }

    #[test]
    fn album_folder_inline_detail_keeps_title_gutter_when_library_pane_unfocused() {
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
        let title_line = out
            .lines()
            .find(|line| line.contains("First Album"))
            .expect("expected selected album title row");

        assert_eq!(
            title_line.find('▌'),
            Some(2),
            "selected album title row should keep the selected-region gutter while unfocused:\n{out}"
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

        assert!(
            focused_line.starts_with("  \u{258c} 2. Focused Track"),
            "expected focused track row to keep the green selected-row gutter in track-selection mode:\n{out}"
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
            focused_line.starts_with("  \u{258c} 2. Focused Track"),
            "expected track-selection row to keep the green selected-row gutter while pane is unfocused:\n{out}"
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
                },
            ],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
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
            }],
            search: None,
            feed_home_video: None,

            album_track_focus: None,
            artist_header_focus: None,
        });

        app
    }

    #[test]
    fn series_library_is_never_album_folders_and_renders_via_episode_detail_path() {
        let mut app = make_power_series_app();
        let lib_idx = 0;

        assert!(
            !app.is_viewing_album_folders(lib_idx),
            "a tvshows library must never satisfy is_viewing_album_folders (gated \
             on collection_type == \"music\")"
        );
        assert!(
            !app.is_album_level(lib_idx),
            "a tvshows library must never satisfy is_album_level either"
        );
        assert!(app.is_series_view(lib_idx));
        assert!(app.libs[lib_idx].album_track_focus.is_none());

        let mut layout = LayoutPower::default();
        let out = render_power_library_to_string(&mut app, &mut layout);

        assert!(
            out.contains("Pilot"),
            "expected the original single-pane episode-detail renderer to fire \
             unchanged:\n{out}"
        );
        assert!(
            app.album_tracks_cache.is_empty(),
            "series rendering must never touch the album-tracks cache added by #145"
        );
        assert!(
            app.libs[lib_idx].album_track_focus.is_none(),
            "series rendering must never set track-selection mode"
        );
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
}
