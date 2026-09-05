use crate::app::layout::LayoutMain;
// Music-group selector painting is a component; screen state remains in App.
use crate::app::render::components::widgets::{render_pill_bar, PillBar};
use crate::app::ui_util::*;
use mbv_core::api::EmbyItem;
use ratatui::layout::*;
use ratatui::text::*;
use ratatui::widgets::*;
use ratatui::Frame;

pub(in crate::app) fn render_music_group_pills_row_with_ctx(
    f: &mut Frame,
    row_area: Rect,
    groups: &[EmbyItem],
    group_cursor: usize,
    layout: &mut LayoutMain,
) {
    if groups.is_empty() || row_area.width == 0 {
        layout.selector_tabs = Vec::new();
        if row_area.width > 0 {
            f.render_widget(
                Paragraph::new(Line::from(Span::raw(" ".repeat(row_area.width as usize)))),
                row_area,
            );
        }
        return;
    }

    const MAX_LABEL: usize = 12;
    let labels: Vec<String> = groups
        .iter()
        .map(|g| trunc_str(&g.name, MAX_LABEL).to_string())
        .collect();
    // Music-group tabs are identified by their 0-based group index.
    let ids: Vec<usize> = (0..labels.len()).collect();
    layout.selector_tabs = render_pill_bar(
        f,
        row_area,
        PillBar {
            labels: &labels,
            ids: &ids,
            selected_pos: group_cursor,
            prefix: Some(" ⌘ "),
        },
    );
}
