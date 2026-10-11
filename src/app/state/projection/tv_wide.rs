use crate::app::{App, PanelMode};
use mbv_render::arrangements::wide_hero;
use mbv_render::components::widgets;
use ratatui::layout::Rect;

impl App {
    /// Whether the wide Wide hero TV workspace owns `lib_idx`, computed
    /// paint-free from the current terminal size — `None` when the library
    /// is not a TV library in a show mode (`All` or a range) or the
    /// breakpoint is narrow. The show mode comes from the current level's
    /// `tv_content_mode`, falling back to the library's exactly as
    /// `push_tv_workspace_content` does — never from row types. Mirrors the
    /// exact gate the Library panel applies (`wide_hero_fits` on the
    /// finalized area), so component mount/focus can be routed a frame
    /// earlier than the deleted previous-frame paint signal this predicate
    /// replaced, which used to flash the narrow browser on entry.
    pub(in crate::app) fn wide_tv_show_area(&self, lib_idx: usize) -> Option<Rect> {
        let library = self.libs.get(lib_idx)?;
        if library.library.collection_type != "tvshows" {
            return None;
        }
        let show_mode = library
            .nav_stack
            .last()
            .and_then(|level| level.tv_content_mode.clone())
            .or_else(|| library.tv_content_mode.clone());
        if !matches!(
            show_mode,
            Some(mbv_queue::TvContentMode::All | mbv_queue::TvContentMode::Range(_))
        ) {
            return None;
        }
        let lib_area = self.right_panel_lib_area()?;
        wide_hero::wide_hero_fits(lib_area).then_some(lib_area)
    }

    /// The right panel's content area for the current terminal size and
    /// panel state, paint-free — `None` when the right panel is not visible
    /// (e.g. Queue-only panel mode). Factored out of `wide_tv_show_area`
    /// so every paint-free breakpoint consumer shares one pipeline.
    fn right_panel_lib_area(&self) -> Option<Rect> {
        let chrome = mbv_render::arrangements::chrome::chrome_geometry(
            mbv_render::arrangements::chrome::ChromeGeometryInput {
                area: Rect::new(0, 0, self.terminal_width, self.terminal_height),
                panel_mode: self.effective_panel_mode(),
                panel_focus: self.effective_panel_focus(),
                queue_column_width: self.queue_column_width,
                terminal_width: self.terminal_width,
                card_height: self.layout.card.height,
                playback_active: self.effective_playback_state().active,
            },
        );
        if !chrome.right_visible {
            return None;
        }
        Some(widgets::right_panel_content_area(
            chrome.right_area,
            self.effective_panel_mode() != PanelMode::Both,
        ))
    }

    /// Whether the right panel is in the wide Wide hero breakpoint right
    /// now, derived paint-free from the current terminal size. Replaces the
    /// four deleted `is_wide_*_active()` paint-inference predicates: the
    /// breakpoint (`wide_hero_fits`) is the same for every
    /// Wide hero destination, so one predicate serves all of them.
    pub(in crate::app) fn is_right_panel_wide(&self) -> bool {
        self.right_panel_lib_area()
            .is_some_and(wide_hero::wide_hero_fits)
    }
}
