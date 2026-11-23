//! The Library panel hero's image projection (`extract-client-core` design D3
//! and D13 Rule B): the one projection that issues a hero artwork fetch and
//! then projects the image state the painter reads. Its fit rule is terminal
//! geometry — `wide_hero_fits`, `PANE_PAD_X`/`PANE_PAD_Y`, the Wide panes, and
//! `hero_artwork_box` — so it lives in the shell. The fetch itself, the decode,
//! and the cache stay in `crate::app::infra::image_fetch`; the radatui-image
//! encoding stays in `crate::app::shell::image_protocol`.

use super::App;
use mbv_render::{PANE_PAD_X, PANE_PAD_Y};

fn wide_landscape_hero_eligible(
    artwork: &mbv_ui_model::library_hero::HeroArtwork,
    panel_area: ratatui::layout::Rect,
) -> bool {
    artwork.shape == mbv_ui_model::library_hero::ArtworkShape::Landscape
        && mbv_render::wide_hero_fits(panel_area)
}

impl App {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; the Wide header's cover-fit box is keyed by the box
    /// `hero_artwork_box` derives from it, so a resize, split drag, or
    /// Workspace shrink re-encodes at the new box size on the next sync pass
    /// and the placeholder shows for at most that one frame.
    fn fetch_hero_logo(
        &mut self,
        artwork: &mbv_ui_model::library_hero::HeroArtwork,
        panel_area: ratatui::layout::Rect,
    ) -> Option<String> {
        use mbv_ui_model::library_hero::ArtworkSource;
        if !wide_landscape_hero_eligible(artwork, panel_area) {
            return None;
        }
        let Some(ArtworkSource::Emby {
            item_id,
            series_id,
            image_types,
            cache_key,
        }) = artwork.decoration.as_ref()
        else {
            return None;
        };
        let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
        self.fetch_card_image(
            cache_key.clone(),
            item_id.clone(),
            series_id.clone(),
            &types,
        );
        Some(cache_key.clone())
    }

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
        use mbv_ui_model::library_hero::ArtworkSource;
        use mbv_ui_model::playback::HeroImageState as State;
        let artwork = &facts.artwork;
        let Some(source) = &artwork.source else {
            return State::None;
        };
        if !self.images.protocol_enabled() {
            return State::None;
        }
        // The one fetch per key (fetch dedupes on its own reservation set).
        let cache_key: Option<String> = match source {
            ArtworkSource::Emby {
                item_id,
                series_id,
                image_types,
                cache_key,
                ..
            } => {
                let types: Vec<&str> = image_types.iter().map(String::as_str).collect();
                self.fetch_card_image(
                    cache_key.clone(),
                    item_id.clone(),
                    series_id.clone(),
                    &types,
                );
                Some(cache_key.clone())
            }
            ArtworkSource::AudiobookshelfCover {
                library_item_id,
                book,
            } => {
                if *book {
                    self.audiobookshelf_book_cover_key(library_item_id)
                } else {
                    self.audiobookshelf_cover_key(library_item_id)
                }
            }
        };
        let Some(cache_key) = cache_key else {
            return State::None;
        };
        if self.images.is_loading(&cache_key) {
            return State::Loading;
        }
        let Some(entry) = self.images.image(&cache_key) else {
            return State::Loading;
        };
        let Some(source_img) = entry.img.as_ref() else {
            // Resolved-empty fetch: no artwork exists; the placeholder is
            // final.
            return State::None;
        };
        let decoded = {
            use image::GenericImageView;
            Some(source_img.dimensions())
        };
        // The fit rule follows the arm on every surface: Landscape artwork
        // is cover-fit (centre-cropped, no margin) for the box that paints
        // it — the Wide Hero pane's or the Library Hero overlay's;
        // Portrait/Square artwork fit-resizes (the whole image, aspect
        // preserved) through the plain protocol.
        let landscape =
            artwork.painted_shape() == mbv_ui_model::library_hero::ArtworkShape::Landscape;
        if !landscape {
            // Drop any stale cover crop so the plain fit protocol rebuilds.
            if let Some(entry) = self.images.image_mut(&cache_key)
                && entry.cover_box.take().is_some()
            {
                self.image_protocols.clear_key(&cache_key);
            }
        } else if mbv_render::wide_hero_fits(panel_area) {
            if let Some(panes) = mbv_render::arrangements::library::wide_library_panes(
                panel_area,
                PANE_PAD_X,
                PANE_PAD_Y,
                list_pane_width,
                true,
            ) {
                let box_cells = mbv_components::library_panel::hero_header::hero_artwork_box(
                    panes.hero_area,
                    facts,
                    workspace_present,
                    terminal_height,
                );
                // The optional Logo never delays the base image: it is
                // reserved only here, once a decoded base exists to decorate,
                // and only for a Movie Landscape hero at Wide geometry. A base
                // that resolved empty, and every other presentation, stays
                // undecorated and issues no Logo request.
                let logo_cache_key = self.fetch_hero_logo(artwork, panel_area);
                if !self.ensure_hero_cover_protocol(
                    &cache_key,
                    (box_cells.width, box_cells.height),
                    logo_cache_key.as_deref(),
                    resizing,
                ) {
                    return State::Loading;
                }
            }
        } else if let Some(box_cells) = overlay_box {
            // The Library Hero overlay paints the same reserved-box flow; a
            // Landscape hero there is cover-fit for the overlay's own box
            // (its provider-link row stays plain, so no Logo).
            if !self.ensure_hero_cover_protocol(&cache_key, box_cells, None, resizing) {
                return State::Loading;
            }
        }
        State::Ready { cache_key, decoded }
    }
}
