use super::Model;
use super::TabSelection;
use mbv_components::podcast_content::PodcastContent;
use mbv_queue::ServiceKind;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind;
use mbv_ui_model::library::LibraryKey;
use mbv_ui_model::library::LibraryKind;
use mbv_ui_msg::PodcastEpisodeIntent;

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
        let key: LibraryKey = self.abs_podcast_key()?;
        self.update_library_owner(&key, || Box::new(PodcastContent::new()), f)
    }

    /// Whether the active podcast library's owner has its `Latest` pill
    /// selected. The pill is component-local state, so the shell reads it
    /// from the mounted owner; a non-podcast active tab answers `false`.
    pub(in crate::app) fn abs_podcast_latest_selected(&self) -> bool {
        let Some(key) = self.abs_podcast_key() else {
            return false;
        };
        self.application
            .get_component(&mbv_ui_msg::ComponentId::Library)
            .and_then(|component| {
                component
                    .as_any()
                    .downcast_ref::<mbv_components::library_panel::LibraryPanel>()
            })
            .and_then(|panel| panel.owner(&key))
            .and_then(|owner| owner.as_any().downcast_ref::<PodcastContent>())
            .is_some_and(PodcastContent::latest_selected)
    }

    /// Spawn one podcast library's destination Latest shelf fetch (spec
    /// `destination-latest-modes`, "Destination Latest lists load lazily"): the fetch runs only while that library's Latest pill is
    /// selected, on refresh while it is selected, and on launch restoration
    /// on Latest — never in the startup catalog loop.
    pub(in crate::app) fn spawn_audiobookshelf_latest_shelves(&self, library_id: &str) {
        crate::app::dispatch::session::service_startup::start_audiobookshelf_shelves(
            self.app.config.lock().unwrap().clone(),
            self.app.audiobookshelf_runtime.generation(),
            library_id.to_string(),
            self.app.channels.lib_tx.clone(),
        );
    }

    pub(in crate::app) fn push_audiobookshelf_podcast_content(&mut self) {
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
        let images_enabled = self.app.images.images_enabled();
        let library_id = snapshot.library.id.clone();
        let latest = self
            .app
            .audiobookshelf_shelf_cache
            .get(&library_id)
            .cloned()
            .unwrap_or_default();
        let selected_latest = self.abs_podcast_latest_selected();
        // Latest is the default pill, so its shelf loads on first view.
        if selected_latest
            && self
                .app
                .audiobookshelf_shelf_requested
                .insert(library_id.clone())
        {
            self.spawn_audiobookshelf_latest_shelves(&library_id);
        }
        let source = crate::app::state::playback::DestinationLatestSource::Audiobookshelf(
            library_id.clone(),
        );
        if selected_latest && !self.acknowledged_home_latest_sources.contains(&source) {
            self.record_home_latest_acknowledgement(source.clone());
        }
        let latest_marker = self.destination_latest_marker(&source);
        self.update_abs_podcast_owner(|owner| {
            owner.set_content(&snapshot, images_enabled);
            owner.set_latest_items(&latest);
            owner.set_latest_marker(latest_marker);
            owner.set_focused(focused);
        });
    }

    pub(in crate::app) fn handle_audiobookshelf_podcast_episode_intent(
        &mut self,
        intent: PodcastEpisodeIntent,
    ) {
        self.app
            .set_panel_focus(mbv_ui_model::settings::PanelFocus::Library);
        let index = self.app.tab.audiobookshelf_index();
        match intent {
            PodcastEpisodeIntent::FocusOrPlay(Some(target))
            | PodcastEpisodeIntent::OpenOrPlay(Some(target)) => {
                if let Some(index) = index {
                    self.app
                        .play_selected_audiobookshelf_episode_target(index, &target);
                }
            }
            // The flat episode browser always resolves a target once rows
            // exist; a selectionless activation is a no-op (the episode
            // selection / hero-overlay paths left with the show browser,
            // reorganize-podcast-pill-navigation 3.1).
            PodcastEpisodeIntent::FocusOrPlay(None) | PodcastEpisodeIntent::OpenOrPlay(None) => {}
        }
    }
}
