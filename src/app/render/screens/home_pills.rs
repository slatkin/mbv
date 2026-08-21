use crate::app::layout::LayoutMain;
use crate::app::render::components::widgets::{render_pill_bar, PillBar};
use crate::app::ui_util::*;
use crate::app::App;
use ratatui::layout::*;
use ratatui::Frame;

impl App {
    pub(in crate::app::render) fn render_home_section_pills_row(
        &mut self,
        f: &mut Frame,
        area: Rect,
        layout: &mut LayoutMain,
    ) {
        if area.width == 0 || area.height == 0 {
            layout.selector_tabs = Vec::new();
            return;
        }

        let mut labels: Vec<(usize, String)> = vec![(0, "Continue".to_string())];
        for (idx, (title, _lib, _items, _cur)) in self.home.latest.iter().enumerate() {
            // Every section in `home.latest` is a real pill (an ABS library,
            // an Emby view, or Feeds). Match the Continue Watching convention:
            // the pill always renders even when its section is empty (which
            // shows an "(empty)" row), so a bare section is still discoverable.
            labels.push((idx + 1, title.clone()));
        }
        // Restore the last-selected Home pill from prefs once a section with
        // that source identity exists (sections arrive asynchronously across
        // providers). Keep it pending until the section appears.
        if let Some(pending) = self.home_section_pending.as_ref() {
            if let Some((idx, _)) = self
                .home
                .latest
                .iter()
                .enumerate()
                .find(|(_, (_, source, _, _))| source == pending)
            {
                self.home.section = idx + 1;
                self.home_section_pending = None;
            }
        }
        if !labels
            .iter()
            .any(|(section_idx, _)| *section_idx == self.home.section)
        {
            self.home.section = labels[0].0;
        }

        const MAX_LABEL: usize = 18;
        let selected_pos = labels
            .iter()
            .position(|(section_idx, _)| *section_idx == self.home.section)
            .unwrap_or(0);
        // Pre-truncated pill labels; ids are the section indices (idx+1) used
        // as click targets, distinct from the pill's display position.
        let label_strs: Vec<String> = labels
            .iter()
            .map(|(_, label)| trunc_str(label, MAX_LABEL).to_string())
            .collect();
        let ids: Vec<usize> = labels.iter().map(|(section_idx, _)| *section_idx).collect();
        layout.selector_tabs = render_pill_bar(
            f,
            area,
            PillBar {
                labels: &label_strs,
                ids: &ids,
                selected_pos,
                prefix: Some(" ⌘ "),
            },
        );
    }
}
