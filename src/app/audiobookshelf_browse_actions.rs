use super::App;

impl App {
    pub(super) fn audiobookshelf_refresh(&mut self) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
            state.shows.clear();
            state.total = 0;
            state.next_page = 0;
            state.error = None;
        }
    }
    pub(super) fn select_audiobookshelf_show(&mut self, cursor: usize) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get_mut(index) else {
            return;
        };
        if state.shows.is_empty() {
            return;
        }
        state.select(cursor.min(state.shows.len() - 1));
        if let Some(id) = state.selected_id.clone() {
            self.start_audiobookshelf_detail(id);
        }
    }

    /// Selects the show at `index`'s show list whose `library_item_id`
    /// matches `id`, if one exists. Returns whether a match was found.
    fn select_show_by_id(&mut self, index: usize, id: &str) -> bool {
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return false;
        };
        let Some(show) = state
            .shows
            .iter()
            .position(|show| show.library_item_id == id)
        else {
            return false;
        };
        self.select_audiobookshelf_show(show);
        true
    }

    pub(super) fn move_audiobookshelf_cursor(&mut self, delta: i64) {
        let Some(index) = self.tab.audiobookshelf_index() else {
            return;
        };
        let Some(state) = self.audiobookshelf_browse.get(index) else {
            return;
        };
        if state.shows.is_empty() {
            return;
        }
        let rows = state.rows();
        let current = state
            .cursor_row()
            .and_then(|row| rows.iter().position(|candidate| *candidate == row))
            .unwrap_or(0);
        let cursor = (current as i64 + delta).clamp(0, rows.len() as i64 - 1) as usize;
        if let Some(row) = rows.get(cursor).cloned() {
            match row {
                super::types_audiobookshelf_browse::AudiobookshelfRowId::Show(id) => {
                    self.select_show_by_id(index, &id);
                }
                super::types_audiobookshelf_browse::AudiobookshelfRowId::Shelf { shelf, entry } => {
                    let target = state
                        .shelves
                        .get(shelf)
                        .and_then(|value| value.entries.get(entry))
                        .cloned();
                    if let Some(target) = target {
                        match target {
                            mbv_core::audiobookshelf::AudiobookshelfShelfEntry::Show(id) => {
                                self.select_show_by_id(index, &id);
                            }
                            mbv_core::audiobookshelf::AudiobookshelfShelfEntry::Episode {
                                library_item_id,
                                episode_id,
                            } => {
                                if self.select_show_by_id(index, &library_item_id) {
                                    if let Some(state) = self.audiobookshelf_browse.get_mut(index) {
                                        state.selected_row = Some(super::types_audiobookshelf_browse::AudiobookshelfRowId::Episode { library_item_id, episode_id });
                                    }
                                }
                            }
                        }
                    }
                }
                row => self.audiobookshelf_browse[index].selected_row = Some(row),
            }
        }
    }
}
