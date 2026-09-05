use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::{Clear, List, ListItem};
use ratatui::Frame;

use crate::app::render::components::backdrop::dim_backdrop;
use crate::app::render::palette;

/// Paint the context menu at the given rect.
///
/// The Interactive Component (`src/app/components/context_menu.rs`) owns the
/// menu's `entries`, `cursor`, and `menu_rect`; it calls this function from
/// its `view()` (design D9). Placement is computed by the shell from
/// `AppLayout` and passed in as `rect` (task 5.3c removed the
/// `layout.context_menu_rect` global).
///
/// `entries` is a slice of `(label, is_selectable)` pairs — the component
/// doesn't need the `ContextAction` (which isn't `Clone`), only the label
/// and whether the entry is a separator (`is_selectable = false`).
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
