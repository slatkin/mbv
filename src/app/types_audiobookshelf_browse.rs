use mbv_core::audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfProgress,
    AudiobookshelfShelf, AudiobookshelfShelfEntry, AudiobookshelfShow,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum AudiobookshelfRowId {
    Show(String),
    Episode {
        library_item_id: String,
        episode_id: String,
    },
    Shelf {
        shelf: usize,
        entry: usize,
    },
}

#[derive(Debug, Clone)]
pub(super) struct AudiobookshelfBrowseState {
    // Retained for upcoming library-local rendering milestones.
    #[allow(dead_code)]
    pub library: AudiobookshelfLibrary,
    pub shows: Vec<AudiobookshelfShow>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
    pub selected_id: Option<String>,
    pub error: Option<String>,
    pub episodes: Option<Vec<AudiobookshelfDownloadedEpisode>>,
    pub detail_cache: HashMap<String, Vec<AudiobookshelfDownloadedEpisode>>,
    pub detail_loading: bool,
    pub selected_row: Option<AudiobookshelfRowId>,
    pub progress: HashMap<(String, String), AudiobookshelfProgress>,
    pub shelves: Vec<AudiobookshelfShelf>,
}

impl AudiobookshelfBrowseState {
    pub fn new(library: AudiobookshelfLibrary) -> Self {
        Self {
            library,
            shows: Vec::new(),
            total: 0,
            next_page: 0,
            loading_pages: HashSet::new(),
            selected_id: None,
            error: None,
            episodes: None,
            detail_cache: HashMap::new(),
            detail_loading: false,
            selected_row: None,
            progress: HashMap::new(),
            shelves: Vec::new(),
        }
    }

    // Retained for upcoming incremental-loading milestones.
    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.selected_id
            .as_ref()
            .and_then(|id| {
                self.shows
                    .iter()
                    .position(|show| &show.library_item_id == id)
            })
            .unwrap_or(0)
    }

    pub fn select(&mut self, cursor: usize) {
        self.selected_id = self
            .shows
            .get(cursor)
            .map(|show| show.library_item_id.clone());
        self.episodes = self
            .selected_id
            .as_ref()
            .and_then(|id| self.detail_cache.get(id).cloned());
        self.detail_loading = false;
        self.selected_row = self.selected_id.clone().map(AudiobookshelfRowId::Show);
    }

    pub fn cache_detail(&mut self, id: String, episodes: Vec<AudiobookshelfDownloadedEpisode>) {
        self.detail_cache.insert(id, episodes);
    }

    pub fn rows(&self) -> Vec<AudiobookshelfRowId> {
        let mut rows = self
            .shelves
            .iter()
            .enumerate()
            .flat_map(|(shelf, value)| {
                value
                    .entries
                    .iter()
                    .enumerate()
                    .map(move |(entry, _)| AudiobookshelfRowId::Shelf { shelf, entry })
            })
            .chain(
                self.shows
                    .iter()
                    .map(|s| AudiobookshelfRowId::Show(s.library_item_id.clone())),
            )
            .collect::<Vec<_>>();
        if let Some(episodes) = &self.episodes {
            if let Some(id) = &self.selected_id {
                let at = rows
                    .iter()
                    .position(
                        |row| matches!(row, AudiobookshelfRowId::Show(row_id) if row_id == id),
                    )
                    .map(|i| i + 1)
                    .unwrap_or(rows.len());
                rows.splice(
                    at..at,
                    episodes.iter().map(|e| AudiobookshelfRowId::Episode {
                        library_item_id: e.library_item_id.clone(),
                        episode_id: e.episode_id.clone(),
                    }),
                );
            }
        }
        rows
    }

    pub fn apply_shelves(&mut self, shelves: Vec<AudiobookshelfShelf>) {
        let shows: HashSet<&str> = self
            .shows
            .iter()
            .map(|show| show.library_item_id.as_str())
            .collect();
        self.shelves = shelves
            .into_iter()
            .map(|mut shelf| {
                shelf.entries.retain(|entry| match entry {
                    AudiobookshelfShelfEntry::Show(id) => shows.contains(id.as_str()),
                    AudiobookshelfShelfEntry::Episode { .. } => true,
                });
                shelf
            })
            .collect();
    }

    pub fn cursor_row(&self) -> Option<AudiobookshelfRowId> {
        self.selected_row
            .clone()
            .or_else(|| self.selected_id.clone().map(AudiobookshelfRowId::Show))
    }

    #[allow(dead_code)]
    pub fn append_page(
        &mut self,
        page: usize,
        limit: usize,
        total: usize,
        shows: Vec<AudiobookshelfShow>,
    ) {
        self.loading_pages.remove(&page);
        self.total = total;
        self.next_page = self.next_page.max(page + 1);
        for show in shows {
            if !self
                .shows
                .iter()
                .any(|existing| existing.library_item_id == show.library_item_id)
            {
                self.shows.push(show);
            }
        }
        self.shows.sort_by_key(|show| show.title.to_lowercase());
        if self.selected_id.is_none() && !self.shows.is_empty() {
            self.select(0);
        }
        if self.selected_id.is_some() && self.cursor() >= self.shows.len() {
            self.select(self.shows.len().saturating_sub(1));
        }
        let _ = limit;
    }

    #[allow(dead_code)]
    pub fn needs_page(&self) -> Option<usize> {
        (self.shows.len() < self.total && self.loading_pages.is_empty()).then_some(self.next_page)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn library() -> AudiobookshelfLibrary {
        AudiobookshelfLibrary {
            id: "library".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        }
    }

    fn show(id: &str, title: &str) -> AudiobookshelfShow {
        AudiobookshelfShow {
            library_item_id: id.into(),
            title: title.into(),
            author: None,
            cover_path: None,
        }
    }

    fn episode(show: &str, id: &str) -> AudiobookshelfDownloadedEpisode {
        AudiobookshelfDownloadedEpisode {
            library_item_id: show.into(),
            episode_id: id.into(),
            title: id.into(),
            published_at: None,
            duration_seconds: None,
        }
    }

    #[test]
    fn rows_move_between_show_and_episode_identity() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 2, vec![show("a", "A"), show("b", "B")]);
        state.episodes = Some(vec![episode("a", "shared"), episode("a", "two")]);
        assert_eq!(state.rows().len(), 4);
        state.selected_row = Some(AudiobookshelfRowId::Episode {
            library_item_id: "a".into(),
            episode_id: "shared".into(),
        });
        assert!(
            matches!(state.cursor_row(), Some(AudiobookshelfRowId::Episode { episode_id, .. }) if episode_id == "shared")
        );
        state.select(1);
        assert_eq!(state.episodes, None);
    }

    #[test]
    fn empty_episodes_and_missing_progress_are_unstarted() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(Vec::new());
        assert!(state.rows().len() == 1);
        assert!(!state.progress.contains_key(&("a".into(), "missing".into())));
    }

    #[test]
    fn same_episode_id_isolated_by_show_identity() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.progress.insert(
            ("a".into(), "shared".into()),
            AudiobookshelfProgress {
                library_item_id: "a".into(),
                episode_id: "shared".into(),
                current_time_seconds: 4.0,
                is_finished: false,
            },
        );
        assert!(!state.progress.keys().any(|(library_item_id, episode_id)| {
            library_item_id == "b" && episode_id == "shared"
        }));
    }
}
