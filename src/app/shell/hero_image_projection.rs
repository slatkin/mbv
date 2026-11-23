//! The Library panel hero's image projection, TUI half (`extract-client-core`
//! design D3 and D13 Rule B): the fit rule is terminal geometry — the Wide
//! pane `hero_artwork_box` derives from the panel area — and the encoding is
//! ratatui-image's, so both live here. The fetch, the decode, and the
//! Resizing/Ready decision are the core's (`crate::app::projection::hero_image`);
//! the cache stays in `crate::app::infra::image_fetch` and the radatui-image
//! encoding in `crate::app::shell::image_protocol`.

use super::Model;
use mbv_render::{PANE_PAD_X, PANE_PAD_Y};

impl Model {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; the Wide header's cover-fit box is keyed by the box
    /// `hero_artwork_box` derives from it, so a resize, split drag, or
    /// Workspace shrink re-encodes at the new box size on the next sync pass
    /// and the placeholder shows for at most that one frame.
    pub(in crate::app) fn project_hero_image(
        &mut self,
        facts: &mbv_ui_model::library_hero::HeroFacts,
        workspace_present: bool,
        panel_area: ratatui::layout::Rect,
        list_pane_width: Option<u16>,
        overlay_box: Option<(u16, u16)>,
        // The shell's recorded terminal height (row 4.3): terminal geometry
        // lives on the `Model`, so the caller passes it in.
        terminal_height: u16,
        resizing: bool,
    ) -> mbv_ui_model::playback::HeroImageState {
        use crate::app::projection::HeroImageProjection;
        use mbv_ui_model::playback::HeroImageState as State;
        match self
            .app
            .project_hero_image(facts, panel_area, list_pane_width)
        {
            HeroImageProjection::State(state) => state,
            // The Wide Hero pane's own cover-fit box. Only a Movie Landscape
            // hero reaches here; the Logo is reserved only once a decoded
            // base exists to decorate, so the base image never waits on it.
            HeroImageProjection::WideLandscape { cache_key, decoded } => {
                let Some(panes) = mbv_render::arrangements::library::wide_library_panes(
                    panel_area,
                    PANE_PAD_X,
                    PANE_PAD_Y,
                    list_pane_width,
                    true,
                ) else {
                    return State::Ready {
                        cache_key,
                        decoded: Some(decoded),
                    };
                };
                let box_cells = mbv_components::library_panel::hero_header::hero_artwork_box(
                    panes.hero_area,
                    facts,
                    workspace_present,
                    terminal_height,
                );
                let logo_cache_key = self.app.fetch_wide_hero_logo(&facts.artwork, panel_area);
                if !self.ensure_hero_cover_protocol(
                    &cache_key,
                    (box_cells.width, box_cells.height),
                    logo_cache_key.as_deref(),
                    resizing,
                ) {
                    return State::Loading;
                }
                State::Ready {
                    cache_key,
                    decoded: Some(decoded),
                }
            }
            // Non-Wide Landscape artwork: the Library Hero overlay paints the
            // same reserved-box flow. A Landscape hero there is cover-fit for
            // the overlay's own box (its provider-link row stays plain, so no
            // Logo).
            HeroImageProjection::ReadyOverlay { cache_key, decoded } => {
                if let Some(box_cells) = overlay_box
                    && !self.ensure_hero_cover_protocol(&cache_key, box_cells, None, resizing)
                {
                    return State::Loading;
                }
                State::Ready {
                    cache_key,
                    decoded: Some(decoded),
                }
            }
        }
    }
}
