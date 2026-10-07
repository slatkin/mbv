use super::{App, AudiobookshelfItem, QueueItem, ToastSeverity};
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBookBrowseState;
use mbv_ui_model::audiobookshelf_browse::books::{
    audiobookshelf_book_item, audiobookshelf_book_queue_item,
};

/// What chapter-row activation resolves into before any effect runs
/// (book-playback spec): one absolute seek on the active book's merged
/// timeline, a replace-and-start of a book that is not the active slot at
/// the chapter's offset, or a concise failure to flash (never a silent
/// no-op).
enum ChapterActivation {
    Seek(f64),
    Play(QueueItem),
    Failed(&'static str),
}

/// Resolves chapter-row activation from the book browse state. The stable
/// Service discriminator (chapter number) is resolved, never a display
/// position: a refresh that re-composes the chapter rows must not make this
/// activation land on a different chapter (design.md D4).
fn resolve_chapter_activation(
    state: &AudiobookshelfBookBrowseState,
    target: &mbv_ui_msg::BookChapterTarget,
    active_book_id: Option<&str>,
) -> ChapterActivation {
    let Some((chapters, _)) = state.detail_cache.get(target.book_library_item_id()) else {
        return ChapterActivation::Failed("Chapter details are not loaded for this book");
    };
    let Some(chapter) = chapters
        .iter()
        .find(|chapter| chapter.id == target.row_discriminator())
    else {
        return ChapterActivation::Failed("That chapter is no longer available");
    };
    let target_seconds = chapter.start;
    if active_book_id == Some(target.book_library_item_id()) {
        return ChapterActivation::Seek(target_seconds);
    }
    let Some(mut book) = audiobookshelf_book_item(state, target.book_library_item_id()) else {
        return ChapterActivation::Failed("That book is no longer in the library");
    };
    // The player honors the book item's position as the resume position on
    // the merged timeline (`resume_seconds`), so starting at the chapter
    // offset needs no separate seek.
    book.position_ticks = super::seconds_to_ticks(target_seconds);
    ChapterActivation::Play(QueueItem::Audiobookshelf(AudiobookshelfItem::Book(book)))
}

// ---- Book browsing actions -----------------------------------------

impl App {
    pub(in crate::app) fn audiobookshelf_book_refresh(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        // Coalesce a duplicate F5 while a replacement batch is already
        // collecting (design D2).
        if self
            .audiobookshelf_book_browse
            .get(index)
            .is_some_and(|state| state.replacement.is_some())
        {
            return;
        }
        let (library_id, generation, request, selected) = {
            let Some(state) = self.audiobookshelf_book_browse.get_mut(index) else {
                return;
            };
            state.error = None;
            let request = state.begin_catalog_replacement();
            (
                state.library.id.clone(),
                self.audiobookshelf_runtime.generation(),
                request,
                state.selected_id.clone(),
            )
        };
        // Request the staged replacement from page 0; the published catalog
        // and detail cache keep serving the UI until the completed
        // replacement commits.
        crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
            self.config.lock().unwrap().clone(),
            generation,
            request,
            library_id,
            0,
            self.channels.lib_tx.clone(),
        );
        // Refetch the selected book's detail without dropping its published
        // presentation; the old detail stays until the fresh result lands.
        if let Some(id) = selected {
            self.start_audiobookshelf_book_detail_inner(id, true);
        }
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

    /// Chapter-row activation (book-playback spec): on the active book, one
    /// absolute seek to `chapters[].start` on the merged timeline, without
    /// stopping/reopening the queue slot or session; on a book that is not
    /// the active slot, a replace-and-start of that book at the chapter's
    /// offset through the same play path the book row uses. Unresolvable
    /// chapters or books flash a concise error instead of silently
    /// doing nothing.
    pub(in crate::app) fn activate_audiobookshelf_book_row_target(
        &mut self,
        target: Option<mbv_ui_msg::BookChapterTarget>,
    ) {
        let Some(target) = target else { return };
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let active_index = self.player.status_snapshot().current_idx;
        let active_book_id = self
            .playback_queue()
            .item_at(active_index)
            .and_then(mbv_queue::QueueItem::as_audiobookshelf_book)
            .map(|book| book.library_item_id.clone());
        let activation = {
            let Some(state) = self.audiobookshelf_book_browse.get(index) else {
                return;
            };
            resolve_chapter_activation(state, &target, active_book_id.as_deref())
        };
        match activation {
            ChapterActivation::Seek(target_seconds) => {
                let _ = self
                    .player
                    .send_command(mbv_ctrl::player::PlayerCommand::SeekAbsolute(
                        target_seconds,
                    ));
            }
            ChapterActivation::Play(item) => {
                if !self.player.can_admit_audiobookshelf() {
                    self.flash(
                        "Audiobookshelf playback owner is unavailable".into(),
                        ToastSeverity::Error,
                    );
                    return;
                }
                let _ = self.submit_queue_item(item, true);
            }
            ChapterActivation::Failed(message) => {
                self.flash(message.into(), ToastSeverity::Error);
            }
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
