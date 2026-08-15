use super::notify_actions::ToastSeverity;
use super::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter;
use super::App;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{AudiobookshelfQueueItem, QueueItem};

impl App {
    pub(super) fn audiobookshelf_refresh(&mut self) {
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
            state.detail_cache.clear();
            state.episodes = None;
            state.episode_selection = None;
            state.scroll = 0;
            state.loading_pages.clear();
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
        super::service_startup::start_audiobookshelf_shows(
            self.config.lock().unwrap().clone(),
            generation,
            library_id,
            0,
            self.lib_tx.clone(),
        );
    }

    pub(super) fn select_audiobookshelf_show(&mut self, cursor: usize) {
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
        self.save_audiobookshelf_position(index);
        if let Some(id) = selected_id {
            self.start_audiobookshelf_detail(id);
        }
    }

    pub(super) fn move_audiobookshelf_show_cursor(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return;
        };
        if state.shows.is_empty() || state.episode_selection.is_some() {
            return;
        }
        let cursor = super::ui_util::move_cursor(state.cursor(), delta, state.shows.len());
        self.select_audiobookshelf_show(cursor);
    }

    pub(super) fn move_audiobookshelf_show_rows(&mut self, rows: i64) {
        let columns = crate::app::library_column_width::library_column_count(
            self.layout.main.left_area.width,
        );
        self.move_audiobookshelf_show_cursor(rows * columns as i64);
    }

    pub(super) fn jump_audiobookshelf_show_cursor(&mut self, end: bool) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return;
        };
        if state.shows.is_empty() || state.episode_selection.is_some() {
            return;
        }
        self.select_audiobookshelf_show(if end { state.shows.len() - 1 } else { 0 });
    }

    pub(super) fn enter_audiobookshelf_episode_selection(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            state.enter_episode_selection();
        }
    }

    pub(super) fn leave_audiobookshelf_episode_selection(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            state.episode_selection = None;
        }
    }

    pub(super) fn move_audiobookshelf_episode_cursor(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        let Some(cursor) = state.episode_selection else {
            return;
        };
        let count = state.visible_episodes().len();
        if count > 0 {
            state.episode_selection = Some(super::ui_util::move_cursor(cursor, delta, count));
        }
    }

    pub(super) fn cycle_audiobookshelf_filter(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        if state.episode_selection.is_none() {
            return;
        }
        let current = AudiobookshelfEpisodeFilter::ALL
            .iter()
            .position(|filter| *filter == state.episode_filter)
            .unwrap_or(0);
        let next = (current as i64 + delta).rem_euclid(3) as usize;
        state.set_episode_filter(AudiobookshelfEpisodeFilter::ALL[next]);
    }

    pub(super) fn select_audiobookshelf_filter(&mut self, target: usize) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(filter) = AudiobookshelfEpisodeFilter::ALL.get(target).copied() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            if state.episode_selection.is_some() {
                state.set_episode_filter(filter);
            }
        }
    }

    /// Resolve the selected downloaded episode at the Audiobookshelf playback
    /// boundary. Queue submission remains the responsibility of the later
    /// action stage; browse state never sees credentials or playback state.
    pub(super) fn activate_audiobookshelf_episode(
        &mut self,
        audiobookshelf_library_index: usize,
    ) -> Option<QueueItem> {
        self.selected_audiobookshelf_queue_item(audiobookshelf_library_index)
    }

    /// Resolve the selected downloaded episode for enqueue without mutating
    /// any queue or opening a playback lifecycle.
    pub(super) fn enqueue_audiobookshelf_episode(
        &mut self,
        audiobookshelf_library_index: usize,
    ) -> Option<QueueItem> {
        self.selected_audiobookshelf_queue_item(audiobookshelf_library_index)
    }

    /// Ordinary play for a downloaded episode. Browse supplies only the
    /// provider-native snapshot; canonical queue ownership and the eligible
    /// Player boundary remain here with the other ordinary actions.
    pub(super) fn play_selected_audiobookshelf_episode(&mut self, index: usize) {
        let Some(item) = self.activate_audiobookshelf_episode(index) else {
            return;
        };
        if !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }

        let scope = self.playback_target_queue_scope();
        let previous_queue = self.queue_for_scope(scope).clone();
        let existing_index = self
            .queue_for_scope(scope)
            .slots()
            .iter()
            .position(|slot| slot.item.content_id() == item.content_id());
        let selected_index = existing_index.unwrap_or_else(|| {
            self.queue_for_scope_mut(scope).queue.append(item.clone());
            self.queue_for_scope(scope).total_queue_len() - 1
        });
        let selected_slot = self
            .queue_for_scope(scope)
            .slot_id_at(selected_index)
            .expect("selected Audiobookshelf queue slot disappeared");
        {
            let queue = self.queue_for_scope_mut(scope);
            queue.queue_cursor = selected_index;
            let _ = queue.queue.set_active_slot(selected_slot);
        }

        let all_items = self.queue_for_scope(scope).all_queue_items();
        let audio_only = all_items.iter().all(QueueItem::is_audio);
        let submitted =
            self.player
                .submit_queue(all_items, selected_index, None, audio_only, self.ui_volume);
        if !submitted {
            *self.queue_for_scope_mut(scope) = previous_queue;
            self.flash(
                "Playback owner rejected this Audiobookshelf item".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.set_queue_scope(scope);
        if !matches!(self.panel_focus, super::PanelFocus::Library) {
            self.set_panel_focus(super::PanelFocus::Queue);
        }
    }

    /// Ordinary enqueue for a downloaded episode. A cold local queue is the
    /// Composed stage and is intentionally allowed without owner admission;
    /// an active or remote playback target is Bound and must be eligible.
    pub(super) fn enqueue_selected_audiobookshelf_episode(&mut self, index: usize) {
        let Some(item) = self.enqueue_audiobookshelf_episode(index) else {
            return;
        };
        let scope = self.visible_queue_scope();
        let bound = scope == self.playback_target_queue_scope()
            && (self.player.is_remote() || self.player.status.lock().unwrap().active);
        if bound && !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }

        let previous_dirty = self.queue_dirty;
        let previous_queue = self.queue_for_scope(scope).clone();
        self.queue_for_scope_mut(scope).queue.append(item.clone());
        if self.local_queue_metadata_applies(scope) {
            self.queue_dirty = true;
        }
        if self.sync_playback_queue_items_after_append(scope, vec![item]) {
            self.persist_local_queue_state_if_needed(scope);
            self.retire_remote_tracking_after_queue_mutation();
        } else {
            self.queue_dirty = previous_dirty;
            *self.queue_for_scope_mut(scope) = previous_queue;
        }
    }

    fn selected_audiobookshelf_queue_item(
        &self,
        audiobookshelf_library_index: usize,
    ) -> Option<QueueItem> {
        let state = self
            .audiobookshelf_browse
            .get(audiobookshelf_library_index)?;
        let episode_index = state.episode_selection?;
        let episode = state.visible_episodes().get(episode_index)?.to_owned();
        if episode.library_item_id.trim().is_empty() || episode.episode_id.trim().is_empty() {
            return None;
        }
        let show = state.selected_show();
        let progress = state
            .progress
            .get(&(episode.library_item_id.clone(), episode.episode_id.clone()));
        let position_ticks = progress
            .map(|progress| seconds_to_ticks(progress.current_time_seconds))
            .unwrap_or(0);
        let is_finished = progress.is_some_and(|progress| progress.is_finished);

        Some(QueueItem::Audiobookshelf(AudiobookshelfQueueItem {
            library_item_id: episode.library_item_id.clone(),
            episode_id: episode.episode_id.clone(),
            title: episode.title.clone(),
            show_title: show.map(|show| show.title.clone()),
            author: show.and_then(|show| show.author.clone()),
            duration_ticks: episode.duration_seconds.and_then(seconds_to_ticks_u64),
            position_ticks,
            played: is_finished,
            pub_date_secs: episode
                .published_at
                .as_deref()
                .and_then(super::feed_parse_date::parse_pub_date_secs),
            is_finished,
            cover_path: show.and_then(|show| show.cover_path.clone()),
        }))
    }
}

pub(super) fn seconds_to_ticks(seconds: f64) -> i64 {
    seconds_to_ticks_u64(seconds)
        .and_then(|ticks| i64::try_from(ticks).ok())
        .unwrap_or(0)
}

fn seconds_to_ticks_u64(seconds: f64) -> Option<u64> {
    (seconds.is_finite() && seconds >= 0.0)
        .then(|| (seconds * TICKS_PER_SECOND as f64).round() as u64)
}
