use super::notify_actions::ToastSeverity;
use super::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter;
use super::types_audiobookshelf_browse::BookRow;
use super::App;
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::playback_queue::{AudiobookshelfBookQueueItem, AudiobookshelfQueueItem, QueueItem};

impl App {
    /// Resolve the browse kind for Audiobookshelf library `index` from its
    /// `media_type`, once. This is the single resolution point the
    /// service-browse-dispatch spec requires: downstream renderers, input
    /// handlers, refresh, and position restore for this destination branch on
    /// the returned kind instead of re-reading `media_type` per action.
    ///
    /// `None` when the index is stale (library removed/replaced); the caller
    /// must stop the triggering operation, matching `normalize_stale_*`.
    pub(super) fn audiobookshelf_kind_at(
        &self,
        index: usize,
    ) -> Option<super::types_audiobookshelf_browse::AudiobookshelfBrowseKind> {
        self.audiobookshelf_libraries.get(index).map(|library| {
            super::types_audiobookshelf_browse::AudiobookshelfBrowseKind::from_media_type(
                &library.media_type,
            )
        })
    }

    /// Fetches the selected podcast show's downloaded episodes.
    pub(super) fn start_audiobookshelf_detail(&mut self, library_item_id: String) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        if let Some(cached) = state.detail_cache.get(&library_item_id).cloned() {
            state.episodes = Some(cached);
            state.detail_loading = false;
            return;
        }
        if state.episodes.is_some() || state.detail_loading {
            return;
        }
        state.detail_loading = true;
        let config_snapshot = self.config.lock().unwrap().clone();
        let Some((setup, key)) =
            super::service_startup::audiobookshelf_setup_and_key(&config_snapshot)
        else {
            return;
        };
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
            let _ = tx.send(super::types_events::LibEvent::AudiobookshelfDetailFetched {
                generation,
                library_item_id,
                result,
            });
        });
    }

    /// Fetches the selected book's chapters/audio-files detail keyed by
    /// `library_item_id`.
    pub(super) fn start_audiobookshelf_book_detail(&mut self, library_item_id: String) {
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
            super::service_startup::audiobookshelf_setup_and_key(&config_snapshot)
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
                super::types_events::LibEvent::AudiobookshelfBookDetailFetched {
                    generation,
                    library_item_id,
                    result,
                },
            );
        });
    }

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

    pub(super) fn podcast_filter_target_active(&self, index: usize) -> bool {
        self.audiobookshelf_browse
            .get(index)
            .is_some_and(|state| state.episode_selection.is_some())
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
        self.submit_queue_item(item, true);
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
        self.submit_queue_item(item, false);
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
            description: None,
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

    // ---- Book browsing actions -----------------------------------------

    pub(super) fn audiobookshelf_book_refresh(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let (library_id, generation) = {
            let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
                return;
            };
            state.books.clear();
            state.total = 0;
            state.next_page = 0;
            state.error = None;
            state.detail_cache.clear();
            state.detail_loading_ids.clear();
            state.chapter_selection = None;
            state.scroll = 0;
            state.loading_pages.clear();
            state.loading_pages.insert(0);
            (
                state.library.id.clone(),
                self.audiobookshelf_runtime.generation(),
            )
        };
        super::service_startup::start_audiobookshelf_books(
            self.config.lock().unwrap().clone(),
            generation,
            library_id,
            0,
            self.lib_tx.clone(),
        );
    }

    pub(super) fn select_audiobookshelf_book(&mut self, cursor: usize) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let selected_id = {
            let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
                return;
            };
            if state.books.is_empty() {
                return;
            }
            state.select(cursor.min(state.books.len() - 1));
            state.selected_id.clone()
        };
        self.save_audiobookshelf_book_position(index);
        if let Some(id) = selected_id {
            self.start_audiobookshelf_book_detail(id);
        }
    }

    /// Moves the right-pane browser cursor within the selected surname
    /// bucket only -- books outside it are not reachable by scrolling
    /// (book-browsing spec) until a different bucket is selected.
    pub(super) fn move_audiobookshelf_book_cursor(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let Some(bucket) = state.buckets.get(state.selected_bucket) else {
            return;
        };
        if bucket.end <= bucket.start {
            return;
        }
        let (start, end) = (bucket.start as i64, bucket.end as i64 - 1);
        let cursor = (state.cursor() as i64).clamp(start, end);
        let new_cursor = (cursor + delta).clamp(start, end) as usize;
        self.select_audiobookshelf_book(new_cursor);
    }

    pub(super) fn jump_audiobookshelf_book_cursor(&mut self, end: bool) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let Some(bucket) = state.buckets.get(state.selected_bucket) else {
            return;
        };
        if bucket.end <= bucket.start {
            return;
        }
        self.select_audiobookshelf_book(if end { bucket.end - 1 } else { bucket.start });
    }

    /// Left/right pane-focus toggle (book-browsing spec: "Left/right arrow
    /// toggles pane focus without hiding either pane"), replacing the old
    /// Enter-to-enter/Esc-to-leave modal transition. `chapter_selection`
    /// doubles as the focus flag -- `Some` focuses the hero's chapter list,
    /// `None` focuses the right-pane browser -- analogous to Music's
    /// `album_track_focus`.
    pub(super) fn focus_audiobookshelf_book_chapters(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
            if state.selected_id.is_some() && state.chapter_selection.is_none() {
                state.chapter_selection = Some(0);
            }
        }
    }

    pub(super) fn focus_audiobookshelf_book_browser(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
            state.chapter_selection = None;
        }
    }

    /// Cycles the selected alphabetical-bucket pill by `delta`, wrapping
    /// around -- the established pattern from `switch_music_group`/
    /// `cycle_letter_pill`.
    pub(super) fn cycle_audiobookshelf_book_bucket(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let n = state.buckets.len();
        if n == 0 {
            return;
        }
        let next = (state.selected_bucket as i64 + delta).rem_euclid(n as i64) as usize;
        self.select_audiobookshelf_book_bucket(next);
    }

    /// Selects bucket `bucket_pos` (a position in `state.buckets`, matching
    /// the pill's click target -- the established pattern from
    /// `select_music_group`), narrowing the right-pane list to it and
    /// re-anchoring the cursor into the new bucket when it falls outside.
    pub(super) fn select_audiobookshelf_book_bucket(&mut self, bucket_pos: usize) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(bucket) = self
            .audiobookshelf_book_browse
            .get(index)
            .and_then(|state| state.buckets.get(bucket_pos).copied())
        else {
            return;
        };
        let target = {
            let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
                return;
            };
            state.selected_bucket = bucket_pos;
            let cursor = state.cursor();
            if cursor >= bucket.start && cursor < bucket.end {
                cursor
            } else {
                bucket.start
            }
        };
        if bucket.end > bucket.start {
            self.select_audiobookshelf_book(target);
        } else {
            self.save_audiobookshelf_book_position(index);
        }
    }

    /// Move the chapter/audio-file selection inside the selected book.
    pub(super) fn move_audiobookshelf_book_row(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
            return;
        };
        let Some(cursor) = state.chapter_selection else {
            return;
        };
        let Some(id) = state.selected_id.as_deref() else {
            return;
        };
        let count = state.visible_rows(id).len();
        if count > 0 {
            state.chapter_selection =
                Some((cursor as i64 + delta).clamp(0, count as i64 - 1) as usize);
        }
    }

    /// Chapter-row activation: one absolute seek to `chapters[].start` on the
    /// active book's merged timeline, without stopping/reopening the queue
    /// slot or session (book-playback spec).
    pub(super) fn activate_audiobookshelf_book_row(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let (target_seconds, book_id) = {
            let state = match self.audiobookshelf_book_browse.get(index) {
                Some(state) => state,
                None => return,
            };
            let Some(id) = state.selected_id.as_deref() else {
                return;
            };
            let Some(cursor) = state.chapter_selection else {
                return;
            };
            let target = match state.visible_rows(id).get(cursor) {
                Some(BookRow::Chapter { start, .. }) => *start,
                // audio-file fallback rows have no chapter offsets; leave the
                // active position untouched rather than seeking somewhere wrong.
                Some(BookRow::AudioFile { .. }) => return,
                None => return,
            };
            (target, id.to_string())
        };
        // Only seek when the active queue slot is this book.
        let active_book = self
            .playback_queue()
            .queue
            .active_slot()
            .and_then(|slot| slot.item.as_audiobookshelf_book())
            .map(|book| book.library_item_id == book_id)
            .unwrap_or(false);
        if active_book {
            self.player
                .send_command(mbv_core::player::PlayerCommand::SeekAbsolute(
                    target_seconds,
                ));
        }
    }

    /// Resolve the selected book as a `QueueItem::AudiobookshelfBook` without
    /// mutating the queue or opening a playback lifecycle. Duration is the
    /// sum of the book's audio-file durations (chapters are offsets, not
    /// durations).
    fn selected_audiobookshelf_book_queue_item(
        &self,
        audiobookshelf_library_index: usize,
    ) -> Option<QueueItem> {
        let state = self
            .audiobookshelf_book_browse
            .get(audiobookshelf_library_index)?;
        let book = state.selected_id.as_ref()?;
        let book = state
            .books
            .iter()
            .find(|candidate| candidate.library_item_id == *book)?;
        if book.library_item_id.trim().is_empty() {
            return None;
        }
        let detail = state.detail_cache.get(&book.library_item_id);
        let duration_seconds = detail
            .map(|(_, audio_files)| audio_files.iter().map(|file| file.duration).sum())
            .filter(|duration| *duration > 0.0)
            .or_else(|| {
                detail.and_then(|(chapters, _)| {
                    chapters
                        .iter()
                        .map(|chapter| chapter.end)
                        .max_by(f64::total_cmp)
                })
            });
        let progress = state.progress.get(&book.library_item_id);
        let position_ticks = progress
            .map(|progress| seconds_to_ticks(progress.current_time_seconds))
            .unwrap_or(0);
        let is_finished = progress.is_some_and(|progress| progress.is_finished);

        Some(QueueItem::AudiobookshelfBook(AudiobookshelfBookQueueItem {
            library_item_id: book.library_item_id.clone(),
            title: book.title.clone(),
            author: book.author_display.clone(),
            duration_ticks: duration_seconds.and_then(seconds_to_ticks_u64),
            position_ticks,
            played: is_finished,
            is_finished,
            cover_path: book.cover_path.clone(),
        }))
    }

    pub(super) fn play_selected_audiobookshelf_book(&mut self, index: usize) {
        let Some(item) = self.selected_audiobookshelf_book_queue_item(index) else {
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
            .expect("selected Audiobookshelf book queue slot disappeared");
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
                "Playback owner rejected this Audiobookshelf book".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.set_queue_scope(scope);
        if !matches!(self.effective_panel_focus(), super::PanelFocus::Library) {
            self.set_panel_focus(super::PanelFocus::Queue);
        }
    }

    pub(super) fn enqueue_selected_audiobookshelf_book(&mut self, index: usize) {
        let Some(item) = self.selected_audiobookshelf_book_queue_item(index) else {
            return;
        };
        if !self.player.can_admit_audiobookshelf() {
            self.flash(
                "Audiobookshelf playback owner is unavailable".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.queue_for_scope_mut(self.visible_queue_scope())
            .queue
            .append(item);
        self.queue_dirty = true;
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

#[cfg(test)]
#[path = "audiobookshelf_book_seek_tests.rs"]
mod book_seek_tests;
