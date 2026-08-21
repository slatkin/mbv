use crate::app::layout::LayoutMain;
use crate::app::render::arrangements::hero_left::{self, PANE_PAD_X, PANE_PAD_Y};
use crate::app::render::arrangements::library as library_arrangement;
use crate::app::render::arrangements::padded_rect;
use crate::app::{palette, App, PanelFocus};
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn is_wide_tv_library(&self, lib_idx: usize) -> bool {
        self.libs.get(lib_idx).is_some_and(|lib| {
            lib.library.collection_type == "tvshows"
                && lib.nav_stack.last().is_some_and(|level| {
                    level.items.is_empty()
                        || level.items.iter().all(|item| item.item_type == "Series")
                })
        })
    }

    pub(in crate::app::render) fn render_wide_tv(
        &mut self,
        f: &mut Frame,
        area: Rect,
        lib_idx: usize,
        focused: bool,
        layout: &mut LayoutMain,
    ) {
        layout.tv_wide_episode_rows.clear();
        layout.tv_wide_season_tabs.clear();

        let Some(panes) = library_arrangement::wide_library_panes(area, PANE_PAD_X, PANE_PAD_Y)
        else {
            return;
        };
        let left_panel = panes.left_panel;
        let right_panel = panes.right_panel;
        let left_area = panes.left_area;
        let right_area = panes.right_area;
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
            crate::app::render::render_placeholder(f, left_area, " Loading\u{2026}");
        }

        let right_pane = hero_left::hero_on_left_right_pane(right_panel, right_area, PANE_PAD_Y);
        if let Some(search) = self.libs[lib_idx].search.as_ref() {
            crate::app::render::components::hero::render_search_box(
                f,
                right_pane.pills_area,
                &search.query,
                search.loading,
            );
        } else if self.should_show_letter_pills(lib_idx) {
            self.render_letter_pills_row(f, right_pane.pills_area, lib_idx, layout);
        }

        let list_panel = right_pane.list_panel;
        let list_area = padded_rect(list_panel, PANE_PAD_X, PANE_PAD_Y);
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
