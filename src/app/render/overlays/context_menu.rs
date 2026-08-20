use super::super::super::layout::AppLayout;
use super::super::super::palette;
use super::super::super::App;
use super::super::super::{ContextMenu, ContextMenuAnchor, PanelFocus};
use super::backdrop::dim_backdrop;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Clear, List, ListItem};
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_context_menu(
        &mut self,
        f: &mut Frame,
        layout: &mut AppLayout,
    ) {
        // A menu cannot coexist with any other modal/sidebar surface (design
        // §4): the backdrop is applied exactly once and never dims the menu.
        let Some(ref menu) = self.context_menu else {
            layout.context_menu_rect = None;
            return;
        };

        let size = ContextMenu::rendered_size(&menu.entries);

        // Resolve the containing panel from the panel focus.
        let (panel_rect, anchor): (Rect, Option<Rect>) = match &menu.anchor {
            ContextMenuAnchor::SelectedItem(focus) => {
                let (panel, selected) = match focus {
                    PanelFocus::Library => (layout.main.left_area, layout.main.selected_item_rect),
                    PanelFocus::Queue => {
                        (layout.main.queue_area, layout.main.queue_selected_item_rect)
                    }
                };
                // A missing selected rect (a supported renderer omitted it
                // this frame) falls back to the panel's origin.
                (panel, selected)
            }
            ContextMenuAnchor::Pointer { .. } => {
                let panel = match self.effective_panel_focus() {
                    PanelFocus::Library if layout.main.is_wide_tv_active() => {
                        let pos = match &menu.anchor {
                            ContextMenuAnchor::Pointer { x, y } => (*x, *y).into(),
                            ContextMenuAnchor::SelectedItem(_) => unreachable!(),
                        };
                        if layout.main.tv_wide_left_area.contains(pos) {
                            layout.main.tv_wide_left_area
                        } else {
                            layout.main.tv_wide_right_area
                        }
                    }
                    PanelFocus::Library => layout.main.left_area,
                    PanelFocus::Queue => layout.main.queue_area,
                };
                (panel, None)
            }
        };

        let pointer = match &menu.anchor {
            ContextMenuAnchor::Pointer { x, y } => Some((*x, *y)),
            _ => None,
        };
        let (x, y) = ContextMenu::place(panel_rect, size, anchor.as_ref(), pointer);
        let (width, height) = size;
        let rect = Rect {
            x,
            y,
            width,
            height,
        };
        layout.context_menu_rect = Some(rect);

        // Dim the background content before drawing the (undimmed) menu.
        dim_backdrop(f);
        f.render_widget(Clear, rect);
        let list_items: Vec<ListItem> = menu
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let style = if entry.action.is_none() {
                    Style::default().fg(palette::SUBTLE)
                } else if i == menu.cursor {
                    Style::default().fg(palette::BASE).bg(palette::IRIS)
                } else {
                    Style::default().fg(palette::TEXT)
                };
                ListItem::new(format!(" {} ", entry.label)).style(style)
            })
            .collect();
        let inner = Rect {
            x,
            y: y.saturating_add(1),
            width,
            height: menu.entries.len() as u16,
        };
        f.render_widget(List::new(list_items), inner);
    }
}
