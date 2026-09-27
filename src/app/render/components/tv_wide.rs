//! TV content projection and paint-free breakpoint helpers (task 8.2,
//! unify-screens-under-panel-components).
//!
//! Task 8.2 deleted the Wide TV painter (`render_wide_tv_with_ctx`), and task
//! 8.3 moves Narrow through the same Library panel skeleton. What remains
//! here is the shell-projected content context the component consumes and
//! the paint-free breakpoint/area helpers the shell's mount/focus gates
//! read.

use crate::app::components::library_panel::content::HeroImageState;
use crate::app::render::components::list_rows::LibraryListRenderCtx;
use crate::app::ui_model::browse::SeriesDetail;
use mbv_emby_model::EmbyItem;

/// All App-derived data needed to paint the wide TV workspace.
#[derive(Clone)]
pub(in crate::app) struct TvWideRenderCtx {
    pub(in crate::app) list: LibraryListRenderCtx,
    pub(in crate::app) selected_series: Option<EmbyItem>,
    pub(in crate::app) series_detail: Option<SeriesDetail>,
    /// Loaded details for every listed show, keyed by series id. The tree
    /// projects children per show from this map so an expanded show keeps
    /// its loaded children while another show is selected; `series_detail`
    /// remains the selected show's detail for the workspace/hero paths.
    /// Empty in unit-test contexts that only push the selected detail.
    pub(in crate::app) series_details: std::collections::HashMap<String, SeriesDetail>,
    pub(in crate::app) season_cursor: usize,
    pub(in crate::app) episode_cursor: Option<usize>,
    pub(in crate::app) focused: bool,
    pub(in crate::app) show_letter_pills: bool,
    pub(in crate::app) tv_content_mode: Option<mbv_queue::TvContentMode>,
    /// The shell-projected hero image state (design D9/D16): the panel's
    /// shared `EmbyItem` producer reads it; painting never fetches.
    pub(in crate::app) hero_image: HeroImageState,
}

impl TvWideRenderCtx {
    pub(in crate::app) fn set_tv_content_mode(&mut self, mode: Option<mbv_queue::TvContentMode>) {
        self.tv_content_mode = mode;
    }

    pub(in crate::app) fn set_series_details(
        &mut self,
        details: std::collections::HashMap<String, SeriesDetail>,
    ) {
        self.series_details = details;
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
            series_details: std::collections::HashMap::new(),
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
