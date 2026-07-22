mod context;
mod meta;
mod row;

use super::super::super::layout::LayoutLibrary;
use super::super::super::palette;
use super::super::super::App;
use mbv_core::api::MediaItem;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState};
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
                .filter_map(|&i| {
                    s.items.get(i).map(|item| {
                        (
                            i,
                            self.recursive_album_display_item(lib_idx, i, item.clone()),
                        )
                    })
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
        let search_loading = self.libs[lib_idx]
            .search
            .as_ref()
            .is_some_and(|search| search.loading);
        let loading = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.loading)
            .unwrap_or(false);
        let msg = if search_loading && self.recursive_album_search_enabled(lib_idx) {
            "  Indexing music library..."
        } else if loading {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::tests::{make_app_stub, make_item};
    use crate::app::{LibSearch, LibraryTab};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    #[test]
    fn recursive_album_search_loading_message_is_explicit() {
        let mut app = make_app_stub();
        app.music_levels = vec!["group".into(), "album".into()];
        let mut library = make_item("Music", "CollectionFolder");
        library.id = "music-lib".into();
        library.collection_type = "music".into();
        library.is_folder = true;
        app.libs.push(LibraryTab {
            library,
            nav_stack: Vec::new(),
            search: Some(LibSearch {
                query: "record".into(),
                items: Vec::new(),
                results: Vec::new(),
                cursor: 0,
                scroll: 0,
                loading: true,
            }),
            feed_home_video: None,
            album_track_focus: None,
            artist_header_focus: None,
            library_total: None,
        });
        let backend = TestBackend::new(50, 4);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                app.render_library_table(
                    frame,
                    Rect::new(0, 0, 50, 4),
                    0,
                    &mut LayoutLibrary::default(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut output = String::new();
        for y in 0..buffer.area().height {
            for x in 0..buffer.area().width {
                output.push_str(buffer[(x, y)].symbol());
            }
        }

        assert!(output.contains("Indexing music library..."), "{output}");

        app.music_levels.clear();
        terminal
            .draw(|frame| {
                app.render_library_table(
                    frame,
                    Rect::new(0, 0, 50, 4),
                    0,
                    &mut LayoutLibrary::default(),
                );
            })
            .unwrap();
        let output = buffer_to_string(terminal.backend().buffer());
        assert!(!output.contains("Indexing music library..."), "{output}");
    }

    fn buffer_to_string(buffer: &ratatui::buffer::Buffer) -> String {
        let mut output = String::new();
        for y in 0..buffer.area().height {
            for x in 0..buffer.area().width {
                output.push_str(buffer[(x, y)].symbol());
            }
        }
        output
    }
}
