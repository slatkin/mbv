use super::components::library_panel::LibraryKey;
use super::components::msg::PodcastEpisodeIntent;
use super::components::podcast_content::PodcastContent;
use super::components::LibraryKind;
use super::shell::Model;
use super::types_audiobookshelf_browse::AudiobookshelfBrowseKind;
use super::TabSelection;
use mbv_core::config::ServiceKind;

impl Model {
    fn abs_podcast_key(&self) -> Option<LibraryKey> {
        let TabSelection::AudiobookshelfLibrary(index) = self.app.tab else {
            return None;
        };
        (matches!(
            self.app.audiobookshelf_kind_at(index),
            Some(AudiobookshelfBrowseKind::Podcast)
        ))
        .then(|| {
            let library = self.app.audiobookshelf_libraries.get(index)?;
            Some(LibraryKey::Service {
                service: ServiceKind::Audiobookshelf,
                library_id: library.id.clone(),
                kind: LibraryKind::AudiobookshelfPodcast,
            })
        })?
    }

    fn update_abs_podcast_owner<R>(
        &mut self,
        f: impl FnOnce(&mut PodcastContent) -> R,
    ) -> Option<R> {
        let key = self.abs_podcast_key()?;
        self.update_library_owner(key, || Box::new(PodcastContent::new()), f)
    }

    pub(super) fn push_audiobookshelf_podcast_content(&mut self) {
        let Some(index) = self.app.tab.audiobookshelf_index() else {
            return;
        };
        if !matches!(
            self.app.audiobookshelf_kind_at(index),
            Some(AudiobookshelfBrowseKind::Podcast)
        ) {
            return;
        }
        let Some(snapshot) = self.app.audiobookshelf_browse.get(index).cloned() else {
            return;
        };
        let focused = matches!(self.app.effective_panel_focus(), super::PanelFocus::Library);
        let images_enabled = self.app.images_enabled();
        let library_id = snapshot.library.id.clone();
        let latest = self
            .app
            .audiobookshelf_shelf_cache
            .get(&library_id)
            .cloned()
            .unwrap_or_default();
        let key = LibraryKey::Service {
            service: ServiceKind::Audiobookshelf,
            library_id: library_id.clone(),
            kind: LibraryKind::AudiobookshelfPodcast,
        };
        let selected_latest = self
            .application
            .get_component(&super::components::ComponentId::Library)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<super::components::library_panel::LibraryPanel>()
            })
            .and_then(|panel| panel.owner(&key))
            .and_then(|owner| owner.as_any().downcast_ref::<PodcastContent>())
            .is_some_and(PodcastContent::latest_selected);
        let source =
            super::types_playback::DestinationLatestSource::Audiobookshelf(library_id.clone());
        if selected_latest && !self.acknowledged_home_latest_sources.contains(&source) {
            self.record_home_latest_acknowledgement(source.clone());
        }
        let has_new = latest.iter().any(|item| {
            super::home_latest::is_new_in_launch_window(item, self.app.home_latest_launch_window)
        });
        let acknowledged = self.acknowledged_home_latest_sources.contains(&source);
        self.update_abs_podcast_owner(|owner| {
            owner.set_content(&snapshot, images_enabled);
            owner.set_latest_items(&latest);
            owner.set_latest_marker(has_new, acknowledged);
            owner.set_focused(focused);
        });
    }

    pub(super) fn handle_audiobookshelf_podcast_episode_intent(
        &mut self,
        intent: PodcastEpisodeIntent,
    ) {
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        let index = self.app.tab.audiobookshelf_index();
        match intent {
            PodcastEpisodeIntent::FocusOrPlay(Some(target)) => {
                if let Some(index) = index {
                    self.app
                        .play_selected_audiobookshelf_episode_target(index, &target);
                }
            }
            // The flat episode browser always resolves a target once rows
            // exist; a selectionless activation is a no-op (the episode
            // selection / hero-overlay paths left with the show browser,
            // reorganize-podcast-pill-navigation 3.1).
            PodcastEpisodeIntent::FocusOrPlay(None) => {}
            PodcastEpisodeIntent::OpenOrPlay(Some(target)) => {
                if let Some(index) = index {
                    self.app
                        .play_selected_audiobookshelf_episode_target(index, &target);
                }
            }
            PodcastEpisodeIntent::OpenOrPlay(None) => {}
            PodcastEpisodeIntent::Enqueue(Some(target)) => {
                if let Some(index) = index {
                    self.app
                        .enqueue_selected_audiobookshelf_episode_target(index, &target);
                }
            }
            PodcastEpisodeIntent::Enqueue(None) => {}
        }
    }
}
