use super::super::super::layout::AppLayout;
use super::super::super::palette;
use super::super::super::{ContextMenu, ContextMenuAnchor, PanelFocus};
use crate::app::render::components::backdrop::dim_backdrop;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Clear, List, ListItem};
use ratatui::Frame;

impl super::super::super::App {
    /// Compute context-menu placement and write it to `layout`. The actual
    /// painting is done by `render_context_menu_content`, called from the
    /// Interactive Component's `view()` (task 2.5).
    ///
    /// Keeps `layout.context_menu_rect` as the mouse hit-test source (read by
    /// `App::handle_mouse` for click-inside/outside behavior).
    pub(in crate::app::render) fn render_context_menu(
        &mut self,
        _f: &mut Frame,
        layout: &mut AppLayout,
    ) {
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
    }
}

/// Paint the context menu at the given rect.
///
/// Extracted from `impl App::render_context_menu` so the Interactive
/// Component (`src/app/components/context_menu.rs`) can call it without an
/// `App` reference (design D9). The `rect` is computed by `App::render_context_menu`
/// (placement, which needs `AppLayout` geometry) and passed to the component
/// via downcast.
///
/// `entries` is a slice of `(label, is_selectable)` pairs — the component
/// doesn't need the `ContextAction` (which isn't `Clone`), only the label
/// and whether the entry is a separator (`is_selectable = false`).
//
// `pub(in crate::app)` so the Interactive Component can call it.
pub(in crate::app) fn render_context_menu_content(
    f: &mut Frame,
    rect: Rect,
    entries: &[(&'static str, bool)],
    cursor: usize,
) {
    // Dim the background content before drawing the (undimmed) menu.
    dim_backdrop(f);
    f.render_widget(Clear, rect);
    let list_items: Vec<ListItem> = entries
        .iter()
        .enumerate()
        .map(|(i, (label, is_selectable))| {
            let style = if !is_selectable {
                Style::default().fg(palette::TEXT_SECONDARY)
            } else if i == cursor {
                Style::default()
                    .fg(palette::TEXT_ON_ACCENT)
                    .bg(palette::ACCENT_ACTIVE)
            } else {
                Style::default().fg(palette::TEXT_PRIMARY)
            };
            ListItem::new(format!(" {} ", label)).style(style)
        })
        .collect();
    let inner = Rect {
        x: rect.x,
        y: rect.y.saturating_add(1),
        width: rect.width,
        height: entries.len() as u16,
    };
    f.render_widget(List::new(list_items), inner);
}
