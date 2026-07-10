mod album;
mod card;
mod detail;
mod episode;
mod home;
mod list;
mod music;
mod queue;

use super::super::layout::LayoutPower;
use super::super::ui_util::{natural_sort_key, trunc_str};
use super::super::{palette, App, PowerFocus};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

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

        // Full-width header: FOAM line + breadcrumb pill right-aligned.
        // Pill shows the nav path as "Library · Level" (bottom level omitted unless
        // it is also the top level). Separator dots are white; the hovered crumb
        // turns white. Clicking a crumb navigates back to that level.
        {
            // Build (display_name, target_truncation_depth) pairs.
            let crumb_depths: Vec<(String, usize)> = if self.power_left_tab == 0 {
                vec![("mbv".to_string(), 0)]
            } else {
                let lib_idx = self.power_left_tab - 1;
                let lib = &self.libs[lib_idx];
                let skip = if lib
                    .nav_stack
                    .first()
                    .map(|l| l.title == lib.library.name)
                    .unwrap_or(false)
                {
                    1
                } else {
                    0
                };
                let mut cd: Vec<(String, usize)> = vec![(lib.library.name.clone(), skip)];
                for (j, lvl) in lib.nav_stack.iter().enumerate().skip(skip) {
                    cd.push((lvl.title.clone(), j + 1));
                }
                // Drop the current (deepest) level from the pill unless it's the only one.
                if cd.len() > 1 {
                    cd.pop();
                }
                cd
            };

            let pill_style = Style::default().fg(palette::BASE).bg(palette::FOAM);
            let sep_style = Style::default().fg(palette::WHITE).bg(palette::FOAM);
            const SEP: &str = " \u{00b7} "; // " · "
            const SEP_W: usize = 3;

            // Budget: leave at least a few dashes on the left.
            let budget = (area.width as usize).saturating_sub(4);
            let raw_w = 1
                + crumb_depths.iter().map(|(s, _)| s.width()).sum::<usize>()
                + crumb_depths.len().saturating_sub(1) * SEP_W
                + 1;
            // If too wide, truncate the last displayed crumb so it fits.
            let last_crumb_budget = if raw_w > budget && crumb_depths.len() > 1 {
                let fixed_w = 1
                    + crumb_depths[..crumb_depths.len() - 1]
                        .iter()
                        .map(|(s, _)| s.width())
                        .sum::<usize>()
                    + (crumb_depths.len() - 1) * SEP_W
                    + 1;
                budget.saturating_sub(fixed_w)
            } else {
                budget
            };

            // Pre-compute display strings (with truncation applied).
            let displays: Vec<(String, usize)> = crumb_depths
                .iter()
                .enumerate()
                .map(|(i, (name, depth))| {
                    let s = if i == crumb_depths.len() - 1 {
                        trunc_str(name, last_crumb_budget).to_string()
                    } else {
                        name.clone()
                    };
                    (s, *depth)
                })
                .collect();

            // Pill geometry.
            let pill_w: usize = 1
                + displays.iter().map(|(s, _)| s.width()).sum::<usize>()
                + displays.len().saturating_sub(1) * SEP_W
                + 1;
            let left_line_w = (area.width as usize).saturating_sub(pill_w);
            let crumb_row = area.y;
            // x of first crumb = pill_start + 1 leading space
            let mut x_cursor: u16 = area.x + left_line_w as u16 + 1;

            // Music-group libraries don't use breadcrumb navigation -- the group
            // selector bar inside the view replaces it. Suppress click regions for them.
            let is_music_group_lib = self.power_left_tab > 0 && {
                let li = self.power_left_tab - 1;
                self.libs[li].library.collection_type == "music"
                    && self
                        .music_levels
                        .first()
                        .map(|s| s == "group")
                        .unwrap_or(false)
            };

            // Build spans and register hover/click regions in one pass.
            let mut pill_spans: Vec<Span> = vec![Span::styled(" ", pill_style)];
            let mut new_power_crumbs: Vec<(u16, u16, u16, usize)> = Vec::new();
            for (i, (display, target_depth)) in displays.iter().enumerate() {
                if i > 0 {
                    pill_spans.push(Span::styled(SEP, sep_style));
                    x_cursor += SEP_W as u16;
                }
                let dw = display.width() as u16;
                let x_start = x_cursor;
                let x_end = x_cursor + dw;
                let hovered = !is_music_group_lib
                    && self.mouse_row == crumb_row
                    && self.mouse_col >= x_start
                    && self.mouse_col < x_end;
                let crumb_fg = if hovered {
                    palette::WHITE
                } else {
                    palette::BASE
                };
                pill_spans.push(Span::styled(
                    display.clone(),
                    Style::default().fg(crumb_fg).bg(palette::FOAM),
                ));
                if !is_music_group_lib {
                    new_power_crumbs.push((x_start, x_end, crumb_row, *target_depth));
                }
                x_cursor = x_end;
            }
            pill_spans.push(Span::styled(" ", pill_style));
            layout.breadcrumbs = new_power_crumbs;

            let mut line_spans = vec![Span::styled(
                "\u{2500}".repeat(left_line_w),
                Style::default().fg(palette::FOAM),
            )];
            line_spans.extend(pill_spans);
            f.render_widget(
                Paragraph::new(Line::from(line_spans)),
                Rect {
                    x: area.x,
                    y: area.y,
                    width: area.width,
                    height: 1,
                },
            );
        }

        let content_h = area.height.saturating_sub(1);
        let left_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: left_w,
            height: content_h,
        };
        let right_area = Rect {
            x: area.x + left_w + 1,
            y: area.y + 1,
            width: right_w.saturating_sub(1),
            height: content_h,
        };

        let queue_focused = matches!(self.power_focus, PowerFocus::Queue);
        let left_focused = !queue_focused;

        // The card fills the top of the left column; the queue list takes the rows
        // below it. At low heights the card can consume most of the column, so relocate
        // the queue under the library on the right instead of cramming it in.
        let (card_h, _) = self.render_power_card(f, left_area);
        let left_remaining = left_area.height.saturating_sub(card_h);

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
                    y: left_area.y + card_h,
                    height: left_remaining,
                    ..left_area
                },
            )
        };

        self.render_power_queue(f, queue_area, queue_focused, layout);
        self.render_power_library(f, lib_area, left_focused, layout);
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
        let has_detail = self.power_left_tab > 0 && self.libs[lib_idx].power_detail_item.is_some();
        let is_feed_group = self.power_left_tab > 0 && self.is_feed_home_video_group_view(lib_idx);
        let is_music_group = self.power_left_tab > 0 && self.is_music_group_view(lib_idx);
        let is_album = self.power_left_tab > 0 && self.is_album_level(lib_idx);
        let is_series = self.power_left_tab > 0 && self.is_series_view(lib_idx);
        let is_home_video = self.power_left_tab > 0 && self.is_home_video_view(lib_idx);
        let show_compact_movie_detail =
            !has_detail && self.power_selected_movie_item(lib_idx).is_some();
        if has_detail {
            self.render_power_detail(f, area, lib_idx, focused, layout);
        } else if show_compact_movie_detail {
            const COMPACT_DETAIL_H: u16 = 13;
            const COMPACT_DETAIL_GAP: u16 = 1;
            let reserved_h = COMPACT_DETAIL_H.saturating_add(COMPACT_DETAIL_GAP);
            let banner_h = COMPACT_DETAIL_H
                .min(area.height.saturating_sub(COMPACT_DETAIL_GAP).max(1))
                .max(1);
            let banner_area = Rect {
                height: banner_h,
                ..area
            };
            let list_area = Rect {
                y: area.y + reserved_h.min(area.height),
                height: area.height.saturating_sub(reserved_h),
                ..area
            };
            self.render_power_compact_detail(f, banner_area, lib_idx, focused, layout);
            if list_area.height > 0 {
                self.render_power_list(f, list_area, focused, layout);
            }
        } else if is_feed_group {
            self.render_power_feed_home_video_group_view(f, area, lib_idx, focused, layout);
        } else if is_music_group {
            self.render_power_music_group_view(f, area, lib_idx, focused, layout);
        } else if is_album {
            self.render_power_album_detail(f, area, lib_idx, focused, layout);
        } else if is_series {
            self.render_power_episode_detail(f, area, lib_idx, focused, layout);
        } else if is_home_video {
            self.render_power_home_video_list(f, area, lib_idx, focused, layout);
        } else {
            self.render_power_list(f, area, focused, layout);
        }
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
    use crate::app::layout::LayoutPower;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{BrowseLevel, LibraryTab};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

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

    fn render_power_library_to_string(app: &mut App, layout: &mut LayoutPower) -> String {
        let backend = TestBackend::new(60, 16);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_library(f, Rect::new(0, 0, 60, 16), true, layout);
        })
        .unwrap();
        buffer_to_string(&term)
    }

    fn render_power_view(app: &mut App, width: u16, height: u16) -> LayoutPower {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_view(f, Rect::new(0, 0, width, height), &mut layout);
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
            power_detail_item: None,
            power_detail_scroll: 0,
        });

        app
    }

    #[test]
    fn movie_library_renders_compact_banner_without_opening_expanded_detail() {
        let mut app = make_power_movie_app();
        let mut layout = LayoutPower::default();

        let out = render_power_library_to_string(&mut app, &mut layout);
        let lines: Vec<&str> = out.lines().collect();

        assert!(app.libs[0].power_detail_item.is_none());
        assert!(
            out.contains("Focused Movie"),
            "expected banner title:\n{out}"
        );
        assert!(
            out.contains("compact movie banner"),
            "expected compact overview text:\n{out}"
        );
        assert!(
            out.contains("Second Movie"),
            "expected list to remain visible:\n{out}"
        );
        assert!(
            !out.contains("Director: Director Hidden"),
            "compact banner must hide director:\n{out}"
        );
        assert_eq!(lines[13].trim(), "", "expected spacer row between banner and list:\n{out}");
        assert!(
            lines[14].contains("Focused Movie"),
            "expected movie list below banner spacer:\n{out}"
        );
    }

    #[test]
    fn power_view_uses_configured_left_column_width() {
        let mut app = make_power_movie_app();
        app.power_left_width = 55;

        let layout = render_power_view(&mut app, 100, 28);

        assert_eq!(layout.queue_area.width, 55);
    }
}
