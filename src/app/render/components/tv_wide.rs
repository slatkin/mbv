//! TV content projection and paint-free breakpoint helpers (task 8.2,
//! unify-screens-under-panel-components).
//!
//! Task 8.2 deleted the Wide TV painter (`render_wide_tv_with_ctx`), and task
//! 8.3 moves Narrow through the same Library panel skeleton. What remains
//! here is the shell-projected content context the component consumes and
//! the paint-free breakpoint/area helpers the shell's mount/focus gates
//! read.

use crate::app::components::library_panel::content::HeroImageState;
use crate::app::render::arrangements::wide_hero;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::{App, PanelMode, SeriesDetail};
use mbv_core::api::EmbyItem;
use ratatui::layout::Rect;

/// All App-derived data needed to paint the wide TV workspace.
#[derive(Clone)]
pub(in crate::app) struct TvWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) selected_series: Option<EmbyItem>,
    pub(in crate::app) series_detail: Option<SeriesDetail>,
    pub(in crate::app) season_cursor: usize,
    pub(in crate::app) episode_cursor: Option<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) show_letter_pills: bool,
    pub(in crate::app) tv_content_mode: Option<mbv_core::config::TvContentMode>,
    /// The shell-projected hero image state (design D9/D16): the panel's
    /// shared `EmbyItem` producer reads it; painting never fetches.
    pub(in crate::app) hero_image: HeroImageState,
}

impl TvWideRenderCtx {
    pub(in crate::app) fn set_tv_content_mode(
        &mut self,
        mode: Option<mbv_core::config::TvContentMode>,
    ) {
        self.tv_content_mode = mode;
    }

    pub(in crate::app) fn new(
        list: LibraryListRenderCtx,
        selected_series: Option<EmbyItem>,
        series_detail: Option<SeriesDetail>,
        season_cursor: usize,
        episode_cursor: Option<usize>,
        show_letter_pills: bool,
    ) -> Self {
        Self {
            list,
            selected_series,
            series_detail,
            season_cursor,
            episode_cursor,
            // Framework focus is owned by the mounted `LibraryPanel` and
            // applied from `Attribute::Focus`; content projection never sets it.
            focused: false,
            show_letter_pills,
            tv_content_mode: None,
            hero_image: HeroImageState::None,
        }
    }
}

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

    /// The right panel's content area for the current terminal size and
    /// panel state, paint-free — `None` when the right panel is not visible
    /// (e.g. Queue-only panel mode). Factored out of `wide_tv_library_area`
    /// so every paint-free breakpoint consumer shares one pipeline.
    fn right_panel_lib_area(&self) -> Option<Rect> {
        let chrome = crate::app::render::arrangements::chrome::chrome_geometry(
            crate::app::render::arrangements::chrome::ChromeGeometryInput {
                area: Rect::new(0, 0, self.terminal_width, self.terminal_height),
                panel_mode: self.effective_panel_mode(),
                panel_focus: self.effective_panel_focus(),
                queue_column_width: self.queue_column_width,
                terminal_width: self.terminal_width,
                card_height: self.layout.card.height,
                playback_active: self.effective_playback_state().active,
                queue_title_expanded: self.transport_title_expanded(),
            },
        );
        if !chrome.right_visible {
            return None;
        }
        Some(
            crate::app::render::components::widgets::right_panel_content_area(
                chrome.right_area,
                self.effective_panel_mode() != PanelMode::Both,
            ),
        )
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

    /// The finalized library content rect when the wide Wide hero TV
    /// workspace owns `lib_idx`, computed paint-free from the current
    /// terminal size — `None` when the library is not a wide-TV series list
    /// or the breakpoint is narrow. Mirrors the exact library-panel gate
    /// applies (`is_wide_tv_library` + `wide_hero_fits` on the
    /// finalized area), so component mount/focus can be routed a frame
    /// earlier than the deleted previous-frame paint signal this predicate
    /// replaced, which used to flash the narrow browser on entry.
    pub(in crate::app) fn wide_tv_library_area(&self, lib_idx: usize) -> Option<Rect> {
        if !self.is_wide_tv_library(lib_idx) {
            return None;
        }
        let lib_area = self.right_panel_lib_area()?;
        wide_hero::wide_hero_fits(lib_area).then_some(lib_area)
    }
}
