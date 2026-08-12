use super::types_audiobookshelf_browse::AudiobookshelfEpisodeFilter;
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
            state.detail_cache.clear();
            state.episodes = None;
            state.episode_selection = None;
            state.scroll = 0;
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
        let cursor = (state.cursor() as i64 + delta).clamp(0, state.shows.len() as i64 - 1);
        self.select_audiobookshelf_show(cursor as usize);
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
            state.episode_selection =
                Some((cursor as i64 + delta).clamp(0, count as i64 - 1) as usize);
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
}
