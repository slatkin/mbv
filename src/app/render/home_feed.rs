use super::super::ui_util::*;
use super::home_video::render_home_video_item;
use crate::app::layout::LayoutMain;
use crate::app::App;
use ratatui::layout::Rect;
use ratatui::Frame;

impl App {
    pub(super) fn render_power_feed_home_video_group_view(
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
        self.ensure_lib_loaded_for(lib_idx);

        let Some(root_level) = self.libs[lib_idx].nav_stack.first() else {
            return;
        };
        let groups = self.libs[lib_idx]
            .feed_home_video
            .as_ref()
            .map(|state| {
                state
                    .groups
                    .iter()
                    .map(|group| group.folder.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let selected_group = self.feed_home_video_selected_group_index(lib_idx);
        let items = self.feed_home_video_selected_items(lib_idx);
        let (cursor, stored_scroll, loading) = self.libs[lib_idx]
            .feed_home_video
            .as_ref()
            .map(|state| (state.video_cursor, state.video_scroll, state.loading))
            .unwrap_or((0, 0, root_level.loading));

        let max_y = area.y + area.height;
        let mut row = area.y;
        let mut selector_tabs: Vec<(Rect, usize)> = Vec::new();

        if row < max_y {
            const MAX_LABEL: usize = 12;
            let labels: Vec<String> = std::iter::once("All".to_string())
                .chain(
                    groups
                        .iter()
                        .map(|g| trunc_str(&g.name, MAX_LABEL).to_string()),
                )
                .collect();
            // Tabs are identified by 0-based index (0 = "All").
            let ids: Vec<usize> = (0..labels.len()).collect();
            selector_tabs = super::render_pill_bar(
                f,
                Rect {
                    x: area.x,
                    y: row,
                    width: area.width,
                    height: 1,
                },
                super::PillBar {
                    labels: &labels,
                    ids: &ids,
                    selected_pos: selected_group,
                    prefix: None,
                },
            );
        }
        if row < max_y {
            row += 1;
        }
        if row < max_y {
            row += 1;
        }
        layout.selector_tabs = selector_tabs;

        let list_area = Rect {
            x: area.x,
            y: row,
            width: area.width,
            height: max_y.saturating_sub(row),
        };
        layout.left_area = list_area;
        if list_area.height == 0 {
            return;
        }

        if items.is_empty() {
            if row < max_y {
                let msg = if loading {
                    " Loading\u{2026}"
                } else {
                    " (empty)"
                };
                super::render_power_placeholder(
                    f,
                    Rect {
                        x: list_area.x,
                        y: list_area.y,
                        width: list_area.width,
                        height: 1,
                    },
                    msg,
                );
            }
            return;
        }

        let current_pos = cursor.min(items.len().saturating_sub(1));
        let text_w_with_sb = (list_area.width as usize).saturating_sub(1);
        let mut item_heights = vec![1; items.len()];
        let selected_panel_width = text_w_with_sb
            .saturating_sub(2 * super::list_rows::SELECTED_BLOCK_SIDE_PADDING as usize)
            as u16;
        let selected_height = self
            .compact_banner_layout_with_overview(&items[current_pos], selected_panel_width, true)
            .content_rows()
            .saturating_add(5) as u16;
        item_heights[current_pos] = selected_height;
        let total_h: u16 = item_heights.iter().sum();
        let needs_scrollbar = total_h > list_area.height;
        let text_w = super::power_content_width(list_area.width, needs_scrollbar);

        let mut scroll = stored_scroll.min(items.len().saturating_sub(1));
        if current_pos < scroll {
            scroll = current_pos;
        }
        while scroll < current_pos {
            let visible_h: u16 = item_heights[scroll..=current_pos].iter().sum();
            if visible_h <= list_area.height {
                break;
            }
            scroll += 1;
        }
        if let Some(state) = self.libs[lib_idx].feed_home_video.as_mut() {
            state.video_scroll = scroll;
        }

        let mut row_map: Vec<Option<usize>> = Vec::with_capacity(list_area.height as usize);
        let mut row_y = list_area.y;
        let mut visible_items = 0usize;
        for (item_idx, item) in items.iter().enumerate().skip(scroll) {
            if row_y >= list_area.y + list_area.height {
                break;
            }
            visible_items += 1;
            let item_h = item_heights[item_idx];
            let selected = item_idx == current_pos;
            if selected {
                layout.cursor_screen_y = Some(row_y);
            }
            render_home_video_item(f, item, row_y, item_h, list_area, text_w, selected, focused);
            if selected {
                self.render_selected_home_video_detail(
                    f,
                    Rect {
                        width: text_w as u16,
                        ..list_area
                    },
                    row_y,
                    item_h,
                    lib_idx,
                    focused,
                    layout,
                );
            }
            let visible_rows = (list_area.y + list_area.height)
                .saturating_sub(row_y)
                .min(item_h);
            for _ in 0..visible_rows {
                row_map.push(Some(item_idx));
            }
            row_y += item_h;
        }
        row_map.resize(list_area.height as usize, None);
        layout.left_row_map = row_map;

        if needs_scrollbar && focused {
            super::render_power_right_scrollbar_with_viewport(
                f,
                list_area,
                items.len(),
                visible_items.max(1),
                scroll,
            );
        }
    }
}
