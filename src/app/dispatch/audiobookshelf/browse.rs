use crate::app::dispatch::notify::ToastSeverity;
#[cfg(test)]
use crate::app::state::types::audiobookshelf_browse::AudiobookshelfEpisodeFilter;
use crate::app::App;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{AudiobookshelfQueueItem, QueueItem};

mod books;
pub(in crate::app) use books::audiobookshelf_book_queue_item;
pub(super) use books::AudiobookshelfBookQueueItem;

/// The number of per-show episode fetches the podcast fan-out keeps in
/// flight at once (design D5: bounded in-flight requests; a library with
/// many shows fills in progressively).
pub(in crate::app) const MAX_PODCAST_DETAILS_IN_FLIGHT: usize = 4;

impl App {
    /// Resolve the browse kind for Audiobookshelf library `index` from its
    /// `media_type`, once. This is the single resolution point the
    /// service-browse-dispatch spec requires: downstream renderers, input
    /// handlers, refresh, and position restore for this destination branch on
    /// the returned kind instead of re-reading `media_type` per action.
    ///
    /// `None` when the index is stale (library removed/replaced); the caller
    /// must stop the triggering operation, matching `normalize_stale_*`.
    pub(in crate::app) fn audiobookshelf_kind_at(
        &self,
        index: usize,
    ) -> Option<crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseKind> {
        self.audiobookshelf_libraries.get(index).map(|library| {
            crate::app::state::types::audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(
                &library.media_type,
            )
        })
    }

    /// Fetches one podcast show's downloaded episodes (design D2: the
    /// per-show expanded-item fetch). The in-flight mark is inserted only
    /// after the Service setup and key resolve: an early return on missing
    /// setup must not leak the mark and blocklist the show for the session
    /// (reorganize-podcast-pill-navigation 3.3 carried obligation).
    pub(in crate::app) fn start_audiobookshelf_detail(&mut self, library_item_id: String) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let config_snapshot = self.config.lock().unwrap().clone();
        let Some((setup, key)) =
            crate::app::dispatch::session::service_startup::audiobookshelf_setup_and_key(
                &config_snapshot,
            )
        else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        if state.detail_cache.contains_key(&library_item_id)
            || state.detail_loading_ids.contains_key(&library_item_id)
        {
            return;
        }
        state.next_detail_request += 1;
        let request = state.next_detail_request;
        state
            .detail_loading_ids
            .insert(library_item_id.clone(), request);
        let generation = self.audiobookshelf_runtime.generation();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let result = mbv_core::audiobookshelf::AudiobookshelfClient::new(&setup.server_url)
                .and_then(|client| {
                    client.podcast_detail_bounded(
                        &key,
                        &library_item_id,
                        mbv_core::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
                    )
                });
            let _ = tx.send(
                crate::app::state::types::events::LibEvent::AudiobookshelfDetailFetched {
                    generation,
                    request,
                    library_item_id,
                    result,
                },
            );
        });
    }

    /// Fetches the selected book's chapters/audio-files detail keyed by
    /// `library_item_id`.
    pub(in crate::app) fn start_audiobookshelf_book_detail(&mut self, library_item_id: String) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
            return;
        };
        if state.detail_cache.contains_key(&library_item_id)
            || state.detail_loading_ids.contains(&library_item_id)
        {
            state.detail_loading = state
                .selected_id
                .as_ref()
                .is_some_and(|id| state.detail_loading_ids.contains(id));
            return;
        }
        state.detail_loading_ids.insert(library_item_id.clone());
        state.detail_loading = true;
        let config_snapshot = self.config.lock().unwrap().clone();
        let Some((setup, key)) =
            crate::app::dispatch::session::service_startup::audiobookshelf_setup_and_key(
                &config_snapshot,
            )
        else {
            return;
        };
        let generation = self.audiobookshelf_runtime.generation();
        let tx = self.lib_tx.clone();
        std::thread::spawn(move || {
            let result = mbv_core::audiobookshelf::AudiobookshelfClient::new(&setup.server_url)
                .and_then(|client| {
                    client.book_detail_bounded(
                        &key,
                        &library_item_id,
                        mbv_core::audiobookshelf::AudiobookshelfClient::REQUEST_HARD_BOUND,
                    )
                });
            let _ = tx.send(
                crate::app::state::types::events::LibEvent::AudiobookshelfBookDetailFetched {
                    generation,
                    library_item_id,
                    result,
                },
            );
        });
    }

    /// The lazy episode fan-out (design D5, task 3.3): the committed pill
    /// decides the required shows — a show pill needs that show; a state
    /// pill needs every listed show. Shows already cached or in flight are
    /// skipped, so each show is fetched at most once per session and
    /// re-arming after an arrival is idempotent; the in-flight cap bounds
    /// the batch. Only the active tab's library fans out: an inactive tab
    /// has no viewed pill to load for.
    pub(in crate::app) fn start_audiobookshelf_podcast_fan_out(&mut self, index: usize) {
        if self.tab.audiobookshelf_index() != Some(index) {
            return;
        }
        let required: Vec<String> = {
            let Some(state) = self.audiobookshelf_browse.get(index) else {
                return;
            };
            match state.committed_show_pill.as_ref() {
                Some(id) => vec![id.clone()],
                None => state
                    .shows
                    .iter()
                    .map(|show| show.library_item_id.clone())
                    .collect(),
            }
        };
        for id in required {
            let in_flight = self.audiobookshelf_browse[index].detail_loading_ids.len();
            if in_flight >= MAX_PODCAST_DETAILS_IN_FLIGHT {
                break;
            }
            self.start_audiobookshelf_detail(id);
        }
    }

    /// A state-pill scope commit or a state-scoped list interaction (design
    /// D5): the required shows are every listed show; arm the bounded
    /// fan-out.
    pub(in crate::app) fn commit_audiobookshelf_podcast_state_scope(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            state.committed_show_pill = None;
        }
        self.start_audiobookshelf_podcast_fan_out(index);
    }

    pub(in crate::app) fn audiobookshelf_refresh(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let (library_id, generation) = {
            let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
                return;
            };
            state.shows.clear();
            state.total = 0;
            state.next_page = 0;
            state.error = None;
            state.clear_episodes();
            // `episode_filter` / episode-pane focus / `scroll` are
            // component-owned now (split-browse-state-interaction-fields task
            // 3.2); the content push after this reset drops the selected show,
            // which resets the component's own interaction state.
            state.loading_pages.clear();
            // The cleared list also drops the component's show pill (it
            // resets to `All` on the content push), so the fan-out scope
            // follows it back to the state pills.
            state.committed_show_pill = None;
            // Mark page 0 pending before re-issuing it so the catalog reloads
            // from the first page (the renderer shows a Loading placeholder
            // until the response lands).
            state.loading_pages.insert(0);
            (
                state.library.id.clone(),
                self.audiobookshelf_runtime.generation(),
            )
        };
        // Restart the catalog request from page 0 after clearing state.
        crate::app::dispatch::session::service_startup::start_audiobookshelf_shows(
            self.config.lock().unwrap().clone(),
            generation,
            library_id.clone(),
            0,
            self.lib_tx.clone(),
        );
        crate::app::dispatch::session::service_startup::start_audiobookshelf_shelves(
            self.config.lock().unwrap().clone(),
            generation,
            library_id,
            self.lib_tx.clone(),
        );
    }

    pub(in crate::app) fn select_audiobookshelf_show(&mut self, cursor: usize) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let selected_id = {
            let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
                return;
            };
            if state.shows.is_empty() {
                return;
            }
            state.select(cursor.min(state.shows.len() - 1));
            state.selected_id.clone()
        };
        if let Some(id) = selected_id {
            self.start_audiobookshelf_detail(id);
        }
    }

    pub(in crate::app) fn select_audiobookshelf_show_target(&mut self, target: &str) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(cursor) = self.audiobookshelf_browse.get(index).and_then(|state| {
            state
                .shows
                .iter()
                .position(|show| show.library_item_id == target)
        }) else {
            return;
        };
        // The resolved show pill scopes the fan-out (design D5): only that
        // show's episodes are required while it is active.
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            state.committed_show_pill = Some(target.to_string());
        }
        self.select_audiobookshelf_show(cursor);
    }

    /// Resolve the downloaded episode at `episode_index` at the Audiobookshelf
    /// playback boundary. Queue submission remains the responsibility of the
    /// later action stage; browse state never sees credentials or playback
    /// state. Read-only resolver seam for the pre-U5 App-level tests; the
    /// shell play/enqueue path threads the component-resolved episode index
    /// and filter directly (task 5.3d.11 U5). The episode filter is
    /// component-owned (split-browse-state-interaction-fields task 3.2), so
    /// this seam resolves against the unfiltered (`All`) view.
    #[cfg(test)]
    pub(in crate::app) fn activate_audiobookshelf_episode(
        &mut self,
        audiobookshelf_library_index: usize,
        episode_index: usize,
    ) -> Option<QueueItem> {
        self.selected_audiobookshelf_queue_item(
            audiobookshelf_library_index,
            episode_index,
            AudiobookshelfEpisodeFilter::All,
        )
    }

    /// Resolve the episode at `episode_index` for enqueue without mutating any
    /// queue or opening a playback lifecycle (see `activate_audiobookshelf_episode`).
    #[cfg(test)]
    pub(in crate::app) fn enqueue_audiobookshelf_episode(
        &mut self,
        audiobookshelf_library_index: usize,
        episode_index: usize,
    ) -> Option<QueueItem> {
        self.selected_audiobookshelf_queue_item(
            audiobookshelf_library_index,
            episode_index,
            AudiobookshelfEpisodeFilter::All,
        )
    }

    /// Ordinary play for the downloaded episode at `episode_index`. The shell
    /// resolves the target from the mounted component's selection (task
    /// 5.3d.11 U5); the App only supplies the provider-native snapshot, while
    /// canonical queue ownership and the eligible Player boundary remain here
    /// with the other ordinary actions.
    pub(in crate::app) fn play_selected_audiobookshelf_episode_target(
        &mut self,
        index: usize,
        target: &crate::app::components::msg::PodcastEpisodeTarget,
    ) {
        let Some(item) = self.selected_audiobookshelf_queue_item_target(index, target) else {
            return;
        };
        if !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.submit_queue_item(item, true);
    }

    pub(in crate::app) fn enqueue_selected_audiobookshelf_episode_target(
        &mut self,
        index: usize,
        target: &crate::app::components::msg::PodcastEpisodeTarget,
    ) {
        let Some(item) = self.selected_audiobookshelf_queue_item_target(index, target) else {
            return;
        };
        let scope = self.viewed_queue_scope();
        let bound = scope == self.playing_queue_scope()
            && (self.player.is_remote() || self.player.status.lock().unwrap().active);
        if bound && !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.submit_queue_item(item, false);
    }

    #[cfg(test)]
    pub(in crate::app) fn play_selected_audiobookshelf_episode(
        &mut self,
        index: usize,
        episode_index: usize,
        filter: AudiobookshelfEpisodeFilter,
    ) {
        let Some(item) = self.selected_audiobookshelf_queue_item(index, episode_index, filter)
        else {
            return;
        };
        if !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.submit_queue_item(item, true);
    }

    /// Ordinary enqueue for the downloaded episode at `episode_index`. A cold
    /// local queue is the Composed stage and is intentionally allowed without
    /// owner admission; an active or remote playback target is Bound and must
    /// be eligible.
    #[cfg(test)]
    pub(in crate::app) fn enqueue_selected_audiobookshelf_episode(
        &mut self,
        index: usize,
        episode_index: usize,
        filter: AudiobookshelfEpisodeFilter,
    ) {
        let Some(item) = self.selected_audiobookshelf_queue_item(index, episode_index, filter)
        else {
            return;
        };
        let scope = self.viewed_queue_scope();
        let bound = scope == self.playing_queue_scope()
            && (self.player.is_remote() || self.player.status.lock().unwrap().active);
        if bound && !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.submit_queue_item(item, false);
    }

    #[cfg(test)]
    fn selected_audiobookshelf_queue_item(
        &self,
        audiobookshelf_library_index: usize,
        episode_index: usize,
        filter: AudiobookshelfEpisodeFilter,
    ) -> Option<QueueItem> {
        let state = self
            .audiobookshelf_browse
            .get(audiobookshelf_library_index)?;
        let episode = state
            .visible_episodes(filter)
            .get(episode_index)?
            .to_owned();
        self.selected_audiobookshelf_queue_item_target(
            audiobookshelf_library_index,
            &crate::app::components::msg::PodcastEpisodeTarget::new(
                episode.library_item_id.clone(),
                episode.episode_id.clone(),
            ),
        )
    }

    pub(in crate::app) fn selected_audiobookshelf_queue_item_target(
        &self,
        audiobookshelf_library_index: usize,
        target: &crate::app::components::msg::PodcastEpisodeTarget,
    ) -> Option<QueueItem> {
        if target.library_item_id().trim().is_empty() || target.episode_id().trim().is_empty() {
            return None;
        }
        let library_id = &self
            .audiobookshelf_libraries
            .get(audiobookshelf_library_index)?
            .id;
        let state = self
            .audiobookshelf_browse
            .get(audiobookshelf_library_index)?;
        let progress = state.progress.get(&(
            target.library_item_id().to_owned(),
            target.episode_id().to_owned(),
        ));
        if let Some(mut item) = self
            .audiobookshelf_shelf_cache
            .get(library_id)
            .and_then(|items| {
                items.iter().find(|item| {
                    matches!(
                        item,
                        QueueItem::Audiobookshelf(
                            mbv_core::playback_queue::AudiobookshelfItem::Episode(episode)
                        ) if episode.library_item_id == target.library_item_id()
                            && episode.episode_id == target.episode_id()
                    )
                })
            })
            .cloned()
        {
            if let (
                QueueItem::Audiobookshelf(mbv_core::playback_queue::AudiobookshelfItem::Episode(
                    episode,
                )),
                Some(progress),
            ) = (&mut item, progress)
            {
                episode.position_ticks = seconds_to_ticks(progress.current_time_seconds);
                episode.played = progress.is_finished;
                episode.is_finished = progress.is_finished;
            }
            return Some(item);
        }
        // The episode is resolved by its own `(library_item_id, episode_id)`
        // identity from the per-show cache, never from the tab's current
        // selection: selection is pill/episode identity now, and the queue
        // item's parent-show metadata follows the episode (design D6).
        let episode = state.episode_by_identity(target.library_item_id(), target.episode_id())?;
        if episode.library_item_id.trim().is_empty() || episode.episode_id.trim().is_empty() {
            return None;
        }
        let show = state
            .shows
            .iter()
            .find(|show| show.library_item_id == episode.library_item_id);
        let position_ticks = progress.map_or(0, |progress| {
            seconds_to_ticks(progress.current_time_seconds)
        });
        let is_finished = progress.is_some_and(|progress| progress.is_finished);

        Some(QueueItem::Audiobookshelf(
            mbv_core::playback_queue::AudiobookshelfItem::Episode(AudiobookshelfQueueItem {
                library_item_id: episode.library_item_id.clone(),
                episode_id: episode.episode_id.clone(),
                title: episode.title.clone(),
                show_title: show.map(|show| show.title.clone()),
                author: show.and_then(|show| show.author.clone()),
                description: None,
                duration_ticks: episode.duration_seconds.and_then(seconds_to_ticks_u64),
                position_ticks,
                played: is_finished,
                pub_date_secs: episode.published_at,
                is_finished,
                cover_path: show.and_then(|show| show.cover_path.clone()),
            }),
        ))
    }
}

pub(in crate::app) fn seconds_to_ticks(seconds: f64) -> i64 {
    seconds_to_ticks_u64(seconds)
        .and_then(|ticks| i64::try_from(ticks).ok())
        .unwrap_or(0)
}
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "seconds↔ticks conversion through f64; no lossless integer-path conversion exists (approved, issue #804)"
)]
pub(super) fn seconds_to_ticks_u64(seconds: f64) -> Option<u64> {
    (seconds.is_finite() && seconds >= 0.0)
        .then(|| (seconds * TICKS_PER_SECOND as f64).round() as u64)
}

#[cfg(test)]
mod book_seek_tests;
