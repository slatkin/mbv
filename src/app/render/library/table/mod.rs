mod context;
mod meta;
mod row;

use super::super::super::layout::LayoutLibrary;
use super::super::super::palette;
use super::super::super::{App, LibSearchResult};
use mbv_core::api::MediaItem;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    List, ListItem, ListState, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
};
use ratatui::Frame;

pub(super) const LIB_SELECTED_IMG_W: u16 = 32;
pub(super) const LIB_AUDIO_IMG_W: u16 = 12;
pub(super) const LIB_EPISODE_IMG_W: u16 = 40;

#[derive(Clone)]
pub(super) struct LibraryTableContext {
    pub(super) images_enabled: bool,
    pub(super) at_album_folders: bool,
    pub(super) at_music_groups: bool,
    pub(super) is_feed_lib: bool,
    pub(super) now_playing_id: Option<String>,
    pub(super) audio_img_h: u16,
    pub(super) selected_img_h: u16,
    pub(super) episode_img_h: u16,
    pub(super) actual_sel_img_h: u16,
}

impl App {
    pub(super) fn render_library_table(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        layout: &mut LayoutLibrary,
    ) {
        if lib_idx >= self.libs.len() {
            return;
        }
        if self.is_viewing_season_grid(lib_idx) {
            self.render_season_grid(f, area, lib_idx, layout);
            return;
        }
        if let Some(v) = layout.lib_table_area.get_mut(lib_idx) {
            *v = area;
        }

        if self.render_recursive_album_search_rows(f, area, lib_idx, layout) {
            return;
        }

        let (display_items, cursor, total_count) = self.library_display_items(lib_idx);
        if self.render_library_table_empty_state(f, area, lib_idx, display_items.len()) {
            return;
        }

        let ctx = self.build_library_table_context(lib_idx, &display_items, cursor);
        let all_heights: Vec<u16> = display_items
            .iter()
            .enumerate()
            .map(|(i, (_, item))| self.library_row_height(area, item, i, cursor, &ctx))
            .collect();
        let scroll = self.library_table_scroll(area, lib_idx, cursor, &all_heights, layout);

        self.prefetch_library_table_assets(&display_items, cursor, &ctx);

        let total_h: u16 = all_heights.iter().sum();
        let needs_scrollbar = total_h > area.height;
        let sep_w = if needs_scrollbar {
            area.width.saturating_sub(1)
        } else {
            area.width
        };

        let mut row_y = area.y;
        let mut rendered_heights: Vec<u16> = Vec::new();
        for (vi, (_, item)) in display_items[scroll..].iter().enumerate() {
            if row_y >= area.y + area.height {
                break;
            }
            let abs_idx = scroll + vi;
            let row_h = all_heights[abs_idx].min(area.y + area.height - row_y);
            self.render_library_table_row(
                f, area, row_y, row_h, sep_w, item, abs_idx, cursor, &ctx,
            );
            rendered_heights.push(row_h);
            row_y += row_h;
        }
        if let Some(v) = layout.lib_row_heights.get_mut(lib_idx) {
            *v = rendered_heights;
        }

        if needs_scrollbar {
            // Use the server-reported total, not `display_items.len()`, so the
            // thumb reflects the real list size instead of shrinking to just
            // whatever page(s) have been lazily fetched so far.
            let mut sb_state =
                ScrollbarState::new(total_count.max(display_items.len())).position(scroll);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area,
                &mut sb_state,
            );
        }
    }

    /// Returns the visible `(index, item)` pairs for the current level, the
    /// cursor position, and the true item total (server-reported, not just
    /// however many pages have been lazily fetched so far).
    fn library_display_items(&self, lib_idx: usize) -> (Vec<(usize, MediaItem)>, usize, usize) {
        let lib = &self.libs[lib_idx];
        if let Some(s) = &lib.search {
            let items: Vec<(usize, MediaItem)> = s
                .results
                .iter()
                .filter_map(|result| match result {
                    LibSearchResult::VisibleItem(i) => {
                        s.items.get(*i).map(|item| (*i, item.clone()))
                    }
                    LibSearchResult::RecursiveAlbum(_) => None,
                })
                .collect();
            let total = items.len();
            (items, s.cursor, total)
        } else {
            let lvl = lib.nav_stack.last();
            let items: Vec<(usize, MediaItem)> = lvl
                .map(|l| {
                    l.items
                        .iter()
                        .enumerate()
                        .map(|(i, item)| (i, item.clone()))
                        .collect()
                })
                .unwrap_or_default();
            let cur = lvl.map(|l| l.cursor).unwrap_or(0);
            let total = lvl.map(|l| l.total_count).unwrap_or(items.len());
            (items, cur, total)
        }
    }

    fn render_library_table_empty_state(
        &self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        items_len: usize,
    ) -> bool {
        if items_len > 0 {
            return false;
        }
        let loading = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.loading)
            .unwrap_or(false);
        let msg = if loading {
            "  Loading..."
        } else if self.libs[lib_idx].search.is_some() {
            "  (no results)"
        } else {
            "  (empty)"
        };
        f.render_widget(
            Paragraph::new(msg).style(Style::default().fg(palette::MUTED)),
            area,
        );
        true
    }

    fn render_recursive_album_search_rows(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        layout: &mut LayoutLibrary,
    ) -> bool {
        let rows = self.libs[lib_idx]
            .search
            .as_ref()
            .map(|search| search.recursive_album_display_rows())
            .unwrap_or_default();
        if rows.is_empty() {
            return false;
        }

        let cursor = self.libs[lib_idx]
            .search
            .as_ref()
            .map(|search| search.cursor)
            .unwrap_or(0);
        let visible = area.height as usize;
        let stored_scroll = layout.lib_scroll.get(lib_idx).copied().unwrap_or(0);
        let offset = stored_scroll.clamp(
            cursor.saturating_sub(visible.saturating_sub(1)),
            cursor.min(rows.len().saturating_sub(1)),
        );

        let list_items: Vec<ListItem> = rows
            .iter()
            .skip(offset)
            .take(visible)
            .map(|(_, label)| {
                ListItem::new(Line::from(vec![Span::raw(" "), Span::raw(label.clone())]))
            })
            .collect();
        let mut state = ListState::default();
        state.select(Some(cursor.saturating_sub(offset)));
        f.render_stateful_widget(
            List::new(list_items).highlight_style(Style::default().add_modifier(Modifier::BOLD)),
            area,
            &mut state,
        );

        if let Some(v) = layout.lib_row_heights.get_mut(lib_idx) {
            *v = vec![1; rows.len().saturating_sub(offset).min(visible)];
        }
        if let Some(v) = layout.lib_scroll.get_mut(lib_idx) {
            *v = offset;
        }
        if rows.len() > visible {
            let mut sb_state = ScrollbarState::new(rows.len()).position(offset);
            f.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight)
                    .thumb_symbol("▐")
                    .track_symbol(Some(" "))
                    .begin_symbol(None)
                    .end_symbol(None)
                    .style(Style::default().fg(palette::SUBTLE)),
                area,
                &mut sb_state,
            );
        }
        true
    }
}
