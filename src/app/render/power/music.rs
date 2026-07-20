use super::super::super::ui_util::*;
use crate::app::layout::LayoutPower;
use crate::app::{palette, App};
use mbv_core::api::MediaItem;
use ratatui::layout::*;
use ratatui::style::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

impl App {
    /// Returns the group level's items and cursor for a music-group library
    /// (the nav-stack level above the current album level), if pushed yet.
    fn music_group_state(&self, lib_idx: usize) -> (Vec<MediaItem>, usize) {
        let lib = &self.libs[lib_idx];
        if lib.nav_stack.len() >= 2 {
            let group_lvl = &lib.nav_stack[lib.nav_stack.len() - 2];
            (group_lvl.items.clone(), group_lvl.cursor)
        } else {
            (Vec::new(), 0)
        }
    }

    /// Renders the music-group selector pills (with horizontal scroll
    /// indicators) inside `row_area`. Gaps between pills, and any unused
    /// trailing width, are filled with blank space so the pills float free
    /// rather than appearing to sit on a divider line. `row_area` must
    /// already be confined to the right column and exclude the fixed
    /// `Music` marker reserved by the caller (#180).
    pub(super) fn render_power_music_group_pills_row(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        lib_idx: usize,
        layout: &mut LayoutPower,
    ) {
        let (groups, group_cursor) = self.music_group_state(lib_idx);
        if groups.is_empty() || row_area.width == 0 {
            layout.selector_tabs = Vec::new();
            if row_area.width > 0 {
                f.render_widget(
                    Paragraph::new(Line::from(Span::raw(" ".repeat(row_area.width as usize)))),
                    row_area,
                );
            }
            return;
        }

        const MAX_LABEL: usize = 12;
        let tab_labels: Vec<String> = groups
            .iter()
            .map(|g| trunc_str(&g.name, MAX_LABEL).to_string())
            .collect();
        let n_tabs = tab_labels.len();
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();

        // Actual display width of each pill: " label " = label_w + 2.
        // Gap between consecutive pills = 1 char.
        let pill_widths: Vec<usize> = tab_labels.iter().map(|l| l.width() + 2).collect();
        let bar_w = row_area.width as usize;

        // Greedy count: how many pills fit starting at `start` within `avail` chars.
        let count_fitting = |start: usize, avail: usize| -> usize {
            let mut used = 0usize;
            let mut count = 0usize;
            for width in pill_widths.iter().take(n_tabs).skip(start) {
                let need = if count == 0 { *width } else { 1 + *width };
                if used + need > avail {
                    break;
                }
                used += need;
                count += 1;
            }
            count
        };

        // Walk scroll_start forward until group_cursor is in the visible window.
        let mut scroll_start = 0usize;
        loop {
            let avail = bar_w
                .saturating_sub(if scroll_start > 0 { 2 } else { 0 }) // "‹ "
                .saturating_sub(2); // reserve for " ›"
            let cnt = count_fitting(scroll_start, avail);
            if cnt == 0 || scroll_start + cnt > group_cursor {
                break;
            }
            scroll_start += 1;
        }

        let has_left = scroll_start > 0;
        let avail_pills = bar_w
            .saturating_sub(if has_left { 2 } else { 0 })
            .saturating_sub(2); // reserve for " ›"
        let cnt = count_fitting(scroll_start, avail_pills);
        let scroll_end = (scroll_start + cnt).min(n_tabs);
        let has_right = scroll_end < n_tabs;

        let mut spans: Vec<Span> = Vec::new();
        let mut x_cursor = row_area.x;
        if has_left {
            let chunk = "\u{2039} ";
            spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
            x_cursor += chunk.width() as u16;
        }
        for (idx, label) in tab_labels[scroll_start..scroll_end].iter().enumerate() {
            if idx > 0 {
                // Blank gap so the pills float free rather than sitting on
                // a continuous divider line.
                spans.push(Span::raw(" "));
                x_cursor += 1;
            }
            let abs_idx = scroll_start + idx;
            let selected = abs_idx == group_cursor;
            let style = if selected {
                Style::default()
                    .fg(palette::YELLOW)
                    .bg(palette::FOAM)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(palette::BASE).bg(palette::FOAM)
            };
            let pill = format!(" {} ", label);
            selector_tabs.push((
                Rect {
                    x: x_cursor,
                    y: row_area.y,
                    width: pill.width() as u16,
                    height: 1,
                },
                abs_idx,
            ));
            spans.push(Span::styled(pill.clone(), style));
            x_cursor += pill.width() as u16;
        }
        if has_right {
            let chunk = " \u{203a}";
            spans.push(Span::styled(chunk, Style::default().fg(palette::FOAM)));
            x_cursor += chunk.width() as u16;
        }

        // Fill any remaining width with blank space so the pills stand
        // alone instead of appearing to sit on a divider line.
        let used_w = (x_cursor - row_area.x) as usize;
        if used_w < bar_w {
            spans.push(Span::raw(" ".repeat(bar_w - used_w)));
        }

        f.render_widget(Paragraph::new(Line::from(spans)), row_area);
        layout.selector_tabs = selector_tabs;
    }

    /// Renders the grouped-by-artist album list for a music group library. The
    /// group-selector pills for this view are rendered by the caller on their
    /// own row above this list (`render_power_music_group_pills_row`) -- this
    /// method starts directly with the album list.
    pub(super) fn render_power_music_group_view(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutPower,
    ) {
        if area.height == 0 {
            return;
        }

        // ── Collect nav state ─────────────────────────────────────────────────
        let (albums, album_cursor) = {
            let lib = &self.libs[lib_idx];
            let last = match lib.nav_stack.last() {
                Some(l) => l,
                None => return,
            };
            (last.items.clone(), last.cursor)
        };

        let max_y = area.y + area.height;
        let row = area.y;

        // ── Loading / empty state ─────────────────────────────────────────────
        if albums.is_empty() {
            if row < max_y {
                let is_loading = self.libs[lib_idx]
                    .nav_stack
                    .last()
                    .map(|l| l.loading)
                    .unwrap_or(false);
                let msg = if is_loading {
                    " Loading\u{2026}"
                } else {
                    " (empty)"
                };
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        msg,
                        Style::default().fg(palette::MUTED),
                    ))),
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                );
            }
            return;
        }

        // ── Album list (grouped by artist) ────────────────────────────────────
        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        if list_area.height == 0 {
            return;
        }

        // Store for click/page-size calculations (used by mouse handler and PageUp/Down).
        layout.left_area = list_area;

        let stored_scroll = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|lvl| lvl.scroll)
            .unwrap_or(0);
        let offset = self.render_power_grouped_album_rows(
            f,
            list_area,
            lib_idx,
            &albums,
            album_cursor,
            stored_scroll,
            focused,
            layout,
        );
        if let Some(lvl) = self.libs[lib_idx].nav_stack.last_mut() {
            lvl.scroll = offset;
        }
    }
}
