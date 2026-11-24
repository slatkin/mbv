//! The Library panel hero's image projection, the TUI Client's own
//! (`extract-client-core` design D3 and D13 Rule B): the fit rule is terminal
//! geometry — the Wide pane `hero_artwork_box` derives from the panel area, and
//! the wide-vs-overlay verdict is `mbv_render`'s — and the encoding is
//! ratatui-image's, so the whole projection policy lives here. The fetch, the
//! decode, the cache, and the queued encoding drop stay in the core
//! (`App::fetch_card_image`, `App::audiobookshelf_cover_key`,
//! `App::image_cache`, `App::clear_image_cover_box`); the radatui-image
//! encoding is `crate::tui::image_protocol`.

use super::Model;
use image::GenericImageView;
use mbv_client_core::App;
use mbv_render::{PANE_PAD_X, PANE_PAD_Y};

/// What one projection pass decided about a Library panel hero (task 5.10,
/// design D9): the image state the painter reads, and whether the TUI's
/// encoding must run before the wide cover-fit state can be `Ready`.
enum HeroImageProjection {
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

/// One panel hero's projected image state (task 5.10, design D9): the
/// projection — never the painter — issues every fetch, then projects the
/// state painting reads. `panel_area` is the Library panel's `RootFrame`
/// content area; a resize, split drag, or Workspace shrink re-keys the wide
/// box on the next sync pass, so the placeholder shows for at most that one
/// frame. Every other shape is decided here outright; the wide landscape hero
/// hands its encoding box back to the shell.
fn project_hero(
    app: &mut App,
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
    if !app.image_cache().protocol_enabled() {
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
            app.fetch_card_image(
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
                app.audiobookshelf_book_cover_key(library_item_id)
            } else {
                app.audiobookshelf_cover_key(library_item_id)
            }
        }
    };
    let Some(cache_key) = cache_key else {
        return State(mbv_ui_model::playback::HeroImageState::None);
    };
    let Some(decoded) = hero_artwork_decoded(app, &cache_key) else {
        // A settled entry holds no decoded pixels — the fetch resolved empty,
        // or this Client keeps the server's bytes only. Either way nothing
        // more will arrive through this projection, so the shared placeholder
        // is final. An interrupted fetch wrote no entry and may still succeed
        // on its retry (invariant 27), so a missing key stays `Loading` like a
        // fetch in flight.
        let settled = app
            .image_cache()
            .image(&cache_key)
            .is_some_and(|entry| entry.img.is_none());
        if settled {
            return State(mbv_ui_model::playback::HeroImageState::None);
        }
        return State(mbv_ui_model::playback::HeroImageState::Loading);
    };
    // The fit rule follows the arm on every surface: Landscape artwork is
    // cover-fit (centre-cropped, no margin) for the box that paints it — the
    // Wide Hero pane's or the Library Hero overlay's; Portrait/Square artwork
    // fit-resizes (the whole image, aspect preserved) through the plain
    // protocol.
    let landscape = artwork.painted_shape() == mbv_ui_model::library_hero::ArtworkShape::Landscape;
    if !landscape {
        // Drop any stale cover crop so the plain fit protocol rebuilds, and
        // the stored encoding with it: the shell re-encodes from the retained
        // source at its next drain (design D2 rule 3).
        app.clear_image_cover_box(&cache_key);
        return State(mbv_ui_model::playback::HeroImageState::Ready {
            cache_key,
            decoded: Some(decoded),
        });
    }
    // The optional Logo never delays the base image. A base that resolved
    // empty, and every non-Wide presentation, stays undecorated and issues no
    // Logo request.
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

/// The decoded pixel size of a cached hero artwork, or `None` while the fetch
/// is pending or resolved empty. The caller tells the two apart from the
/// cache: a settled entry with no pixels means nothing will arrive, an absent
/// or loading key means the fetch is in flight.
fn hero_artwork_decoded(app: &App, cache_key: &str) -> Option<(u32, u32)> {
    if app.image_cache().is_loading(cache_key) {
        return None;
    }
    let entry = app.image_cache().image(cache_key)?;
    let source_img = entry.img.as_ref()?;
    Some(source_img.dimensions())
}

/// The Wide header's optional Logo fetch for a Landscape hero: a Movie's
/// Landscape artwork at Wide geometry only, returning its cache key.
fn fetch_wide_hero_logo(
    app: &mut App,
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
    app.fetch_card_image(
        cache_key.clone(),
        item_id.clone(),
        series_id.clone(),
        &types,
    );
    Some(cache_key.clone())
}

impl Model {
    /// One panel hero's projected image state (task 5.10, design D9): the
    /// projection — never the painter — issues every fetch, then projects the
    /// state painting reads. `panel_area` is the Library panel's `RootFrame`
    /// content area; the Wide header's cover-fit box is keyed by the box
    /// `hero_artwork_box` derives from it, so a resize, split drag, or
    /// Workspace shrink re-encodes at the new box size on the next sync pass
    /// and the placeholder shows for at most that one frame.
    pub(in crate::tui) fn project_hero_image(
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
        use mbv_ui_model::playback::HeroImageState as State;
        match project_hero(&mut self.app, facts, panel_area, list_pane_width) {
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
                let logo_cache_key =
                    fetch_wide_hero_logo(&mut self.app, &facts.artwork, panel_area);
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
