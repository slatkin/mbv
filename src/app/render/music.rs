use super::super::ui_util::*;
use crate::app::layout::LayoutMain;
use crate::app::App;
use mbv_core::api::MediaItem;
use ratatui::layout::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

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
        layout: &mut LayoutMain,
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
        let labels: Vec<String> = groups
            .iter()
            .map(|g| trunc_str(&g.name, MAX_LABEL).to_string())
            .collect();
        // Music-group tabs are identified by their 0-based group index.
        let ids: Vec<usize> = (0..labels.len()).collect();
        layout.selector_tabs = super::render_pill_bar(
            f,
            row_area,
            super::PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos: group_cursor,
                prefix: None,
                underlay: super::PillUnderlay::Blank { fill: true },
            },
        );
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
        layout: &mut LayoutMain,
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
                super::render_power_placeholder(
                    f,
                    Rect {
                        x: area.x,
                        y: row,
                        width: area.width,
                        height: 1,
                    },
                    msg,
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
