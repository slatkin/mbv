//! Audiobookshelf Podcasts' embedded Library panel content owner (task 11.1).
//!
//! This owner is deliberately plain: the legacy podcast component still paints
//! the surface in this slice, while the shell-projected snapshot is also ready
//! for the Library panel migration.

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
};
use super::library_panel::hero::hero_content_abs_show;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation,
};
use super::msg::Msg;
use crate::app::types_audiobookshelf_browse::{
    AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter,
};
use crate::app::ui_util::clean_overview;

/// Plain owner for one Audiobookshelf podcast library. Content is projected by
/// the shell; filter, focus, and list selection remain local interaction state.
pub(in crate::app) struct PodcastContent {
    pub(in crate::app) state: AudiobookshelfBrowseState,
    episode_filter: AudiobookshelfEpisodeFilter,
    episode_focused: bool,
    initialized: bool,
    focused: bool,
    carrier: MediaListCarrier<String>,
    episode_list: MediaListCarrier<String>,
    hero_image: HeroImageState,
}

impl PodcastContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            state: AudiobookshelfBrowseState::new(
                mbv_core::audiobookshelf::AudiobookshelfLibrary {
                    id: String::new(),
                    name: String::new(),
                    media_type: "podcast".into(),
                },
            ),
            episode_filter: AudiobookshelfEpisodeFilter::All,
            episode_focused: false,
            initialized: false,
            focused: false,
            carrier: MediaListCarrier::new(Presentation::Inline),
            episode_list: MediaListCarrier::new(Presentation::Wide),
            hero_image: HeroImageState::None,
        }
    }

    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBrowseState,
        _images_enabled: bool,
    ) {
        let survived = self.initialized
            && self.state.selected_id.as_ref().is_some_and(|prior| {
                snapshot
                    .shows
                    .iter()
                    .any(|show| &show.library_item_id == prior)
            });
        self.state = snapshot.clone();
        let rows = self
            .state
            .shows
            .iter()
            .map(|show| MediaListRow::Item {
                target: show.library_item_id.clone(),
                primary: show.title.clone(),
                trailing: None,
                duration: None,
                kind: MediaKind::Collection,
                semantic_state: MediaSemanticState::Ordinary,
            })
            .collect::<Vec<_>>();
        if self.carrier.rows() != rows.as_slice() {
            self.carrier.set_content(rows);
        }
        if !self.initialized {
            if let Some(id) = self.state.selected_id.clone() {
                self.carrier.select_target(&id);
            }
        } else if !survived {
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
            self.episode_focused = false;
            self.carrier.select_first();
            self.state.select(0);
            self.episode_list.select_first();
        }
        self.initialized = true;
        self.project_episode_rows();
    }

    fn project_episode_rows(&mut self) {
        let rows = self
            .state
            .visible_episodes(self.episode_filter)
            .into_iter()
            .map(|episode| MediaListRow::Item {
                target: episode.episode_id.clone(),
                primary: episode.title.clone(),
                trailing: None,
                duration: episode.duration_seconds.and_then(|seconds| {
                    crate::app::ui_util::list_duration_secs(seconds.round() as i64)
                }),
                kind: MediaKind::Media,
                semantic_state: self
                    .state
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .map(|p| {
                        if p.is_finished {
                            MediaSemanticState::Played
                        } else {
                            MediaSemanticState::Ordinary
                        }
                    })
                    .unwrap_or(MediaSemanticState::Ordinary),
            })
            .collect::<Vec<_>>();
        if self.episode_list.rows() != rows.as_slice() {
            self.episode_list.set_content(rows);
        }
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        let show = self.state.selected_show()?;
        let mut data = hero_content_abs_show(show);
        data.facts.artwork.image = self.hero_image.clone();
        Some(data)
    }

    pub(in crate::app) fn set_hero_image(&mut self, image: HeroImageState) {
        self.hero_image = image;
    }

    pub(in crate::app) fn content(&mut self) -> LibraryPanelContent<'_> {
        let hero = self.state.selected_show().map(|show| {
            let mut data = hero_content_abs_show(show);
            data.facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts: data.facts,
                overview: data
                    .overview
                    .map(|s| clean_overview(&s))
                    .filter(|s| !s.is_empty()),
                workspace: Some(Workspace {
                    selector: Some(SelectorRow {
                        pills: AudiobookshelfEpisodeFilter::ALL
                            .iter()
                            .map(|filter| filter.label().to_string())
                            .collect(),
                        active: Some(
                            AudiobookshelfEpisodeFilter::ALL
                                .iter()
                                .position(|filter| *filter == self.episode_filter)
                                .unwrap_or(0),
                        ),
                    }),
                    list: &mut self.episode_list,
                    focused: self.focused && self.episode_focused,
                }),
            }
        });
        let buckets =
            crate::app::types_audiobookshelf_browse::build_show_title_buckets(&self.state.shows);
        let active_bucket = self
            .state
            .selected_id
            .as_ref()
            .and_then(|id| {
                self.state
                    .shows
                    .iter()
                    .position(|show| &show.library_item_id == id)
            })
            .and_then(|cursor| {
                buckets
                    .iter()
                    .position(|bucket| (bucket.start..bucket.end).contains(&cursor))
            })
            .unwrap_or(0);
        let selector = (!buckets.is_empty()).then(|| SelectorRow {
            pills: buckets
                .iter()
                .map(|bucket| bucket.label.to_string())
                .collect(),
            active: Some(active_bucket),
        });
        let list = if self.state.shows.is_empty() {
            ListSlot::Empty {
                loading: !self.state.loading_pages.is_empty(),
                text: self
                    .state
                    .error
                    .clone()
                    .unwrap_or_else(|| "No podcasts".into()),
            }
        } else {
            ListSlot::Media(&mut self.carrier)
        };
        LibraryPanelContent {
            selector,
            controls: None,
            list,
            hero,
        }
    }
}

impl Default for PodcastContent {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryContentOwner for PodcastContent {
    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.content()
    }
    fn on_slot_event(&mut self, _event: LibrarySlotEvent) -> Option<Msg> {
        None
    }
    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }
    fn set_hero_image(&mut self, image: HeroImageState) {
        self.set_hero_image(image);
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mbv_core::audiobookshelf::{AudiobookshelfLibrary, AudiobookshelfShow};

    fn state() -> AudiobookshelfBrowseState {
        let mut state = AudiobookshelfBrowseState::new(AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        });
        state.append_page(
            0,
            1,
            1,
            vec![AudiobookshelfShow {
                library_item_id: "show".into(),
                title: "Show".into(),
                author: Some("Author".into()),
                description: Some("Overview".into()),
                cover_path: Some("cover".into()),
            }],
        );
        state.select(0);
        state
    }

    #[test]
    fn content_projects_podcast_panel_slots() {
        let mut owner = PodcastContent::new();
        owner.set_content(&state(), false);
        let content = owner.content();
        assert_eq!(
            content.selector.as_ref().map(|row| row.pills.len()),
            Some(1)
        );
        let hero = content.hero.expect("podcast hero");
        assert_eq!(
            hero.facts.artwork.shape,
            super::super::library_panel::ArtworkShape::Square
        );
        assert_eq!(hero.overview.as_deref(), Some("Overview"));
        let workspace = hero.workspace.expect("episode workspace");
        assert_eq!(
            workspace.selector.as_ref().map(|row| row.pills.clone()),
            Some(vec!["All".into(), "Played".into(), "Unplayed".into()])
        );
    }
}
