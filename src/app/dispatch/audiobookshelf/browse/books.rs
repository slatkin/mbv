use super::{App, QueueItem, ToastSeverity};
use mbv_ui_model::audiobookshelf_browse::books::audiobookshelf_book_queue_item;

// ---- Book browsing actions -----------------------------------------

impl App {
    pub(in crate::app) fn audiobookshelf_book_refresh(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let (library_id, generation, request) = {
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
                state.catalog_request,
            )
        };
        crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
            self.config.lock().unwrap().clone(),
            generation,
            request,
            library_id,
            0,
            self.channels.lib_tx.clone(),
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
        _selection: Option<mbv_ui_msg::BookChapterTarget>,
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
        target: Option<mbv_ui_msg::BookChapterTarget>,
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
        let active_index = self.player.status.lock().unwrap().current_idx;
        let active_book = self
            .playback_queue()
            .item_at(active_index)
            .and_then(mbv_queue::QueueItem::as_audiobookshelf_book)
            .is_some_and(|book| book.library_item_id == target.book_library_item_id());
        if active_book {
            let _ = self
                .player
                .send_command(mbv_ctrl::player::PlayerCommand::SeekAbsolute(
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

        let _ = self.submit_queue_item(item, true);
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
        let _ = self.submit_queue_item(item, false);
    }
}
