mod album;
mod card;
mod detail;
mod episode;
mod home;
mod list;
mod music;
mod queue;

use super::super::ui_util::trunc_str;
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
        if s.len() > prefix.len() && s[..prefix.len()].eq_ignore_ascii_case(prefix) {
            return &s[prefix.len()..];
        }
    }
    s
}

/// Returns the effective sort key for an item: `sort_name` when Emby provides it,
/// otherwise the item's display name with any leading article stripped.
pub(super) fn effective_sort_str(item: &crate::api::MediaItem) -> &str {
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
pub(super) fn letter_bucket(item: &crate::api::MediaItem, total: usize) -> String {
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
    pub(super) fn render_power_view(&mut self, f: &mut Frame, area: Rect) {
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

        // Left panel (fixed 40 cols, card + queue) | Right panel (library, remaining).
        let left_w: u16 = 40;
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
            self.layout_power_breadcrumbs = new_power_crumbs;

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
        let (lib_area, queue_area) = if left_remaining < MIN_LIST_ROWS {
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

        self.render_power_queue(f, queue_area, queue_focused);
        self.render_power_library(f, lib_area, left_focused);
    }

    fn render_power_library(&mut self, f: &mut Frame, area: Rect, focused: bool) {
        // If a music-group library's nav_stack was truncated to just the group
        // level (e.g., stale breadcrumb click), immediately re-push the album level.
        if self.power_left_tab > 0 {
            self.ensure_music_group_album_level(self.power_left_tab - 1);
        }

        if self.power_left_tab == 0 {
            self.render_power_home_list(f, area, focused);
            return;
        }
        let lib_idx = self.power_left_tab.saturating_sub(1);
        let has_detail = self.power_left_tab > 0 && self.libs[lib_idx].power_detail_item.is_some();
        if has_detail {
            self.render_power_detail(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_music_group_view(lib_idx) {
            self.render_power_music_group_view(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_album_level(lib_idx) {
            self.render_power_album_detail(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_series_view(lib_idx) {
            self.render_power_episode_detail(f, area, lib_idx, focused);
        } else if self.power_left_tab > 0 && self.is_home_video_view(lib_idx) {
            self.render_power_home_video_list(f, area, lib_idx, focused);
        } else {
            self.render_power_list(f, area, focused);
        }
    }
}
