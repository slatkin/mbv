//! The Library panel hero's image projection policy (task 5.10, design D9;
//! `extract-client-core` design D3 and D13 Rule B): the part that issues the
//! artwork fetch and decides the projected image state the painter reads. The
//! fetch itself, the decode, and the cache stay in `crate::app::infra::image_fetch`.
//!
//! The terminal-geometry half — the Wide pane fit and the `hero_artwork_box`
//! the encoding is keyed by — and the ratatui-image encoding are the TUI
//! Client's, so they stay in the shell (`shell::hero_image_projection`,
//! `shell::image_protocol`), whose `Model` holds the protocol store. This
//! module only queues the encoding drop the shell performs (design D2 rule 3
//! of `split-tui-gui-clients`).

use crate::app::App;
use image::GenericImageView;
use mbv_render::{PANE_PAD_X, PANE_PAD_Y};

/// What one projection pass decided about a Library panel hero (design D9):
/// the image state the painter reads, and whether the shell's encoding must
/// run before the wide cover-fit state can be `Ready`.
pub(in crate::app) enum HeroImageProjection {
    /// A final state the shell paints as it is.
    State(mbv_ui_model::playback::HeroImageState),
    /// A ready image the wide Landscape hero paints: the shell keys one
    /// cover-fit encoding by the Wide pane's box, then reports `Ready` (or
    /// `Loading` while that encode completes).
    WideLandscape {
        cache_key: String,
        decoded: (u32, u32),
    },
    /// A ready image the Library Hero overlay paints: the shell keys one
    /// cover-fit encoding by the overlay's own box (no Logo).
    ReadyOverlay {
        cache_key: String,
        decoded: (u32, u32),
    },
}

impl App {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; a resize, split drag, or Workspace shrink re-keys the
    /// wide box on the next sync pass, so the placeholder shows for at most
    /// that one frame. Every other shape is decided here outright; the wide
    /// landscape hero hands its encoding box back to the shell.
    pub(in crate::app) fn project_hero_image(
        &mut self,
        facts: &mbv_ui_model::library_hero::HeroFacts,
        panel_area: ratatui::layout::Rect,
        list_pane_width: Option<u16>,
    ) -> HeroImageProjection {
        use HeroImageProjection::{ReadyOverlay, State, WideLandscape};
        use mbv_ui_model::library_hero::ArtworkSource;
        let artwork = &facts.artwork;
        let Some(source) = &artwork.source else {
            return State(mbv_ui_model::playback::HeroImageState::None);
        };
        if !self.images.protocol_enabled() {
            return State(mbv_ui_model::playback::HeroImageState::None);
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
            return State(mbv_ui_model::playback::HeroImageState::None);
        };
        let Some(decoded) = self.hero_artwork_decoded(&cache_key) else {
            // A fetch that resolved empty (or got no answer, so may never) is
            // final: the shared placeholder shows, and no encode is owed. Only
            // a fetch still in flight paints as loading.
            let entry = self.images.image(&cache_key);
            let settled = entry.is_some_and(|entry| !entry.has_art());
            if settled || self.images.is_fetch_interrupted(&cache_key) {
                return State(mbv_ui_model::playback::HeroImageState::None);
            }
            return State(mbv_ui_model::playback::HeroImageState::Loading);
        };
        // The fit rule follows the arm on every surface: Landscape artwork
        // is cover-fit (centre-cropped, no margin) for the box that paints
        // it — the Wide Hero pane's or the Library Hero overlay's;
        // Portrait/Square artwork fit-resizes (the whole image, aspect
        // preserved) through the plain protocol.
        let landscape =
            artwork.painted_shape() == mbv_ui_model::library_hero::ArtworkShape::Landscape;
        if !landscape {
            // Drop any stale cover crop so the plain fit protocol rebuilds,
            // and the stored encoding with it: the shell re-encodes from the
            // retained source at its next drain (design D2 rule 3).
            if self.clear_stale_hero_cover_box(&cache_key) {
                self.queue_protocol_clear(&cache_key);
            }
            return State(mbv_ui_model::playback::HeroImageState::Ready {
                cache_key,
                decoded: Some(decoded),
            });
        }
        // The optional Logo never delays the base image. A base that
        // resolved empty, and every non-Wide presentation, stays
        // undecorated and issues no Logo request.
        if mbv_render::wide_hero_fits(panel_area)
            && mbv_render::arrangements::library::wide_library_panes(
                panel_area,
                PANE_PAD_X,
                PANE_PAD_Y,
                list_pane_width,
                true,
            )
            .is_some()
        {
            return WideLandscape { cache_key, decoded };
        }
        ReadyOverlay { cache_key, decoded }
    }

    /// The decoded pixel size of a cached hero artwork, or `None` while the
    /// fetch is pending or resolved empty. The caller tells the two apart from
    /// the cache: a settled entry with no pixels means nothing will arrive, an
    /// absent or loading key means the fetch is in flight.
    fn hero_artwork_decoded(&self, cache_key: &str) -> Option<(u32, u32)> {
        if self.images.is_loading(cache_key) {
            return None;
        }
        let entry = self.images.image(cache_key)?;
        let source_img = entry.img.as_ref()?;
        Some(source_img.dimensions())
    }

    /// The Wide header's optional Logo fetch for a Landscape hero: a Movie's
    /// Landscape artwork at Wide geometry only, returning its cache key.
    pub(in crate::app) fn fetch_wide_hero_logo(
        &mut self,
        artwork: &mbv_ui_model::library_hero::HeroArtwork,
        panel_area: ratatui::layout::Rect,
    ) -> Option<String> {
        use mbv_ui_model::library_hero::ArtworkSource;
        if artwork.shape != mbv_ui_model::library_hero::ArtworkShape::Landscape
            || !mbv_render::wide_hero_fits(panel_area)
        {
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

    /// Drop a stale cover crop so the plain fit protocol rebuilds: the
    /// report is whether one was dropped.
    fn clear_stale_hero_cover_box(&mut self, cache_key: &str) -> bool {
        self.images
            .image_mut(cache_key)
            .is_some_and(|entry| entry.cover_box.take().is_some())
    }
}
