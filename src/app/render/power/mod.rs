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

// Power View re-renders frequently while scrolling; prefer a cheaper filter in
// these hot paths to reduce terminal image preparation stalls.
pub(super) const POWER_RENDER_FILTER: ratatui_image::FilterType =
    ratatui_image::FilterType::Triangle;

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
        //
        // Music-group libraries replace the breadcrumb pill with their group
        // selector pills, rendered inline on this same rule row, ending in a
        // fixed `Music` marker pinned to the far right (see #180).
        {
            let crumb_row = area.y;

            let is_music_group_lib =
                self.power_left_tab > 0 && self.is_music_group_view(self.power_left_tab - 1);

            if is_music_group_lib {
                layout.breadcrumbs = Vec::new();
                let lib_idx = self.power_left_tab - 1;

                // Base rule spans the full header row (matches the plain FOAM
                // dash rule used elsewhere) -- the pills/marker below only
                // overlay the right-column segment of it, so the dash rule
                // still shows through underneath/between the pills and over
                // the left (card/queue) column, which has no selector of its
                // own.
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        "\u{2500}".repeat(area.width as usize),
                        Style::default().fg(palette::FOAM),
                    ))),
                    Rect {
                        x: area.x,
                        y: crumb_row,
                        width: area.width,
                        height: 1,
                    },
                );

                // Confine the selector to the right column's width -- same
                // horizontal footprint as the library panel below it.
                let right_col_x = area.x + left_w + 1;
                let right_col_w = right_w.saturating_sub(1);

                let marker_text = format!(" {} ", self.libs[lib_idx].library.name);
                let marker_w = (marker_text.width() as u16).min(right_col_w);
                let pills_w = right_col_w.saturating_sub(marker_w);

                if pills_w > 0 {
                    let pills_area = Rect {
                        x: right_col_x,
                        y: crumb_row,
                        width: pills_w,
                        height: 1,
                    };
                    self.render_power_music_group_pills_row(f, pills_area, lib_idx, layout);
                } else {
                    layout.selector_tabs = Vec::new();
                }

                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        marker_text,
                        Style::default().fg(palette::BASE).bg(palette::FOAM),
                    ))),
                    Rect {
                        x: right_col_x + pills_w,
                        y: crumb_row,
                        width: marker_w,
                        height: 1,
                    },
                );
            } else {
                layout.selector_tabs = Vec::new();

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
                // x of first crumb = pill_start + 1 leading space
                let mut x_cursor: u16 = area.x + left_line_w as u16 + 1;

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
                    let hovered = self.mouse_row == crumb_row
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
                    new_power_crumbs.push((x_start, x_end, crumb_row, *target_depth));
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
        if has_detail {
            self.render_power_detail(f, area, lib_idx, focused, layout);
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
    use mbv_core::api::MediaItem;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn power_view_uses_triangle_resampling() {
        assert_eq!(POWER_RENDER_FILTER, ratatui_image::FilterType::Triangle);
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

    fn render_power_library_to_string(app: &mut App, layout: &mut LayoutPower) -> String {
        // Height is one row taller than the banner's own reserved footprint
        // (1 selected row + 15 rule/content/gap rows: 1 opening rule + 13
        // content + 1 closing rule) to also leave room for the " N items"
        // header row that `render_power_list` now draws unconditionally for a
        // focused library panel -- there's no separate top-pinned title row
        // to absorb it now that this goes through the shared catch-all path.
        let backend = TestBackend::new(60, 18);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            app.render_power_library(f, Rect::new(0, 0, 60, 18), true, layout);
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
        // Row 0 is the " N items" header that `render_power_list` always draws
        // for a focused, non-search library panel. Row 1 is the banner's
        // opening horizontal rule -- placed directly *above* the selected
        // movie's own row (row 2) so the selected title reads as set off from
        // the row above it. Row 16 closes the banner with another rule before
        // the next list item (row 17).
        assert!(
            lines[1].contains('\u{2500}'),
            "expected opening rule directly above the selected movie row:\n{out}"
        );
        assert!(
            lines[2].contains("Focused Movie"),
            "expected selected movie row directly below the opening rule:\n{out}"
        );
        assert_eq!(
            lines[2].chars().next(),
            Some('▌'),
            "expected the selected-row indicator at the banner edge:\n{out}"
        );
        assert!(
            out.contains("compact movie banner"),
            "expected compact overview text:\n{out}"
        );
        assert_eq!(
            lines[3].chars().next(),
            Some('▌'),
            "expected the selection indicator to continue through the banner:\n{out}"
        );
        assert!(
            lines[17].contains("Second Movie"),
            "expected remaining movie list below banner:\n{out}"
        );
        assert!(
            out.contains("Director: Director Hidden"),
            "compact banner must show director:\n{out}"
        );
        let director_line = lines
            .iter()
            .position(|l| l.contains("Director: Director Hidden"))
            .expect("expected compact director line to render");
        assert!(
            lines
                .get(director_line.saturating_sub(1))
                .map(|l| l.starts_with('▌') && l.chars().skip(1).all(|c| c == ' '))
                .unwrap_or(false),
            "expected a spacer row before the director line:\n{out}"
        );
        assert!(
            lines[16].contains('\u{2500}'),
            "expected closing rule below banner content:\n{out}"
        );
        assert_ne!(
            lines[16].chars().next(),
            Some('▌'),
            "expected the selection indicator to stop before the banner's closing rule:\n{out}"
        );
        assert!(
            !lines[1].contains("Focused Movie") && !lines[16].contains("Focused Movie"),
            "expected selected row's title to appear only once, not repeated on a rule row:\n{out}"
        );
        assert!(
            !lines[17].contains("Focused Movie"),
            "expected selected row to stay above banner, not repeat below it:\n{out}"
        );
    }

    #[test]
    fn power_view_uses_configured_left_column_width() {
        let mut app = make_power_movie_app();
        app.power_left_width = 55;

        let layout = render_power_view(&mut app, 100, 28);

        assert_eq!(layout.queue_area.width, 55);
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
            power_detail_item: None,
            power_detail_scroll: 0,
        });

        app
    }

    #[test]
    fn music_group_pills_and_marker_render_on_top_rule_row() {
        let mut app = make_power_music_group_app();
        app.power_left_width = 20;
        let width = 100u16;
        let height = 20u16;
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let mut layout = LayoutPower::default();
        term.draw(|f| {
            app.render_power_view(f, Rect::new(0, 0, width, height), &mut layout);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let row0 = out.lines().next().unwrap();

        // Group pills render inline on row 0 -- the same row as the top rule --
        // instead of on a separate band below it.
        assert!(
            row0.contains("Alpha") && row0.contains("Beta"),
            "expected group pills on the top rule row:\n{out}"
        );

        // Cell x/y is a column index, not a byte offset -- the dash-rule glyph
        // is multi-byte UTF-8, so byte offsets from `str::find` would drift.
        // Convert by counting chars (every glyph in this row is single-width).
        let char_x = |needle: &str| -> u16 {
            let byte_idx = row0.find(needle).expect("needle not found in row 0");
            row0[..byte_idx].chars().count() as u16
        };
        let rchar_x = |needle: &str| -> u16 {
            let byte_idx = row0.rfind(needle).expect("needle not found in row 0");
            row0[..byte_idx].chars().count() as u16
        };

        // The fixed `Music` marker sits at the far right of that row.
        let music_x = rchar_x("Music");
        assert!(
            music_x + "Music".len() as u16 + 1 >= width,
            "expected the Music marker pinned to the far right of the row:\n{out}"
        );

        let buf = term.backend().buffer();
        assert_eq!(
            buf[(music_x, 0)].bg,
            palette::FOAM,
            "expected the Music marker to keep the standard blue pill background"
        );
        assert_eq!(
            buf[(music_x, 0)].fg,
            palette::BASE,
            "expected the Music marker to keep the standard base (black) text"
        );

        // The selector (pills + marker) is confined to the right column's
        // width -- the segment over the left card/queue column stays a plain
        // dash rule, not stretched-out pills.
        let right_col_x = app.power_left_width + 1;
        assert!(
            row0.chars()
                .take(right_col_x as usize)
                .all(|c| c == '\u{2500}'),
            "expected a plain dash rule over the left column, not pills:\n{out}"
        );

        // The selected group pill ("Alpha", cursor 0) uses the yellow-text
        // treatment; a non-selected pill ("Beta") stays blue.
        let alpha_x = char_x("Alpha");
        assert!(
            alpha_x >= right_col_x,
            "expected pills confined to the right column"
        );
        assert_eq!(buf[(alpha_x, 0)].bg, palette::FOAM);
        assert_eq!(
            buf[(alpha_x, 0)].fg,
            palette::YELLOW,
            "expected the selected group pill to use yellow text"
        );
        let beta_x = char_x("Beta");
        assert_eq!(buf[(beta_x, 0)].bg, palette::FOAM);
        assert_eq!(
            buf[(beta_x, 0)].fg,
            palette::BASE,
            "expected a non-selected group pill to stay blue with base text"
        );

        // The gap between the two pills is the dash rule, not a blank space
        // -- the rule reads as continuous underneath/between the pills.
        let (gap_start, gap_end) = (alpha_x.min(beta_x), alpha_x.max(beta_x));
        let between: String = row0
            .chars()
            .skip(gap_start as usize)
            .take((gap_end - gap_start) as usize)
            .collect();
        assert!(
            between.contains('\u{2500}'),
            "expected a dash rule between adjacent pills, not blank space:\n{between:?}"
        );

        // Selector hitboxes are registered on the header row, confined to the
        // right column, and line up with the rendered pills (not the fixed
        // Music marker).
        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert_eq!(rect.y, 0, "expected selector hitboxes on the top rule row");
            assert!(
                rect.x >= right_col_x,
                "expected selector hitboxes confined to the right column"
            );
            assert!(
                rect.x < music_x,
                "expected selector hitboxes to stay left of the fixed Music marker"
            );
        }

        // The album list starts directly below the combined header row --
        // no leftover blank/pill/blank selector band.
        let row1 = out.lines().nth(1).unwrap();
        assert!(
            row1.contains("Alpha") || row1.contains("First Album"),
            "expected album list content to start immediately below the header row:\n{out}"
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
            app.render_power_view(f, Rect::new(0, 0, width, height), &mut layout);
        })
        .unwrap();
        let out = buffer_to_string(&term);
        let row0 = out.lines().next().unwrap();

        // Too many groups to fit within the (narrow) right column -- a right
        // scroll indicator appears on the same row as the pills, and the
        // fixed Music marker still renders.
        assert!(
            row0.contains('\u{203a}'),
            "expected a right scroll indicator on the pills row:\n{out}"
        );
        assert!(
            row0.contains("Music"),
            "expected the Music marker to keep rendering when pills overflow:\n{out}"
        );

        // The Music marker doesn't scroll with the pills: it still sits at
        // the far right of the (narrow) row.
        let music_x = row0.rfind("Music").unwrap();
        assert!(
            music_x as u16 + "Music".len() as u16 + 1 >= width,
            "expected the Music marker to remain pinned to the far right:\n{out}"
        );

        // The selector is still confined to the right column -- the segment
        // over the left card/queue column stays a plain dash rule.
        let right_col_x = (app.power_left_width + 1) as usize;
        assert!(
            row0.chars().take(right_col_x).all(|c| c == '\u{2500}'),
            "expected a plain dash rule over the left column, not pills:\n{out}"
        );

        // Every registered pill hitbox stays inside the space reserved for
        // pills, right of the left column and left of Music.
        assert!(!layout.selector_tabs.is_empty());
        for (rect, _) in &layout.selector_tabs {
            assert!(
                rect.x as usize >= right_col_x,
                "expected pill hitboxes confined to the right column"
            );
            assert!(
                (rect.x + rect.width) as usize <= music_x,
                "expected pill hitboxes confined to the scrollable area left of Music"
            );
        }
    }
}
