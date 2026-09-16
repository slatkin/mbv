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

    pub(in crate::app) fn abs_podcast_owner(&self) -> Option<&PodcastContent> {
        let key = self.abs_podcast_key()?;
        self.library_owner(&key)
    }
    pub(in crate::app) fn abs_podcast_owner_mut(&mut self) -> Option<&mut PodcastContent> {
        let key = self.abs_podcast_key()?;
        self.library_owner_mut(&key)
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
        self.update_abs_podcast_owner(|owner| {
            owner.set_content(&snapshot, images_enabled);
            owner.set_focused(focused);
        });
    }

    pub(super) fn handle_audiobookshelf_podcast_episode_intent(
        &mut self,
        intent: PodcastEpisodeIntent,
    ) {
        if self.abs_podcast_owner().is_none() {
            return;
        }
        self.app.set_panel_focus(crate::app::PanelFocus::Library);
        let index = self.app.tab.audiobookshelf_index();
        let wide = self.app.is_right_panel_wide();
        match intent {
            PodcastEpisodeIntent::FocusOrPlay(Some(target)) => {
                if let Some(index) = index {
                    self.app
                        .play_selected_audiobookshelf_episode_target(index, &target);
                }
            }
            PodcastEpisodeIntent::FocusOrPlay(None) => {
                self.abs_podcast_owner_mut().unwrap().enter_episode_focus();
            }
            PodcastEpisodeIntent::OpenOrPlay(Some(target)) => {
                if let Some(index) = index {
                    self.app
                        .play_selected_audiobookshelf_episode_target(index, &target);
                }
            }
            PodcastEpisodeIntent::OpenOrPlay(None) => {
                if wide {
                    self.abs_podcast_owner_mut().unwrap().enter_episode_focus();
                } else {
                    self.open_library_hero_overlay();
                }
            }
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
