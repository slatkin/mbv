use super::sort_filter::LetterFilter;
use crate::app::layout::LayoutMain;
use crate::app::render::components::widgets::{render_pill_bar, PillBar};
use crate::app::App;
use ratatui::layout::*;
use ratatui::Frame;

impl App {
    /// Renders the letter-range pill row (`A–C`, `D–F`, … `V–Z`, `#`) for a
    /// large non-music library's top level, inside `row_area`. A direct
    /// copy of `render_music_group_pills_row` (music.rs) with the
    /// bucket labels from `LetterFilter` in place of music-group names --
    /// music-group and letter-pill views are mutually exclusive (a library
    /// is either music or not), so both safely write `layout.selector_tabs`.
    pub(in crate::app::render) fn render_letter_pills_row(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        lib_idx: usize,
        layout: &mut LayoutMain,
    ) {
        if row_area.width == 0 {
            layout.selector_tabs = Vec::new();
            return;
        }
        let selected_pos = self.libs[lib_idx]
            .nav_stack
            .last()
            .and_then(|l| l.letter_filter.as_ref())
            .map(|f| f.index)
            .unwrap_or(0);
        let labels = LetterFilter::labels();
        let ids: Vec<usize> = (0..labels.len()).collect();
        layout.selector_tabs = render_pill_bar(
            f,
            row_area,
            PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos,
                prefix: Some(" ⌘ "),
            },
        );
    }
}
