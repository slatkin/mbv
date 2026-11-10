//! The Shows destination's Hero (add-gui-library-panel design D12, D14): the
//! shared producers' facts for the selected series or episode, the `Primary`
//! poster arm this Client paints instead of the artwork policy's landscape
//! chain, the series Hero's Workspace, and the neighbour artwork warm. The
//! projection's own vocabulary and list rules live one level up.

use mbv_components::library_panel::content::{ArtworkShape, ArtworkSource};
use mbv_components::library_panel::hero::{
    HeroContentData, hero_content_emby, hero_content_series_with_episode,
};
use mbv_emby_model::EmbyItem;

use super::super::home::artwork_shape;
use super::{
    GuiShowsHero, GuiShowsMode, GuiShowsProjection, GuiShowsSeasonPill, GuiShowsWorkspace,
    episode_row,
};
use crate::app::App;

/// The Shows Hero's artwork: the item's `Primary` image, as the Music Hero
/// paints its album's, instead of the artwork policy's landscape chain. A
/// series keeps its `:ser:` cache key family under the `Primary` chain and
/// declares the portrait arm its poster paints; an episode's `Primary` is its
/// landscape still, so it keeps the policy's declared arm. A Hero with no
/// declared source keeps none, so the placeholder paints, and the fetched
/// image re-arms the shape by its own pixels either way.
fn primary_poster(mut content: HeroContentData, series: bool) -> HeroContentData {
    if let Some(ArtworkSource::Emby {
        item_id,
        image_types,
        cache_key,
        ..
    }) = content.facts.artwork.source.as_mut()
    {
        *image_types = vec!["Primary".to_owned()];
        *cache_key = if series {
            mbv_images::series_image_cache_key(item_id, &["Primary"])
        } else {
            format!("{item_id}:Primary")
        };
        if series {
            content.facts.artwork.shape = ArtworkShape::Portrait;
        }
    }
    content
}

impl App {
    /// The selected row's Hero (design D12): the All series mode's series
    /// Hero — the shared Emby hero producer's facts, the artwork policy's
    /// landscape chain, and the Workspace the series detail holds — or the
    /// Latest mode's episode Hero. The Upcoming mode projects no Hero at
    /// all.
    pub(super) fn gui_shows_hero(
        &mut self,
        lib_idx: usize,
        mode: GuiShowsMode,
        selected: &str,
    ) -> Option<GuiShowsHero> {
        match mode {
            GuiShowsMode::Upcoming => None,
            GuiShowsMode::All => {
                let series = self.gui_shows_item(lib_idx, selected)?;
                let seasons = self
                    .series_detail_cache
                    .get(selected)
                    .map(|detail| detail.seasons.len());
                let workspace = self.gui_shows_workspace(lib_idx, selected);
                Some(self.gui_shows_series_hero_facts(&series, seasons, workspace))
            }
            GuiShowsMode::Latest => {
                let episode = self.gui_shows_item(lib_idx, selected)?;
                Some(self.gui_shows_hero_facts(&episode, None))
            }
        }
    }

    /// The shared Emby hero producer's facts for one item, with its artwork
    /// fetched through the image cache at the artwork policy's declared
    /// chain — the same facts and fetch the TUI's hero paints.
    fn gui_shows_hero_facts(
        &mut self,
        item: &EmbyItem,
        workspace: Option<GuiShowsWorkspace>,
    ) -> GuiShowsHero {
        let content = primary_poster(hero_content_emby(item), false);
        self.gui_shows_hero_from(&content, workspace)
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
        let content = primary_poster(
            hero_content_series_with_episode(series, None, false, seasons),
            true,
        );
        self.gui_shows_hero_from(&content, workspace)
    }

    /// One hero's facts from a shared producer's output, with its artwork
    /// fetched through the image cache at the producer's declared chain.
    fn gui_shows_hero_from(
        &mut self,
        content: &HeroContentData,
        workspace: Option<GuiShowsWorkspace>,
    ) -> GuiShowsHero {
        let art = self.gui_emby_hero_art(content.facts.artwork.source.as_ref());
        GuiShowsHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(
                self.gui_hero_shape(content.facts.artwork.shape, art.as_ref()),
            ),
            art,
            workspace,
        }
    }

    /// A series Hero's Workspace (design D12): the detail's season pills and
    /// the selected season's episodes. `None` while the series detail has
    /// not landed — the projection's `workspace_loading` names that load.
    fn gui_shows_workspace(&self, lib_idx: usize, series_id: &str) -> Option<GuiShowsWorkspace> {
        let detail = self.series_detail_cache.get(series_id)?;
        let selected_season = self.gui_shows_selected_season(lib_idx, series_id)?;
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

    /// The season whose episodes the Workspace shows (design D12): the
    /// stored choice while the detail holds it, else the first season — the
    /// season pills start on the first season and are not kept across a
    /// series change. `None` while the detail holds no season.
    pub(super) fn gui_shows_selected_season(
        &self,
        lib_idx: usize,
        series_id: &str,
    ) -> Option<String> {
        let detail = self.series_detail_cache.get(series_id)?;
        let stored = self.gui_shows_selection(lib_idx).season_id;
        stored
            .filter(|stored| detail.seasons.iter().any(|season| &season.id == stored))
            .or_else(|| detail.seasons.first().map(|season| season.id.clone()))
    }

    /// Whether the Workspace's loads have not landed yet (design D12): the
    /// selected series' detail fetch, or the selected season's episodes
    /// fetch — the in-flight marks the shared fetch helpers keep.
    pub(super) fn gui_shows_workspace_loading(&self, lib_idx: usize, series_id: &str) -> bool {
        if self.series_detail_loading.contains(series_id) {
            return true;
        }
        self.gui_shows_selected_season(lib_idx, series_id)
            .is_some_and(|season_id| {
                self.series_season_loading
                    .contains(&(series_id.to_string(), season_id))
            })
    }

    /// Warm the Hero artwork of the entries neighbouring the picked one in the
    /// painted list's order — the All mode's series nodes, the flat modes'
    /// rows — through the shared navigation-idle gate. The pick's own art is
    /// the Hero's fetch, so the picked entry is never warmed here, and the
    /// Upcoming mode paints no Hero, so it warms nothing.
    pub(super) fn warm_gui_shows_hero_neighbours(
        &mut self,
        lib_idx: usize,
        mode: GuiShowsMode,
        projection: &GuiShowsProjection,
    ) {
        let Some(picked) = projection.selected.as_deref() else {
            return;
        };
        let ids: Vec<&str> = match mode {
            GuiShowsMode::All => projection
                .series
                .iter()
                .map(|series| series.id.as_str())
                .collect(),
            GuiShowsMode::Latest | GuiShowsMode::Upcoming => {
                projection.rows.iter().map(|row| row.id.as_str()).collect()
            }
        };
        let Some(picked_index) = ids.iter().position(|id| *id == picked) else {
            return;
        };
        self.warm_gui_hero_neighbours(&ids, picked_index, |app, id| {
            app.gui_shows_hero_source(lib_idx, mode, id)
        });
    }

    /// The artwork source the Hero paints for `item_id`, from the same
    /// producer and poster arm the Hero's own fetch reads.
    fn gui_shows_hero_source(
        &self,
        lib_idx: usize,
        mode: GuiShowsMode,
        item_id: &str,
    ) -> Option<ArtworkSource> {
        let item = self.gui_shows_item(lib_idx, item_id)?;
        let content = match mode {
            GuiShowsMode::All => primary_poster(
                hero_content_series_with_episode(&item, None, false, None),
                true,
            ),
            GuiShowsMode::Latest => primary_poster(hero_content_emby(&item), false),
            GuiShowsMode::Upcoming => return None,
        };
        content.facts.artwork.source
    }
}
