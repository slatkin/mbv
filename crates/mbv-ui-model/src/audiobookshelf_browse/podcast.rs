//! Podcast-tab browse state: the paged show list, the per-show episode
//! cache with its lazy fan-out bookkeeping, and the flat episode views.

use crate::feed_age::{FeedAgeGroup, feed_age_group};
use mbv_audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfLibrary, AudiobookshelfProgress,
    AudiobookshelfShow,
};
use mbv_queue::EpisodeResume;
use std::collections::{HashMap, HashSet};

/// The podcast tab's state pills, in the painted pill-bar order (spec: the
/// state pills `All` / `Unplayed` / `Played` precede the show pills).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum AudiobookshelfEpisodeFilter {
    #[default]
    All,
    Unplayed,
    Played,
}

impl AudiobookshelfEpisodeFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Unplayed, Self::Played];

    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Played => "Played",
            Self::Unplayed => "Unplayed",
        }
    }
}

/// The podcast tab's pill selection, stored by value and never by a painted
/// position (design D3): the show list grows and re-sorts as pages land
/// (`append_page` sorts by title), so an index would silently rebind the
/// active view. The remembered pill across tab switches is the same value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PillSelection {
    /// The library's cached Newest Episodes shelf, independent of show and state.
    Latest,
    /// A state pill: every fetched show's episodes, filtered by play state.
    State(AudiobookshelfEpisodeFilter),
    /// A show pill: that show's episodes regardless of play state, by the
    /// show's provider-native `library_item_id`.
    Show(String),
}

/// The staged replacement catalog for one in-flight refresh (design D2 of
/// `separate-data-refresh-from-ui-reset`): pages collect here while the
/// published `shows` list keeps serving the UI, so a slow or failed refresh
/// never empties or mutates the visible catalog. Only a completed traversal
/// publishes the batch (`commit_catalog_replacement`); a failure discards it
/// (`abort_catalog_replacement`).
#[derive(Debug, Clone, Default)]
pub struct ShowCatalogReplacement {
    /// The catalog request serial this batch was started under: a page or
    /// result carrying a different serial belongs to a superseded chain and
    /// is ignored.
    pub request: u64,
    pub shows: Vec<AudiobookshelfShow>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
}

#[derive(Debug, Clone)]
pub struct AudiobookshelfBrowseState {
    pub library: AudiobookshelfLibrary,
    /// The paged show list; it feeds the podcast pill bar (one pill per
    /// show). Page arrivals append at most once per show and sort by title.
    pub shows: Vec<AudiobookshelfShow>,
    pub total: usize,
    pub next_page: usize,
    pub loading_pages: HashSet<usize>,
    pub selected_id: Option<String>,
    pub error: Option<String>,
    /// Per-show downloaded-episode cache filled by the per-show episode
    /// fan-out: one entry per fetched show, keyed by that show's
    /// `library_item_id`. A re-arrival replaces the entry (append at most
    /// once); the flat episode views concatenate the entries in show order.
    pub detail_cache: HashMap<String, Vec<AudiobookshelfDownloadedEpisode>>,
    /// Show ids with an episode fetch in flight, holding the request serial
    /// each fetch was issued under (the state's monotonically increasing
    /// `next_detail_request`). A response retires — and may write the cache
    /// from — its own serial only, so an orphaned or superseded response can
    /// neither retire a newer request's mark nor overwrite a newer cache
    /// entry.
    pub detail_loading_ids: HashMap<String, u64>,
    /// Show ids whose episodes an in-flight refresh still owes a forced
    /// re-request (design D2): the bounded fan-out re-requests each even
    /// though its old episodes stay published until the fresh result lands,
    /// and retires the mark when the request is issued.
    pub refresh_pending: HashSet<String>,
    /// The serial issued to the most recent per-show episode fetch.
    pub next_detail_request: u64,
    /// The committed show-pill scope for the lazy episode fan-out (design
    /// D5): `Some(id)` = a show pill is active, so that show's episodes are
    /// the view's only requirement; `None` = a state pill (or the tab's
    /// default) is active, so every listed show is required. Written by the
    /// shell from the component's resolved pill movement, read by the
    /// fan-out scheduler — never a mirror of the owner's painted selection.
    pub committed_show_pill: Option<String>,
    /// The tab's selected episode, by `(library_item_id, episode_id)`
    /// identity — the same identity as the progress map. A refresh that
    /// removes the episode from the views clears it.
    pub selected_episode: Option<(String, String)>,
    pub progress: HashMap<(String, String), AudiobookshelfProgress>,
    /// The staged replacement catalog, `Some` only while a refresh batch is
    /// collecting. `append_replacement_page`/`replacement_needs_page` stage
    /// and drive it; the published `shows` list is untouched until
    /// `commit_catalog_replacement`.
    pub replacement: Option<ShowCatalogReplacement>,
    /// The serial issued to the most recent catalog request chain.
    pub next_catalog_request: u64,
    /// The catalog request serial the state currently expects: `0` for the
    /// initial load, bumped by `begin_catalog_replacement`. Page completions
    /// carrying a different serial are rejected as pre-refresh results.
    pub catalog_request: u64,
}

impl AudiobookshelfBrowseState {
    #[must_use]
    pub fn new(library: AudiobookshelfLibrary) -> Self {
        Self {
            library,
            shows: Vec::new(),
            total: 0,
            next_page: 0,
            loading_pages: HashSet::new(),
            selected_id: None,
            error: None,
            detail_cache: HashMap::new(),
            detail_loading_ids: HashMap::new(),
            refresh_pending: HashSet::new(),
            next_detail_request: 0,
            committed_show_pill: None,
            selected_episode: None,
            progress: HashMap::new(),
            replacement: None,
            next_catalog_request: 0,
            catalog_request: 0,
        }
    }

    #[must_use]
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
    }

    pub fn cache_detail(&mut self, id: String, episodes: Vec<AudiobookshelfDownloadedEpisode>) {
        self.detail_cache.insert(id, episodes);
    }

    /// The fetched episode with exactly this `(library_item_id, episode_id)`
    /// identity, from the per-show cache — regardless of which show's fetch
    /// placed it or which pill view is active.
    #[must_use]
    pub fn episode_by_identity(
        &self,
        library_item_id: &str,
        episode_id: &str,
    ) -> Option<&AudiobookshelfDownloadedEpisode> {
        self.detail_cache
            .get(library_item_id)?
            .iter()
            .find(|episode| episode.episode_id == episode_id)
    }

    /// The episode's resume state from the fetched progress, or not-started.
    #[must_use]
    pub fn episode_resume(&self, library_item_id: &str, episode_id: &str) -> EpisodeResume {
        self.progress
            .get(&(library_item_id.to_owned(), episode_id.to_owned()))
            .map_or(EpisodeResume::NOT_STARTED, |progress| {
                EpisodeResume::from_seconds(progress.current_time_seconds, progress.is_finished)
            })
    }

    /// The flat episode view: every fetched show's downloaded episodes,
    /// concatenated in show (pill-bar) order so the view is deterministic.
    /// The active pill scopes and the state filter narrows this view in the
    /// owner; here it is the unfiltered union.
    #[must_use]
    pub fn visible_episodes(
        &self,
        filter: AudiobookshelfEpisodeFilter,
    ) -> Vec<&AudiobookshelfDownloadedEpisode> {
        let mut visible = Vec::new();
        for show in &self.shows {
            if let Some(episodes) = self.detail_cache.get(&show.library_item_id) {
                visible.extend(self.visible_episodes_from(episodes, filter));
            }
        }
        // Each per-show run is already newest-first; one stable merge pass
        // over the whole union keeps undated episodes last.
        visible.sort_by(|left, right| {
            compare_publication_dates(left.published_at, right.published_at)
        });
        visible
    }

    #[must_use]
    pub fn visible_episodes_from<'a>(
        &self,
        source: &'a [AudiobookshelfDownloadedEpisode],
        filter: AudiobookshelfEpisodeFilter,
    ) -> Vec<&'a AudiobookshelfDownloadedEpisode> {
        let mut episodes = source
            .iter()
            .filter(|episode| match filter {
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
            compare_publication_dates(left.published_at, right.published_at)
        });
        episodes
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
        if let Some(selected_id) = self.selected_id.as_deref() {
            if self
                .shows
                .iter()
                .any(|show| show.library_item_id == selected_id)
                && self.cursor() >= self.shows.len()
            {
                self.select(self.shows.len().saturating_sub(1));
            } else if self.shows.len() >= self.total
                && !self
                    .shows
                    .iter()
                    .any(|show| show.library_item_id == selected_id)
            {
                self.select(0);
            }
        }
        let _ = limit;
    }

    #[must_use]
    pub fn needs_page(&self) -> Option<usize> {
        (self.shows.len() < self.total && self.loading_pages.is_empty()).then_some(self.next_page)
    }

    /// Starts a library-local replacement batch (design D2): the published
    /// catalog keeps serving the UI while pages stage separately. Returns the
    /// batch's request serial; the pre-refresh chain is superseded, and page 0
    /// is marked in flight for the caller's immediate request.
    pub fn begin_catalog_replacement(&mut self) -> u64 {
        self.next_catalog_request += 1;
        self.catalog_request = self.next_catalog_request;
        let mut replacement = ShowCatalogReplacement {
            request: self.catalog_request,
            ..ShowCatalogReplacement::default()
        };
        replacement.loading_pages.insert(0);
        self.replacement = Some(replacement);
        // The pre-refresh chain's in-flight page marks are superseded: its
        // results are discarded at the event boundary, so a leaked mark would
        // strand the published catalog behind an unretirable `loading_pages`
        // entry if the replacement later aborts (#745).
        self.loading_pages.clear();
        self.catalog_request
    }

    /// Arms a forced episode re-request for each of `shows` (design D2): the
    /// bounded fan-out re-requests them despite their cached episodes, which
    /// stay published until the fresh result succeeds.
    pub fn begin_detail_refresh(&mut self, shows: Vec<String>) {
        self.refresh_pending.extend(shows);
    }

    /// Whether `id` still awaits its forced refresh request.
    #[must_use]
    pub fn needs_detail_refresh(&self, id: &str) -> bool {
        self.refresh_pending.contains(id)
    }

    /// Retires `id`'s forced-refresh mark once its request is issued.
    pub fn retire_detail_refresh(&mut self, id: &str) {
        self.refresh_pending.remove(id);
    }

    /// Stages a replacement page when `request` is the current chain; a
    /// superseded request is ignored whole. Never touches the published
    /// catalog or its selection. Returns whether it staged.
    pub fn append_replacement_page(
        &mut self,
        request: u64,
        page: usize,
        limit: usize,
        total: usize,
        shows: Vec<AudiobookshelfShow>,
    ) -> bool {
        if self.catalog_request != request {
            return false;
        }
        let Some(replacement) = self.replacement.as_mut() else {
            return false;
        };
        replacement.loading_pages.remove(&page);
        replacement.total = total;
        replacement.next_page = replacement.next_page.max(page + 1);
        for show in shows {
            if !replacement
                .shows
                .iter()
                .any(|existing| existing.library_item_id == show.library_item_id)
            {
                replacement.shows.push(show);
            }
        }
        replacement
            .shows
            .sort_by_key(|show| show.title.to_lowercase());
        let _ = limit;
        true
    }

    /// The next page the active replacement needs, when `request` is current.
    #[must_use]
    pub fn replacement_needs_page(&self, request: u64) -> Option<usize> {
        if self.catalog_request != request {
            return None;
        }
        let replacement = self.replacement.as_ref()?;
        (replacement.shows.len() < replacement.total && replacement.loading_pages.is_empty())
            .then_some(replacement.next_page)
    }

    /// Publishes the staged replacement once traversal completed (design D2):
    /// the published list becomes the authoritative result, so changed
    /// metadata is updated and deleted shows are removed. The selected show,
    /// committed pill and selected episode are reconciled against that
    /// complete result only — never against a partial page. Returns whether a
    /// batch was published.
    pub fn commit_catalog_replacement(&mut self, request: u64) -> bool {
        if self.catalog_request != request {
            return false;
        }
        let Some(replacement) = self.replacement.take() else {
            return false;
        };
        self.shows = replacement.shows;
        self.total = replacement.total;
        self.next_page = replacement.next_page;
        self.loading_pages = replacement.loading_pages;
        self.shows.sort_by_key(|show| show.title.to_lowercase());
        let selected_still_present = self
            .selected_id
            .as_deref()
            .is_some_and(|id| self.shows.iter().any(|show| show.library_item_id == id));
        if !selected_still_present {
            self.selected_id = self.shows.first().map(|show| show.library_item_id.clone());
        }
        if self
            .committed_show_pill
            .as_deref()
            .is_some_and(|id| !self.shows.iter().any(|show| show.library_item_id == id))
        {
            self.committed_show_pill = None;
        }
        if self
            .selected_episode
            .as_ref()
            .is_some_and(|(show, _)| !self.shows.iter().any(|s| s.library_item_id == *show))
        {
            self.selected_episode = None;
        }
        // A show absent from the authoritative result no longer owes a
        // refresh request.
        self.refresh_pending
            .retain(|id| self.shows.iter().any(|show| &show.library_item_id == id));
        true
    }

    /// Discards the staged replacement on failure; the published catalog and
    /// its browsing context are untouched. Returns whether a batch was
    /// discarded.
    pub fn abort_catalog_replacement(&mut self, request: u64) -> bool {
        if self.catalog_request != request {
            return false;
        }
        self.replacement.take().is_some()
    }
}
fn compare_publication_dates(left: Option<u64>, right: Option<u64>) -> std::cmp::Ordering {
    match (left, right) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(left), Some(right)) => right.cmp(&left),
    }
}

/// One renderable row in the podcast tab's grouped flat episode list,
/// mirroring `FeedDisplayRow`: non-selectable age-group headings and spacers
/// around selectable `Entry` indices into the flat episode slice (grouping
/// never changes the indices or the stable `(library_item_id, episode_id)`
/// targeting). Consumed by the podcast owner (row 3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PodcastDisplayRow {
    Spacer,
    Heading(FeedAgeGroup),
    Entry(usize),
}

/// Groups the flat episode slice under the Feeds age groups. The slice is
/// sorted newest-first globally (undated episodes last) before the
/// consecutive-run merge, so an interleaved slice does not repeat headings;
/// a group with no episodes produces no heading.
#[must_use]
pub fn podcast_display_rows(
    episodes: &[AudiobookshelfDownloadedEpisode],
    now_secs: u64,
) -> Vec<PodcastDisplayRow> {
    let mut order: Vec<usize> = (0..episodes.len()).collect();
    order.sort_by(|&left, &right| {
        compare_publication_dates(episodes[left].published_at, episodes[right].published_at)
    });

    let mut rows = Vec::new();
    let mut last_group = None;
    for idx in order {
        let group = feed_age_group(episodes[idx].published_at, now_secs);
        if last_group != Some(group) {
            if last_group.is_some() {
                rows.push(PodcastDisplayRow::Spacer);
            }
            rows.push(PodcastDisplayRow::Heading(group));
            last_group = Some(group);
        }
        rows.push(PodcastDisplayRow::Entry(idx));
    }
    rows
}
