use crate::app::{App, LibEvent};

impl App {
    pub(super) fn handle_audiobookshelf_event(&mut self, ev: LibEvent) -> Option<LibEvent> {
        if let LibEvent::AudiobookshelfProgressAcknowledged(update) = ev {
            if !self.audiobookshelf_runtime.accepts(update.generation) {
                return None;
            }
            let position_ticks = crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks(
                update.current_time_seconds,
            );
            self.reconcile_audiobookshelf_progress(
                &update.library_item_id,
                &update.episode_id,
                position_ticks,
                update.current_time_seconds,
                update.is_finished,
            );
            return None;
        }
        if let LibEvent::AudiobookshelfBookProgressAcknowledged(update) = ev {
            if !self.audiobookshelf_runtime.accepts(update.generation) {
                return None;
            }
            let position_ticks = crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks(
                update.current_time_seconds,
            );
            self.reconcile_audiobookshelf_book_progress(
                &update.library_item_id,
                position_ticks,
                update.is_finished,
            );
            return None;
        }
        if let LibEvent::AudiobookshelfBooksFetched {
            generation,
            library_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return None;
            }
            if let Some(index) = self
                .audiobookshelf_libraries
                .iter()
                .position(|library| library.id == library_id)
            {
                let mut next_page = None;
                let mut selected_detail = None;
                if let Some(state) = self.audiobookshelf_book_browse.get_mut(index) {
                    match result {
                        Ok(page) => {
                            state.append_page_books(page.page, page.total, page.items);
                            next_page = state.needs_page();
                            if !state.detail_loading {
                                selected_detail = state.selected_id.clone();
                            }
                        }
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
                if let Some(selected_detail) = selected_detail {
                    self.start_audiobookshelf_book_detail(selected_detail);
                }
                if let Some(next_page) = next_page {
                    crate::app::dispatch::session::service_startup::start_audiobookshelf_books(
                        self.config.lock().unwrap().clone(),
                        generation,
                        library_id,
                        next_page,
                        self.lib_tx.clone(),
                    );
                }
            }
            return None;
        }
        if let LibEvent::AudiobookshelfBookDetailFetched {
            generation,
            library_item_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return None;
            }
            match result {
                Ok(detail) => {
                    if let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
                        state
                            .books
                            .iter()
                            .any(|book| book.library_item_id == library_item_id)
                    }) {
                        state.detail_loading_ids.remove(&library_item_id);
                        state.detail_loading = state
                            .selected_id
                            .as_ref()
                            .is_some_and(|id| state.detail_loading_ids.contains(id));
                        state.detail_cache.insert(library_item_id.clone(), detail);
                    }
                }
                Err(_error) => {
                    if let Some(state) = self.audiobookshelf_book_browse.iter_mut().find(|state| {
                        state
                            .books
                            .iter()
                            .any(|book| book.library_item_id == library_item_id)
                    }) {
                        state.detail_loading_ids.remove(&library_item_id);
                        state.detail_loading = state
                            .selected_id
                            .as_ref()
                            .is_some_and(|id| state.detail_loading_ids.contains(id));
                    }
                }
            }
            return None;
        }
        if let LibEvent::AudiobookshelfDetailFetched {
            generation,
            request,
            library_item_id,
            result,
        } = ev
        {
            let index = self.audiobookshelf_browse.iter().position(|state| {
                state
                    .shows
                    .iter()
                    .any(|show| show.library_item_id == library_item_id)
            });
            let Some(state) = index.and_then(|index| self.audiobookshelf_browse.get_mut(index))
            else {
                return None;
            };
            // The response belongs to this state only when the show's
            // in-flight mark still carries its request serial: an orphaned
            // response (its mark cleared by a refresh) or a superseded one (a
            // newer request for the show was issued) is discarded whole — it
            // must neither retire the newer request's mark nor write the
            // cache over a newer entry.
            if state.detail_loading_ids.get(&library_item_id) != Some(&request) {
                return None;
            }
            state.detail_loading_ids.remove(&library_item_id);
            // The mark is retired and the batch re-armed on the
            // rejected-generation path too: a generation bump between spawn
            // and arrival must not leak the in-flight slot and permanently
            // stall the remaining shows behind the bounded cap. A rejected
            // payload is still never cached.
            if self.audiobookshelf_runtime.accepts(generation) {
                match result {
                    Ok(episodes) => {
                        state.cache_detail(library_item_id, episodes);
                    }
                    Err(_error) => {
                        // A failed fetch consumed the show's once-per-session
                        // request: caching an empty result keeps the bounded
                        // fan-out from re-issuing it forever (design D5);
                        // the refresh key re-requests everything.
                        state.cache_detail(library_item_id, Vec::new());
                    }
                }
            }
            // The fan-out continues its bounded batch: the next required
            // show's request starts as this one retires (design D5).
            if let Some(index) = index {
                self.start_audiobookshelf_podcast_fan_out(index);
            }
            return None;
        }
        if let LibEvent::AudiobookshelfShowsFetched {
            generation,
            library_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return None;
            }
            if let Some(index) = self
                .audiobookshelf_libraries
                .iter()
                .position(|library| library.id == library_id)
            {
                let mut next_page = None;
                if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
                    match result {
                        Ok(page) => {
                            state.append_page(page.page, page.limit, page.total, page.items);
                            next_page = state.needs_page();
                        }
                        Err(error) => state.error = Some(error.to_string()),
                    }
                }
                // A landed page may list shows the active pill's fan-out has
                // not requested yet (a state pill requires every show); the
                // scheduler re-arms idempotently and stays bounded (design D5).
                self.start_audiobookshelf_podcast_fan_out(index);
                if let Some(next_page) = next_page {
                    crate::app::dispatch::session::service_startup::start_audiobookshelf_shows(
                        self.config.lock().unwrap().clone(),
                        generation,
                        library_id,
                        next_page,
                        self.lib_tx.clone(),
                    );
                }
            }
            return None;
        }
        if let LibEvent::AudiobookshelfShelfFetched {
            generation,
            library_id,
            result,
        } = ev
        {
            if !self.audiobookshelf_runtime.accepts(generation) {
                return None;
            }
            if let Ok(shelves) = result {
                let items = App::newest_episodes_items(shelves);
                self.audiobookshelf_shelf_cache.insert(library_id, items);
            }
            return None;
        }
        Some(ev)
    }
}
