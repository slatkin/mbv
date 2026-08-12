use mbv_core::audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfProgress,
    AudiobookshelfShow,
};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AudiobookshelfEpisodeFilter {
    All,
    Played,
    Unplayed,
}

impl AudiobookshelfEpisodeFilter {
    pub(super) const ALL: [Self; 3] = [Self::All, Self::Played, Self::Unplayed];

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Played => "Played",
            Self::Unplayed => "Unplayed",
        }
    }
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
    pub progress: HashMap<(String, String), AudiobookshelfProgress>,
    pub episode_filter: AudiobookshelfEpisodeFilter,
    pub episode_selection: Option<usize>,
    pub scroll: usize,
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
            progress: HashMap::new(),
            episode_filter: AudiobookshelfEpisodeFilter::All,
            episode_selection: None,
            scroll: 0,
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
        let previous_id = self.selected_id.clone();
        self.selected_id = self
            .shows
            .get(cursor)
            .map(|show| show.library_item_id.clone());
        if self.selected_id != previous_id {
            self.episode_filter = AudiobookshelfEpisodeFilter::All;
        }
        self.episodes = self
            .selected_id
            .as_ref()
            .and_then(|id| self.detail_cache.get(id).cloned());
        self.detail_loading = false;
        self.episode_selection = None;
    }

    pub fn cache_detail(&mut self, id: String, episodes: Vec<AudiobookshelfDownloadedEpisode>) {
        self.detail_cache.insert(id, episodes);
    }

    pub fn selected_show(&self) -> Option<&AudiobookshelfShow> {
        let id = self.selected_id.as_deref()?;
        self.shows.iter().find(|show| show.library_item_id == id)
    }

    pub fn visible_episodes(&self) -> Vec<&AudiobookshelfDownloadedEpisode> {
        let mut episodes = self
            .episodes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter(|episode| match self.episode_filter {
                AudiobookshelfEpisodeFilter::All => true,
                AudiobookshelfEpisodeFilter::Played => self
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .is_some_and(|progress| progress.is_finished),
                AudiobookshelfEpisodeFilter::Unplayed => !self
                    .progress
                    .get(&(episode.library_item_id.clone(), episode.episode_id.clone()))
                    .is_some_and(|progress| progress.is_finished),
            })
            .collect::<Vec<_>>();
        episodes.sort_by(|left, right| {
            compare_publication_dates(left.published_at.as_deref(), right.published_at.as_deref())
        });
        episodes
    }

    pub fn set_episode_filter(&mut self, filter: AudiobookshelfEpisodeFilter) {
        self.episode_filter = filter;
        if self.episode_selection.is_some() {
            self.episode_selection = Some(0);
        }
    }

    pub fn enter_episode_selection(&mut self) {
        self.episode_selection = Some(0);
    }

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

    pub fn needs_page(&self) -> Option<usize> {
        (self.shows.len() < self.total && self.loading_pages.is_empty()).then_some(self.next_page)
    }
}

fn compare_publication_dates(left: Option<&str>, right: Option<&str>) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(left), Some(right)) => match (left.parse::<f64>(), right.parse::<f64>()) {
            (Ok(left), Ok(right)) => right
                .partial_cmp(&left)
                .unwrap_or(std::cmp::Ordering::Equal),
            _ => right.cmp(left),
        },
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
        state.enter_episode_selection();
        state.select(1);
        assert_eq!(state.episodes, None);
        assert_eq!(state.episode_selection, None);
        assert_eq!(state.episode_filter, AudiobookshelfEpisodeFilter::All);
    }

    #[test]
    fn empty_episodes_and_missing_progress_are_unstarted() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(1, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(Vec::new());
        assert_eq!(state.shows.len(), 1);
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

    #[test]
    fn filters_completed_progress_and_treats_partial_as_unplayed() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(0, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(vec![
            episode("a", "finished"),
            episode("a", "partial"),
            episode("a", "missing"),
        ]);
        state.progress.insert(
            ("a".into(), "finished".into()),
            AudiobookshelfProgress {
                library_item_id: "a".into(),
                episode_id: "finished".into(),
                current_time_seconds: 1.0,
                is_finished: true,
            },
        );
        state.progress.insert(
            ("a".into(), "partial".into()),
            AudiobookshelfProgress {
                library_item_id: "a".into(),
                episode_id: "partial".into(),
                current_time_seconds: 1.0,
                is_finished: false,
            },
        );

        state.set_episode_filter(AudiobookshelfEpisodeFilter::Played);
        assert_eq!(state.visible_episodes()[0].episode_id, "finished");
        state.set_episode_filter(AudiobookshelfEpisodeFilter::Unplayed);
        assert_eq!(
            state
                .visible_episodes()
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["partial", "missing"]
        );
    }

    #[test]
    fn visible_episodes_are_newest_first_with_undated_last() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(0, 20, 1, vec![show("a", "A")]);
        state.episodes = Some(vec![
            episode_with_date("a", "old", Some("2026-01-01")),
            episode_with_date("a", "undated", None),
            episode_with_date("a", "new", Some("2026-08-12")),
        ]);

        assert_eq!(
            state
                .visible_episodes()
                .into_iter()
                .map(|episode| episode.episode_id.as_str())
                .collect::<Vec<_>>(),
            ["new", "old", "undated"]
        );
    }

    fn episode_with_date(
        show: &str,
        id: &str,
        published_at: Option<&str>,
    ) -> AudiobookshelfDownloadedEpisode {
        AudiobookshelfDownloadedEpisode {
            library_item_id: show.into(),
            episode_id: id.into(),
            title: id.into(),
            published_at: published_at.map(str::to_string),
            duration_seconds: None,
        }
    }
}
