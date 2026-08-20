use super::hero_left;
use crate::app::layout::LayoutMain;
use crate::app::{palette, App, PanelFocus};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

const PANE_PAD_X: u16 = 2;
const PANE_PAD_Y: u16 = 1;

impl App {
    pub(super) fn is_wide_tv_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            lib.library.collection_type == "tvshows"
                && lib.nav_stack.last().is_some_and(|level| {
                    level.items.is_empty()
                        || level.items.iter().all(|item| item.item_type == "Series")
                })
        })
    }

    pub(super) fn render_wide_tv(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.tv_wide_episode_rows.clear();
        layout.tv_wide_season_tabs.clear();

        let Some((mut left_panel, right_panel)) = hero_left::shared_hero_presentation(area) else {
            return;
        };
        left_panel.height = area.height.saturating_sub(1);
        let left_area = Rect {
            x: left_panel.x.saturating_add(PANE_PAD_X),
            y: left_panel.y.saturating_add(PANE_PAD_Y),
            width: left_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: left_panel.height.saturating_sub(PANE_PAD_Y * 2),
        };
        let right_area = Rect {
            y: right_panel.y.saturating_add(PANE_PAD_Y),
            height: right_panel.height.saturating_sub(PANE_PAD_Y * 2),
            ..right_panel
        };
        layout.tv_wide_left_area = left_area;
        layout.tv_wide_right_area = right_area;
        layout.left_area = Rect::default();

        let episode_focused = focused
            && matches!(self.effective_panel_focus(), PanelFocus::Library)
            && self.libs[lib_idx].series_selection.is_some();
        let right_focused = focused && !episode_focused;

        f.render_widget(
            Block::default().style(Style::default().bg(palette::SURFACE_BACKDROP)),
            left_panel,
        );
        if self.selected_series_item(lib_idx).is_some() {
            self.render_series_inline_detail(
                f,
                left_area,
                lib_idx,
                episode_focused,
                true,
                true,
                layout,
            );
        } else {
            super::render_placeholder(f, left_area, " Loading\u{2026}");
        }

        let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        if let Some(search) = self.libs[lib_idx].search.as_ref() {
            super::hero::render_search_box(f, right_pane.pills_area, &search.query, search.loading);
        } else if self.should_show_letter_pills(lib_idx) {
            self.render_letter_pills_row(f, right_pane.pills_area, lib_idx, layout);
        }

        let list_panel = right_pane.list_panel;
        let list_area = Rect {
            x: list_panel.x.saturating_add(PANE_PAD_X),
            y: list_panel.y.saturating_add(PANE_PAD_Y),
            width: list_panel.width.saturating_sub(PANE_PAD_X * 2),
            height: list_panel.height.saturating_sub(PANE_PAD_Y * 2),
        };
        if list_panel.height > 0 {
            f.render_widget(
                Block::default().style(palette::resolve_surface_focus(right_focused)),
                list_panel,
            );
        }

        self.render_wide_library_rows(f, list_area, lib_idx, right_focused, layout);

        hero_left::hero_on_left_list_panel_border(f, list_panel, right_focused);
    }
}

#[cfg(test)]
#[path = "tv_wide_tests.rs"]
mod tests;
