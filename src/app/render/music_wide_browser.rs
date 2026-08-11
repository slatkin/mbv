use super::album_plan::HeaderFocusCtx;
use super::album_rows::AlbumRowCtx;
use crate::app::layout::LayoutMain;
use crate::app::App;
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
            // Album-row renderers supply their own one-cell leading gutter.
            // Extend the row one cell left so visible text still begins at
            // the browser area's standard two-column interior inset, while
            // retaining two columns at the right edge.
            let row_area = Rect {
                x: browser_area.x.saturating_sub(1),
                y: browser_area.y + screen_y,
                width: browser_area.width.saturating_add(1),
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
                        layout.cursor_screen_y = Some(browser_area.y + screen_y);
                    }
                    if selected && right_focused {
                        self.render_wide_selected_album_row(
                            f,
                            row_area,
                            panel_area,
                            *idx,
                            &album_info,
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
            super::render_right_scrollbar(f, browser_area, max_off, offset);
        }

        // Populate left_row_targets for mouse hit-testing in the right pane.
        // Indexed from wide_music_right_area.y so clicks on the browser's
        // album rows resolve correctly (gap rows above the browser are None).
        {
            let right_area_top = layout.wide_music_right_area.y;
            let right_area_h = layout.wide_music_right_area.height as usize;
            let browser_y_offset = if browser_area.y > right_area_top {
                (browser_area.y - right_area_top) as usize
            } else {
                0
            };
            let mut targets = vec![None; right_area_h];
            for (row_idx, row) in &visible_rows {
                let screen_y = *row_idx as isize - offset as isize;
                if screen_y < 0 {
                    continue;
                }
                let target_y = browser_y_offset + screen_y as usize;
                if target_y < targets.len() {
                    targets[target_y] = row.row_target();
                }
            }
            layout.left_row_targets = targets;
        }

        // Update left_sorted_indices for cursor navigation.
        layout.left_sorted_indices = plan.order;
    }
}
