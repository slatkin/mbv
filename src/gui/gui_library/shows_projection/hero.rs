//! The Shows destination's Hero (add-gui-library-panel design D12, D14;
//! gui-collection-dropdown design D8): the shared series producer's facts for
//! the selected series, the `Primary` poster arm this Client paints instead of
//! the artwork policy's landscape chain, the series Hero's Workspace, and the
//! neighbour artwork warm. The projection's own vocabulary and list rules live
//! one level up.

use mbv_emby_model::EmbyItem;
use mbv_ui_model::library_hero::producers::{HeroContentData, hero_content_series_with_episode};
use mbv_ui_model::library_hero::{ArtworkShape, ArtworkSource};

use super::super::home::artwork_shape;
use super::{GuiShowsHero, GuiShowsProjection, GuiShowsSeasonPill, GuiShowsWorkspace, episode_row};
use crate::gui::GuiCore;
use crate::gui::gui_client_state::GuiClientState;
use mbv_client_core::App;

/// The season whose episodes the Workspace shows (design D12): the stored
/// choice while the detail holds it, else the first season — the season pills
/// start on the first season and are not kept across a series change. `None`
/// while the detail holds no season. It reads the core's detail cache and the
/// destination's stored selection, so it takes both the `App` and the Client's
/// state, as the Hero and the destination's load site do.
pub(crate) fn shows_selected_season(
    app: &App,
    gui: &GuiClientState,
    lib_idx: usize,
    series_id: &str,
) -> Option<String> {
    let detail = app.series_detail(series_id)?;
    let stored = gui
        .tv_shows
        .get(&lib_idx)
        .cloned()
        .unwrap_or_default()
        .season_id;
    stored
        .filter(|stored| detail.seasons.iter().any(|season| &season.id == stored))
        .or_else(|| detail.seasons.first().map(|season| season.id.clone()))
}

/// The artwork source the Hero paints for `item_id`, from the same producer and
/// poster arm the Hero's own fetch reads. It reads only the core's library
/// state, so it is a core function the neighbour warm reaches through the App
/// it already has.
fn shows_hero_source(
    app: &App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) -> Option<ArtworkSource> {
    let item = super::shows_item(app, gui, lib_idx, item_id)?;
    hero_content_series_with_episode(&item, None, false, None)
        .facts
        .artwork
        .source
}

/// The series Hero's artwork: the series' `Primary` image, as the Music Hero
/// paints its album's, instead of the artwork policy's landscape chain. A
/// series keeps its `:ser:` cache key family under the `Primary` chain and
/// declares the portrait arm its poster paints. A Hero with no declared source
/// keeps none, so the placeholder paints, and the fetched image re-arms the
/// shape by its own pixels either way.
fn primary_poster(mut content: HeroContentData) -> HeroContentData {
    if let Some(ArtworkSource::Emby {
        item_id,
        image_types,
        cache_key,
        ..
    }) = content.facts.artwork.source.as_mut()
    {
        *image_types = vec!["Primary".to_owned()];
        *cache_key = mbv_images::series_image_cache_key(item_id, &["Primary"]);
        content.facts.artwork.shape = ArtworkShape::Portrait;
    }
    content
}

impl GuiCore {
    /// The selected series' Hero (design D12): the shared series producer's
    /// facts, the series' `Primary` poster, and the Workspace the series detail
    /// holds.
    pub(super) fn shows_hero(&mut self, lib_idx: usize, selected: &str) -> Option<GuiShowsHero> {
        let series = super::shows_item(&self.app, &self.gui, lib_idx, selected)?;
        let seasons = self
            .app
            .series_detail(selected)
            .map(|detail| detail.seasons.len());
        let workspace = self.gui_shows_workspace(lib_idx, selected);
        Some(self.gui_shows_series_hero_facts(&series, seasons, workspace))
    }

    /// The TUI series producer's facts for one series (design D14): the genre,
    /// the show's year range (else the release date), the season count, and
    /// the network, with no duration row while the Workspace has no focus —
    /// `hero_content_series_with_episode`, the producer the TUI's series Hero
    /// paints. The season count comes from the cached detail, so the row
    /// appears as soon as the detail lands.
    fn gui_shows_series_hero_facts(
        &mut self,
        series: &EmbyItem,
        seasons: Option<usize>,
        workspace: Option<GuiShowsWorkspace>,
    ) -> GuiShowsHero {
        let content = primary_poster(hero_content_series_with_episode(
            series, None, false, seasons,
        ));
        self.shows_hero_from(&content, workspace)
    }

    /// One hero's facts from a shared producer's output, with its artwork
    /// fetched through the image cache at the producer's declared chain.
    fn shows_hero_from(
        &mut self,
        content: &HeroContentData,
        workspace: Option<GuiShowsWorkspace>,
    ) -> GuiShowsHero {
        let art = super::super::artwork::emby_hero_art(
            &mut self.app,
            content.facts.artwork.source.as_ref(),
        );
        GuiShowsHero {
            title: content.facts.title.clone(),
            parent: None,
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(super::super::artwork::hero_shape(
                &self.app,
                content.facts.artwork.shape,
                art.as_ref(),
            )),
            art,
            workspace,
        }
    }

    /// A series Hero's Workspace (design D12): the detail's season pills and
    /// the selected season's episodes. `None` while the series detail has
    /// not landed — the projection's `workspace_loading` names that load.
    fn gui_shows_workspace(&self, lib_idx: usize, series_id: &str) -> Option<GuiShowsWorkspace> {
        let detail = self.app.series_detail(series_id)?;
        let selected_season = shows_selected_season(&self.app, &self.gui, lib_idx, series_id)?;
        let rows = detail
            .episodes
            .get(&selected_season)
            .map(|episodes| {
                episodes
                    .iter()
                    .enumerate()
                    .map(|(position, episode)| episode_row(episode, position))
                    .collect()
            })
            .unwrap_or_default();
        let seasons = detail
            .seasons
            .iter()
            .map(|season| GuiShowsSeasonPill {
                season_id: season.id.clone(),
                name: season.name.clone(),
                selected: selected_season == season.id,
            })
            .collect();
        Some(GuiShowsWorkspace {
            seasons,
            selected_season: Some(selected_season),
            episodes: rows,
        })
    }

    /// Whether the Workspace's loads have not landed yet (design D12): the
    /// selected series' detail fetch, or the selected season's episodes
    /// fetch — the in-flight marks the shared fetch helpers keep.
    pub(super) fn shows_workspace_loading(&self, lib_idx: usize, series_id: &str) -> bool {
        if self.app.series_detail_loading(series_id) {
            return true;
        }
        shows_selected_season(&self.app, &self.gui, lib_idx, series_id)
            .is_some_and(|season_id| self.app.series_season_loading(series_id, &season_id))
    }

    /// Warm the Hero artwork of the entries neighbouring the picked one in the
    /// painted series list's order, through the shared navigation-idle gate.
    /// The pick's own art is the Hero's fetch, so the picked entry is never
    /// warmed here.
    pub(super) fn warm_shows_hero_neighbours(
        &mut self,
        lib_idx: usize,
        projection: &GuiShowsProjection,
    ) {
        let Some(picked) = projection.selected.as_deref() else {
            return;
        };
        let ids: Vec<&str> = projection
            .series
            .iter()
            .map(|series| series.id.as_str())
            .collect();
        let Some(picked_index) = ids.iter().position(|id| *id == picked) else {
            return;
        };
        let gui = &self.gui;
        super::super::artwork::warm_hero_neighbours(
            &mut self.app,
            &ids,
            picked_index,
            |app, id| shows_hero_source(app, gui, lib_idx, id),
        );
    }
}
