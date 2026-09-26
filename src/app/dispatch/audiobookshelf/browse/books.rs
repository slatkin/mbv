use super::{seconds_to_ticks, seconds_to_ticks_u64, App, QueueItem, ToastSeverity};
pub(in crate::app) use mbv_core::playback_queue::AudiobookshelfBookQueueItem;

// ---- Book browsing actions -----------------------------------------

impl App {
    pub(in crate::app) fn audiobookshelf_book_refresh(&mut self) {
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
            state.loading_pages.clear();
            state.loading_pages.insert(0);
            (
                state.library.id.clone(),
                self.audiobookshelf_runtime.generation(),
            )
        };
        crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
            self.config.lock().unwrap().clone(),
            generation,
            library_id,
            0,
            self.lib_tx.clone(),
        );
    }

    pub(in crate::app) fn select_audiobookshelf_book(&mut self, cursor: usize) {
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

    pub(in crate::app) fn select_audiobookshelf_book_target(&mut self, target: &str) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(cursor) = self
            .audiobookshelf_book_browse
            .get(index)
            .and_then(|state| {
                state
                    .books
                    .iter()
                    .position(|book| book.library_item_id == target)
            })
        else {
            return;
        };
        self.select_audiobookshelf_book(cursor);
    }

    /// The book chapter focus is component-owned interaction state
    /// (split-browse-state-interaction-fields task 2.2): the component tracks
    /// it locally and carries the resolved row at activation time. This
    /// handler exists only so the `ChapterFocus` request stays claimed and
    /// routed (a redraw nudge); it stores nothing shell-side.
    pub(in crate::app) fn set_audiobookshelf_book_chapter_focus(
        _selection: Option<crate::app::components::msg::BookChapterTarget>,
    ) {
    }

    /// Selects bucket `bucket_pos` (a position in `state.buckets`, matching
    /// the pill's click target -- the established pattern from
    /// `select_music_group`), narrowing the right-pane list to it and
    /// re-anchoring the cursor into the new bucket when it falls outside.
    pub(in crate::app) fn select_audiobookshelf_book_bucket(&mut self, bucket_pos: usize) {
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
            let Some(state) = self.audiobookshelf_book_browse.get(index) else {
                return;
            };
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

    /// Chapter-row activation: one absolute seek to `chapters[].start` on the
    /// active book's merged timeline, without stopping/reopening the queue
    /// slot or session (book-playback spec).
    pub(in crate::app) fn activate_audiobookshelf_book_row_target(
        &mut self,
        target: Option<crate::app::components::msg::BookChapterTarget>,
    ) {
        let Some(target) = target else { return };
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_book_browse.get(index) else {
            return;
        };
        let Some((chapters, _)) = state.detail_cache.get(target.book_library_item_id()) else {
            return;
        };
        // Resolve the stable Service discriminator (chapter number), never a
        // display position: a refresh that re-composes the chapter rows must
        // not make this seek land on a different chapter (design.md D4).
        let Some(chapter) = chapters
            .iter()
            .find(|chapter| chapter.id == target.row_discriminator())
        else {
            return;
        };
        let target_seconds = chapter.start;
        let active_book = self
            .playback_queue()
            .queue
            .active_slot()
            .and_then(|slot| slot.item.as_audiobookshelf_book())
            .is_some_and(|book| book.library_item_id == target.book_library_item_id());
        if active_book {
            self.player
                .send_command(mbv_core::player::PlayerCommand::SeekAbsolute(
                    target_seconds,
                ));
        }
    }

    /// Resolve the selected book as a `QueueItem::Audiobookshelf(AudiobookshelfItem::Book)` without
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
        audiobookshelf_book_queue_item(state)
    }

    pub(in crate::app) fn play_selected_audiobookshelf_book(&mut self, index: usize) {
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

        let scope = self.playing_queue_scope();
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

        let all_slots = self.queue_for_scope(scope).all_queue_slots();
        let audio_only = all_slots.iter().all(|slot| slot.item.is_audio());
        let submitted = self.player.submit_queue_slots(
            all_slots,
            selected_index,
            self.queue_source.clone(),
            None,
            audio_only,
            self.ui_volume,
        );
        if !submitted {
            *self.queue_for_scope_mut(scope) = previous_queue;
            self.flash(
                "Playback owner rejected this Audiobookshelf book".into(),
                ToastSeverity::Error,
            );
            return;
        }
        self.set_queue_scope(scope);
        if !matches!(
            self.effective_panel_focus(),
            crate::app::PanelFocus::Library
        ) {
            self.set_panel_focus(crate::app::PanelFocus::Queue);
        }
    }

    pub(in crate::app) fn enqueue_selected_audiobookshelf_book(&mut self, index: usize) {
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
        self.queue_for_scope_mut(self.viewed_queue_scope())
            .queue
            .append(item);
        self.queue_dirty = true;
    }
}

/// Resolve the Books tab's selected book as a `QueueItem::Audiobookshelf(AudiobookshelfItem::Book)`
/// without mutating the queue or opening a playback lifecycle (factored out
/// of `App::selected_audiobookshelf_book_queue_item` for task 5.4: the Books
/// tab's selection path and Home's queue-item path feed the one hero
/// producer through this single conversion).
pub(in crate::app) fn audiobookshelf_book_queue_item(
    state: &crate::app::state::types::audiobookshelf_browse::AudiobookshelfBookBrowseState,
) -> Option<QueueItem> {
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
    let position_ticks = progress.map_or(0, |progress| {
        seconds_to_ticks(progress.current_time_seconds)
    });
    let is_finished = progress.is_some_and(|progress| progress.is_finished);

    Some(QueueItem::Audiobookshelf(
        mbv_core::playback_queue::AudiobookshelfItem::Book(AudiobookshelfBookQueueItem {
            library_item_id: book.library_item_id.clone(),
            title: book.title.clone(),
            author: book.author_display.clone(),
            duration_ticks: duration_seconds.and_then(seconds_to_ticks_u64),
            position_ticks,
            played: is_finished,
            is_finished,
            cover_path: book.cover_path.clone(),
        }),
    ))
}
