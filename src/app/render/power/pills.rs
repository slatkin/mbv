use super::LetterFilter;
use crate::app::layout::LayoutPower;
use crate::app::App;
use ratatui::layout::*;
use ratatui::Frame;

impl App {
    /// Renders the letter-range pill row (`A–C`, `D–F`, … `V–Z`, `#`) for a
    /// large non-music library's top level, inside `row_area`. A direct
    /// copy of `render_power_music_group_pills_row` (music.rs) with the
    /// bucket labels from `LetterFilter` in place of music-group names --
    /// music-group and letter-pill views are mutually exclusive (a library
    /// is either music or not), so both safely write `layout.selector_tabs`.
    pub(super) fn render_power_letter_pills_row(
        &mut self,
        f: &mut Frame,
        row_area: Rect,
        lib_idx: usize,
        layout: &mut LayoutPower,
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
        layout.selector_tabs = super::render_pill_bar(
            f,
            row_area,
            super::PillBar {
                labels: &labels,
                ids: &ids,
                selected_pos,
                prefix: None,
                underlay: super::PillUnderlay::Blank { fill: true },
            },
        );
    }
}
