//! The GUI Library's Podcasts projection (add-gui-library-panel design D12,
//! row 10.1): the interim collection pill row of All and one pill per show,
//! the selection's episode list in the list's sort — Title orders the episode
//! titles, and the date sorts use the shared `compare_gui_date_order` rule,
//! newest first with undated episodes last while Newest — the core's selected
//! episode, and its Hero. All merges every listed show's episodes; a checked
//! set merges the checked shows'. The list shows the letter rail exactly while
//! its sort is a name sort. The Hero is the shared
//! `hero_content_abs_episode` producer's facts — the episode title, the show,
//! the duration, and the publish date — its description, and the parent
//! show's Square cover, which the Service-scoped cover chain caches. The
//! per-show episode views are the TUI's own
//! (`AudiobookshelfBrowseState::visible_episodes`, `visible_episodes_from`),
//! so the two Clients cannot drift; the projection reads `App` data only.

use mbv_audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfProgress, AudiobookshelfShow,
};
use mbv_emby_model::seconds_to_ticks;
use mbv_queue::{AudiobookshelfEpisodeCatalog, AudiobookshelfQueueItem};
use mbv_ui_model::audiobookshelf_browse::{AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter};
use mbv_ui_model::library_hero::ArtworkSource;
use mbv_ui_model::library_hero::producers::hero_content_abs_episode;
use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::ui_util::row_date;
use mbv_ui_msg::PodcastEpisodeTarget;
use std::collections::HashSet;

use super::GuiLibraryArtwork;
use super::GuiListSort;
use super::GuiSortTarget;
use super::artwork::{GuiRowMediaKind, catalog_show_row_source};
use super::collection::{GuiCollectionSet, GuiCollectionTarget};
use super::entry_list::{GuiEntryListProjection, GuiEntryRow, picked_or_first};
use super::home::{GuiArtShape, artwork_shape};
use super::list_sort::gui_title_sort_key;
use crate::app::App;
use crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks_u64;
use crate::gui::GuiCore;
use mbv_ui_model::meta::MetaSegment;

/// One episode's stable identity across every list view (row 10.1): the
/// provider's `(library_item_id, episode_id)` pair — the identity the progress
/// map and the queue-item resolver use — because an episode id alone names no
/// row across shows. The frame carries it as one opaque string, in the form
/// [`GuiPodcastEpisodeId::encode`] writes and
/// [`GuiPodcastEpisodeId::decode`] reads, so a command built from the frame
/// addresses the episode that frame painted or nothing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPodcastEpisodeId {
    library_item_id: String,
    episode_id: String,
}

impl GuiPodcastEpisodeId {
    /// The identity of the episode `episode_id` of the show `library_item_id`.
    #[must_use]
    pub fn new(library_item_id: String, episode_id: String) -> Self {
        Self {
            library_item_id,
            episode_id,
        }
    }

    /// The parent show's provider-native `library_item_id`.
    #[must_use]
    pub fn library_item_id(&self) -> &str {
        &self.library_item_id
    }

    /// The episode's provider-native `episode_id`.
    #[must_use]
    pub fn episode_id(&self) -> &str {
        &self.episode_id
    }

    /// The row id the frame carries: the byte length of the show id, a `:`,
    /// then the show id and the episode id run together. The length prefix is
    /// what makes the pair one unambiguous string whatever the ids hold — a
    /// separator would collide with an id that contains it — and the decode is
    /// its only reader.
    #[must_use]
    pub fn encode(&self) -> String {
        format!(
            "{}:{}{}",
            self.library_item_id.len(),
            self.library_item_id,
            self.episode_id
        )
    }

    /// The identity the frame's row id names, or `None` when the string is no
    /// encoding of this form: a length that is not a number, a length past the
    /// end, or a split that leaves one half empty. Every command arm treats
    /// `None` as a stale frame's id and changes nothing.
    #[must_use]
    pub fn decode(encoded: &str) -> Option<Self> {
        let (length, rest) = encoded.split_once(':')?;
        let length: usize = length.parse().ok()?;
        let library_item_id = rest.get(..length)?;
        let episode_id = rest.get(length..)?;
        if library_item_id.is_empty() || episode_id.is_empty() {
            return None;
        }
        Some(Self {
            library_item_id: library_item_id.to_string(),
            episode_id: episode_id.to_string(),
        })
    }

    /// The player's target for this identity: the value the TUI's own play,
    /// chapter, and select entries take.
    #[must_use]
    pub fn target(&self) -> PodcastEpisodeTarget {
        PodcastEpisodeTarget::new(self.library_item_id.clone(), self.episode_id.clone())
    }
}

impl From<&PodcastEpisodeTarget> for GuiPodcastEpisodeId {
    fn from(target: &PodcastEpisodeTarget) -> Self {
        Self::new(
            target.library_item_id().to_string(),
            target.episode_id().to_string(),
        )
    }
}

/// One row of the Podcasts list (row 10.1) — the shared collection-scoped
/// entry-list row. The identity is the episode's stable `(library_item_id,
/// episode_id)` pair, encoded as one opaque string a select, play, or enqueue
/// command sends, never a row position.
pub type GuiPodcastsRow = GuiEntryRow;

/// The selected episode's Hero facts (row 10.1): the shared
/// `hero_content_abs_episode` producer's facts — the episode title, the show
/// title, the duration, and the publish date — with the cleaned description as
/// the overview, and the parent show's Square cover. Podcasts have no
/// Workspace, so none rides here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPodcastsHero {
    /// The hero's title: the episode title.
    pub title: String,
    /// The name of the largest unit the item belongs to (the invariant 30 header
    /// title rule): the show, the feed, or the series. `None` when the item is
    /// its own largest unit. The meta rows never repeat it.
    pub parent: Option<String>,
    /// The meta rows in display order: the show title, the duration, and the
    /// publish date, exactly as the producer orders them.
    pub meta_rows: Vec<Vec<MetaSegment>>,
    /// Which meta row paints the duration role, when one does.
    pub duration_row: Option<usize>,
    /// Which meta row paints the progress role. The shared producer fills
    /// none for an episode, so this is `None` exactly as the TUI's is.
    pub progress_row: Option<usize>,
    /// The cleaned description text, when the episode's payload carries one.
    pub overview: Option<String>,
    /// The artwork box the Hero reserves: the episode artwork policy's
    /// Square, which is every podcast's shape.
    pub art_shape: GuiArtShape,
    /// The parent show's cover, fetched through the Service-scoped cover
    /// chain. `None` when the show declares no cover or no server is
    /// configured; the window content paints its own placeholder there.
    pub art: Option<GuiLibraryArtwork>,
}

/// The Podcasts projection the GUI Library panel paints (row 10.1) — the
/// shared collection-scoped entry-list projection over the collection selector
/// and Hero: All and the shows with their checked state, the selection's
/// episode rows, the core's selected episode, and its Hero. No Workspace rides
/// here.
pub type GuiPodcastsProjection = GuiEntryListProjection<GuiPodcastsHero>;

impl GuiCore {
    /// The Podcasts projection the GUI Library panel paints (row 10.1): the
    /// collection selector, the selection's episode rows in the list's sort
    /// order, the selected episode, and its Hero. A `lib_idx` that names no
    /// podcast library projects the empty projection, as the frame's gate
    /// excludes that destination anyway.
    #[must_use]
    pub fn gui_podcasts_projection(&mut self, lib_idx: usize) -> GuiPodcastsProjection {
        podcasts_projection(&mut self.app, lib_idx)
    }
}

/// The Podcasts projection the GUI Library panel paints (row 10.1).
fn podcasts_projection(app: &mut App, lib_idx: usize) -> GuiPodcastsProjection {
    if !super::podcasts::podcast_library(app, lib_idx) {
        return GuiPodcastsProjection::default();
    }
    // The one effective-set read the frame takes: the rows read the set
    // it borrows, so the frame never recomputes the retention.
    let set = super::podcasts::podcast_effective_set(app, lib_idx);
    let sort_frame = super::list_sort::list_sort_frame(app, GuiSortTarget::Podcasts(lib_idx));
    let sort = sort_frame.active;
    let collections = app.gui_collections(GuiCollectionTarget::Podcasts(lib_idx));
    let (rows, loading, failed) = podcast_rows(app, lib_idx, set.as_ref(), sort);
    let selected = podcast_selected(app, lib_idx, &rows);
    let hero = selected
        .as_deref()
        .and_then(GuiPodcastEpisodeId::decode)
        .and_then(|episode| podcast_hero(app, lib_idx, &episode));
    GuiPodcastsProjection {
        collections,
        loading,
        failed,
        rows,
        sort: Some(sort_frame),
        selected,
        hero,
    }
}

/// The Podcasts collection selector's catalog (row 10.1): each listed show
/// by title — the order the show list already holds — as its
/// `library_item_id` key and title. The catalog streams borrowed shows and
/// clones each pair as the selector keeps it, with no freshly cloned
/// vector between the two.
pub(in crate::app::gui_library) fn podcast_shows(
    app: &App,
    lib_idx: usize,
) -> impl Iterator<Item = (String, String)> {
    app.audiobookshelf_browse
        .get(lib_idx)
        .into_iter()
        .flat_map(|state| state.shows.iter())
        .map(|show| (show.library_item_id.clone(), show.title.clone()))
}

/// The selection's episode rows with the view's loading and failure states
/// (row 10.1; gui-collection-dropdown design D2, D4): All merges every listed
/// show's cached episodes, and a checked set merges the checked shows' — both
/// ordered by `sort` (design D6).
fn podcast_rows(
    app: &App,
    lib_idx: usize,
    set: &GuiCollectionSet,
    sort: GuiListSort,
) -> (Vec<GuiPodcastsRow>, bool, bool) {
    let Some(state) = app.audiobookshelf_browse.get(lib_idx) else {
        return (Vec::new(), true, false);
    };
    let mut episodes = match set {
        GuiCollectionSet::All => state.visible_episodes(AudiobookshelfEpisodeFilter::All),
        GuiCollectionSet::Checked(keys) => {
            // The merge walks the shows, not the keys: the final stable
            // date sort keeps equal dates in merge order, so the
            // show-order walk alone yields the identical order. The key
            // set is borrowed and hashed once, so the walk is one
            // hash probe per show instead of a linear scan per show.
            let keys: HashSet<&str> = keys.iter().map(String::as_str).collect();
            let mut visible = Vec::with_capacity(
                state
                    .shows
                    .iter()
                    .filter(|show| keys.contains(show.library_item_id.as_str()))
                    .filter_map(|show| state.detail_cache.get(show.library_item_id.as_str()))
                    .map(Vec::len)
                    .sum(),
            );
            for show in &state.shows {
                if !keys.contains(show.library_item_id.as_str()) {
                    continue;
                }
                if let Some(cached) = state.detail_cache.get(show.library_item_id.as_str()) {
                    visible.extend(
                        state.visible_episodes_from(cached, AudiobookshelfEpisodeFilter::All),
                    );
                }
            }
            visible
        }
    };
    if sort.is_name_sort() {
        episodes.sort_by_cached_key(|episode| {
            (
                gui_title_sort_key(&episode.title),
                episode.published_at.is_none(),
                std::cmp::Reverse(episode.published_at.unwrap_or_default()),
                episode.library_item_id.clone(),
                episode.episode_id.clone(),
            )
        });
    } else {
        episodes.sort_by(|left, right| {
            sort.compare_date(left.published_at, right.published_at)
                .then_with(|| {
                    (left.library_item_id.as_str(), left.episode_id.as_str())
                        .cmp(&(right.library_item_id.as_str(), right.episode_id.as_str()))
                })
        });
    }
    let fetching = match set {
        GuiCollectionSet::Checked(keys) if keys.len() == 1 => {
            let show_id = keys.first().map(String::as_str).unwrap_or_default();
            state.detail_loading_ids.contains_key(show_id)
        }
        _ => !state.detail_loading_ids.is_empty(),
    };
    let rows = episodes
        .iter()
        .map(|episode| podcast_row(app, state, episode, sort))
        .collect();
    (
        rows,
        fetching || !state.loading_pages.is_empty(),
        state.error.is_some(),
    )
}

/// The episode the Hero shows (row 10.1): the core's pick while the active
/// list still carries it, else that list's first row — the Hero default.
fn podcast_selected(app: &App, lib_idx: usize, rows: &[GuiPodcastsRow]) -> Option<String> {
    picked_or_first(
        super::podcasts::podcast_picked_episode(app, lib_idx)
            .map(GuiPodcastEpisodeId::from)
            .map(|episode| episode.encode()),
        |picked| rows.iter().any(|row| row.id == picked),
        || rows.first().map(|row| row.id.clone()),
    )
}

/// The selected episode's Hero (row 10.1): the shared
/// `hero_content_abs_episode` producer's facts and description, and the
/// parent show's cover fetched through the Service-scoped cover chain —
/// the same fetch the TUI's hero projection issues.
fn podcast_hero(
    app: &mut App,
    lib_idx: usize,
    episode: &GuiPodcastEpisodeId,
) -> Option<GuiPodcastsHero> {
    let item = podcast_hero_item(app, lib_idx, episode)?;
    let content = hero_content_abs_episode(&item);
    let art = match &content.facts.artwork.source {
        Some(ArtworkSource::AudiobookshelfCover {
            library_item_id,
            book: false,
        }) => podcast_episode_artwork(app, library_item_id),
        Some(_) | None => None,
    };
    // The show is the largest unit and rides as the parent: the producer puts
    // it first, so the rows drop it and the duration index follows.
    let parent = item.show_title.clone().filter(|show| !show.is_empty());
    let skip = usize::from(parent.is_some());
    let duration_row = content
        .facts
        .duration_row
        .and_then(|row| row.checked_sub(skip));
    Some(GuiPodcastsHero {
        title: content.facts.title.clone(),
        parent,
        meta_rows: content.facts.meta_rows.iter().skip(skip).cloned().collect(),
        duration_row,
        progress_row: content.facts.progress_row,
        overview: content.overview.clone(),
        art_shape: artwork_shape(super::artwork::hero_shape(
            app,
            content.facts.artwork.shape,
            art.as_ref(),
        )),
        art,
    })
}

/// The episode as the shared hero producer's input (row 10.1): the cached
/// episode over its parent show's identity, title, and cover — the same
/// catalog the TUI's podcast owner builds for its own hero
/// (`PodcastContent::selected_episode_item`), the description included.
/// `None` when the per-show cache no longer carries the episode.
pub(in crate::app::gui_library) fn podcast_hero_item(
    app: &App,
    lib_idx: usize,
    episode: &GuiPodcastEpisodeId,
) -> Option<AudiobookshelfQueueItem> {
    let state = app.audiobookshelf_browse.get(lib_idx)?;
    let episode = state.episode_by_identity(episode.library_item_id(), episode.episode_id())?;
    let show = state
        .shows
        .iter()
        .find(|show| show.library_item_id == episode.library_item_id);
    let catalog = AudiobookshelfEpisodeCatalog {
        library_item_id: episode.library_item_id.clone(),
        episode_id: episode.episode_id.clone(),
        title: episode.title.clone(),
        show_title: show.map(|show| show.title.clone()),
        author: show.and_then(|show| show.author.clone()),
        description: episode.description.clone(),
        duration_ticks: episode.duration_seconds.and_then(seconds_to_ticks_u64),
        pub_date_secs: episode.published_at,
        cover_path: show.and_then(|show| show.cover_path.clone()),
    };
    let resume = state.episode_resume(&catalog.library_item_id, &catalog.episode_id);
    Some(AudiobookshelfQueueItem::from_catalog(catalog, resume))
}

/// The Hero's cover artwork (row 10.1): the Service-scoped show-cover
/// cache key the episode's artwork policy names, fetched here and encoded
/// from the image cache once per key — the same resolution the other
/// projections' hero art runs. `None` when no Audiobookshelf server is
/// configured.
fn podcast_episode_artwork(app: &mut App, show_id: &str) -> Option<GuiLibraryArtwork> {
    let cache_key = app.audiobookshelf_cover_key(show_id)?;
    Some(super::artwork::resolve_gui_art(app, &cache_key))
}

/// One row of the active list (row 10.1): the TUI podcast row's facts —
/// the episode title, the parent show's title, and the publish date in the
/// right-aligned gutter — with the row's played state, the rail letter the
/// list's name sort gives it (design D6), and the show cover the episode
/// artwork policy names (gui-delineated-rows design D5), read from the cache
/// alone.
fn podcast_row(
    app: &App,
    state: &AudiobookshelfBrowseState,
    episode: &AudiobookshelfDownloadedEpisode,
    sort: GuiListSort,
) -> GuiPodcastsRow {
    let show = show_of(state, &episode.library_item_id);
    let progress = state
        .progress
        .get(&(episode.library_item_id.clone(), episode.episode_id.clone()));
    let art = app.gui_row_art(show.and_then(catalog_show_row_source).as_ref());
    GuiPodcastsRow {
        id: GuiPodcastEpisodeId::new(episode.library_item_id.clone(), episode.episode_id.clone())
            .encode(),
        title: episode.title.clone(),
        subtitle: show.map(|show| show.title.clone()),
        date: row_date(episode.published_at),
        state: episode_row_state(progress, episode.duration_seconds),
        sort_letter: sort.rail_letter(&episode.title),
        art,
        media: GuiRowMediaKind::Audio,
    }
}

/// The parent show of `library_item_id` in the published show list.
fn show_of<'a>(
    state: &'a AudiobookshelfBrowseState,
    library_item_id: &str,
) -> Option<&'a AudiobookshelfShow> {
    state
        .shows
        .iter()
        .find(|show| show.library_item_id == library_item_id)
}

/// One row's played state (row 10.1): the TUI podcast row's rule through the
/// shared Audiobookshelf constructor — a finished episode is `Played`
/// whatever its position, because the server restarts it; an unfinished one
/// is `Active` exactly when Play would resume from its position, with the
/// progress percentage of its duration (no percentage when the duration is
/// unknown, but still `Active`); and an episode with no progress entry has
/// neither fact, so its row is `Ordinary`.
fn episode_row_state(
    progress: Option<&AudiobookshelfProgress>,
    duration_seconds: Option<f64>,
) -> MediaSemanticState {
    match progress {
        Some(progress) => MediaSemanticState::from_audiobookshelf(
            progress.is_finished,
            seconds_to_ticks(progress.current_time_seconds),
            duration_seconds.map_or(0, seconds_to_ticks),
        ),
        None => MediaSemanticState::Ordinary,
    }
}
