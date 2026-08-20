use super::album_plan::{GroupedAlbumDisplayRow, HeaderFocusCtx};
use super::album_rows::AlbumRowCtx;
use super::list_rows::draw_column_selection_markers;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App};
use ratatui::layout::*;
use ratatui::Frame;

impl App {
    /// Renders the wide right pane's album browser: a one-column
    /// artist-grouped album list.
    pub(super) fn render_wide_right_album_browser(
        &mut self,
        f: &mut Frame,
        browser_area: Rect,
        panel_area: Rect,
        lib_idx: usize,
        right_focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.wide_music_browser_area = browser_area;
        let Some(level) = self.libs[lib_idx].nav_stack.last() else {
            return;
        };
        let albums = level.items.clone();
        let cursor = level.cursor;

        if albums.is_empty() {
            let msg = if level.loading {
                " Loading\u{2026}"
            } else {
                " (empty)"
            };
            super::render_placeholder(f, browser_area, msg);
            return;
        }

        let (album_info, order) = {
            let catalog = self.libs[lib_idx]
                .nav_stack
                .last()
                .and_then(|l| l.music_grouping.as_ref())
                .and_then(|s| s.settled.as_ref());
            match catalog {
                Some(cat) => {
                    let info = self.group_album_info(&albums, Some(cat));
                    let order: Vec<usize> = cat
                        .entries
                        .iter()
                        .map(|e| e.album_index)
                        .filter(|&i| i < albums.len())
                        .collect();
                    (info, order)
                }
                None => {
                    let info = self.group_album_info(&albums, None);
                    let order = super::sorted_group_album_order(&info);
                    (info, order)
                }
            }
        };

        // One column only — no two-column packing, and no selectable
        // headers in the wide right rail (design Decision 5).
        let plan = self.build_grouped_album_display_plan(
            &albums,
            &album_info,
            &order,
            cursor,
            true,
            HeaderFocusCtx {
                in_music_group_view: true,
                expand_selected: false,
            },
            None, // No wrap_widths needed for one-column.
            true,
        );

        // Scroll to keep the selected album visible.
        let display_cursor = plan.display_cursor;
        let total_rows = plan.rows.len();
        let visible = browser_area.height as usize;
        let stored_scroll = self.libs[lib_idx]
            .nav_stack
            .last()
            .map(|l| l.scroll)
            .unwrap_or(0);
        let max_offset = total_rows.saturating_sub(visible);
        let mut offset = stored_scroll.min(max_offset);
        if display_cursor < offset {
            offset = display_cursor;
        } else if display_cursor >= offset + visible {
            offset = display_cursor
                .saturating_add(1)
                .saturating_sub(visible)
                .min(max_offset);
        }

        // Update scroll.
        if let Some(level) = self.libs[lib_idx].nav_stack.last_mut() {
            level.scroll = offset;
        }

        let visible_rows: Vec<_> = plan
            .rows
            .iter()
            .enumerate()
            .skip(offset)
            .take(visible)
            .collect();

        for (row_idx, row) in &visible_rows {
            let screen_y = (*row_idx - offset) as u16;
            // Album-row renderers supply their own one-cell leading gutter,
            // so visible text begins one cell right of `browser_area.x` --
            // the same convention every other inline browser uses
            // (`item_cell_spans`' leading space), matching the Movies/TV
            // indent. The caller is responsible for any inset needed to
            // land text where it wants (see `render_wide_music_group`'s
            // `browser_area`).
            let row_area = Rect {
                x: browser_area.x,
                y: browser_area.y + screen_y,
                width: browser_area.width,
                height: 1,
            };

            match row {
                super::album_plan::GroupedAlbumDisplayRow::ArtistHeader(header) => {
                    // Wide right rail: no selectable headers (design Decision 5).
                    self.render_artist_header_row(
                        f, row_area, header, true, // in_music_group_view
                        None, // No selected block in wide right rail.
                        *row_idx, 0, // No art reservation in right rail.
                    );
                }
                super::album_plan::GroupedAlbumDisplayRow::ArtistGroupSpacer => {}
                super::album_plan::GroupedAlbumDisplayRow::Album(idx) => {
                    let selected = *idx == cursor;
                    if selected {
                        layout.selected_item_rect = Some(Rect {
                            x: browser_area.x,
                            y: browser_area.y + screen_y,
                            width: browser_area.width,
                            height: 1,
                        });
                    }
                    if selected && right_focused {
                        self.render_wide_selected_album_row(
                            f,
                            row_area,
                            panel_area,
                            *idx,
                            &album_info,
                            right_focused,
                        );
                    } else {
                        self.render_album_row(
                            f,
                            AlbumRowCtx {
                                row_area,
                                idx: *idx,
                                album_info: &album_info,
                                cursor,
                                avail: row_area.width as usize,
                                selected_block_bounds: None,
                                in_music_group_view: true,
                                abs_row_idx: *row_idx,
                                selected_art_reserved_w: 0,
                                focused: right_focused,
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        // Scrollbar.
        if total_rows > visible && right_focused {
            let max_off = total_rows.saturating_sub(visible);
            super::render_right_scrollbar(f, browser_area, max_off, offset, palette::SCROLLBAR);
        }

        // Draw the unified edge selection marker (design.md decision 2) in
        // the outer gutter, flush with the parent panel -- matching every
        // other list (movies/TV, audiobooks, feeds, narrow's own plain
        // rows) instead of a marker glyph baked into the selected row's own
        // text flow.
        let item_rows: Vec<Vec<usize>> = plan
            .rows
            .iter()
            .map(|row| match row {
                GroupedAlbumDisplayRow::Album(idx) => vec![*idx],
                _ => Vec::new(),
            })
            .collect();
        draw_column_selection_markers(f, browser_area, cursor, &item_rows, offset);

        // Populate left_row_targets for mouse hit-testing, indexed relative
        // to `browser_area`'s own top -- self-contained so this works
        // identically whether the caller is the wide right pane (browser
        // sits below the pill row) or the narrow inline presentation
        // (browser_area == left_area, already below hero+pills).
        {
            let mut targets = vec![None; browser_area.height as usize];
            for (row_idx, row) in &visible_rows {
                let screen_y = *row_idx as isize - offset as isize;
                if screen_y < 0 {
                    continue;
                }
                if let Some(slot) = targets.get_mut(screen_y as usize) {
                    *slot = row.row_target();
                }
            }
            layout.left_row_targets = targets;
        }

        // Update left_sorted_indices for cursor navigation.
        layout.left_sorted_indices = plan.order;
    }
}
