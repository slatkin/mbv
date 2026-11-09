//! The GUI Library's Podcasts projection (add-gui-library-panel design D12,
//! row 10.1): the single-select pill row of Latest and one pill per show, the
//! active pill's episode list in the TUI's date order — newest first with
//! undated episodes last — the core's selected episode, and its Hero. Latest
//! merges every listed show's episodes; a show pill scopes the list to that
//! one show. The list shows no letter rail, because it is sorted by date. The
//! Hero is the shared `hero_content_abs_episode` producer's facts — the
//! episode title, the show, the duration, and the publish date — its
//! description, and the parent show's Square cover, which the Service-scoped
//! cover chain caches. The order and the per-pill view are the TUI's own
//! (`AudiobookshelfBrowseState::visible_episodes`,
//! `visible_episodes_from`), so the two Clients cannot drift; the projection
//! reads `App` data only.

use mbv_audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfProgress, AudiobookshelfShow,
};
use mbv_components::library_panel::content::ArtworkSource;
use mbv_components::library_panel::hero::hero_content_abs_episode;
use mbv_emby_model::saturating_i64_from_f64;
use mbv_queue::{AudiobookshelfEpisodeCatalog, AudiobookshelfQueueItem};
use mbv_ui_model::audiobookshelf_browse::{AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter};
use mbv_ui_model::ui_util::fmt_publish_date_short;
use mbv_ui_msg::PodcastEpisodeTarget;

use super::GuiLibraryArtwork;
use super::home::{GuiArtShape, GuiHomeRowState, artwork_shape};
use super::podcasts::GuiPodcastPill;
use crate::app::App;
use crate::app::dispatch::audiobookshelf::browse::seconds_to_ticks_u64;

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

/// One pill of the Podcasts pill row (row 10.1): Latest first, then one pill
/// per show in the show list's title order. The pill row is single-select, as
/// the TUI's one-valued `PillSelection` is; the state pills are dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPodcastsPillOption {
    /// The pill's selection: the set-pill command carries it back.
    pub pill: GuiPodcastPill,
    /// The pill's label: `Latest`, or the show's title.
    pub name: String,
    /// Whether the core's stored pill is this one.
    pub selected: bool,
}

/// One row of the Podcasts list (row 10.1). The identity is the episode's
/// stable `(library_item_id, episode_id)` pair, encoded as one opaque string a
/// select, play, or enqueue command sends, never a row position.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPodcastsRow {
    /// The episode identity, as [`GuiPodcastEpisodeId::encode`] writes it.
    pub id: String,
    /// The episode's display title, the row's primary label.
    pub title: String,
    /// The parent show's title, the row's secondary label — the one fact that
    /// names the row's show in the merged Latest view.
    pub subtitle: Option<String>,
    /// The gutter date: the TUI row's fixed right-aligned `day month` text
    /// (`fmt_publish_date_short` over the publish instant). `None` on an
    /// undated episode, and `Some("")` never rides here — the TUI's rows drop
    /// an empty gutter too.
    pub date: Option<String>,
    /// The row's played state: the TUI podcast row's arm — `Played` for a
    /// finished episode, `Active` with the resume percent while some of it is
    /// played, otherwise `Ordinary`.
    pub state: GuiHomeRowState,
}

/// The selected episode's Hero facts (row 10.1): the shared
/// `hero_content_abs_episode` producer's facts — the episode title, the show
/// title, the duration, and the publish date — with the cleaned description as
/// the overview, and the parent show's Square cover. Podcasts have no
/// Workspace, so none rides here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPodcastsHero {
    /// The hero's title: the episode title.
    pub title: String,
    /// The meta rows in display order: the show title, the duration, and the
    /// publish date, exactly as the producer orders them.
    pub meta_rows: Vec<String>,
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

/// The Podcasts projection the GUI Library panel paints (row 10.1): the
/// single-select pill row, the active pill's episode rows, the core's selected
/// episode, and its Hero. No letter rail rides here — the list is sorted by
/// date — and no Workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiPodcastsProjection {
    /// The pill row in paint order: Latest, then each show in the show list's
    /// title order. The selected pill is the core's stored one; a stored show
    /// pill the published show list no longer carries falls back to Latest.
    pub pills: Vec<GuiPodcastsPillOption>,
    /// Whether the active view's episodes are still landing: the show
    /// catalog's pages or that view's own episode fetches.
    pub loading: bool,
    /// Whether the catalog fetch failed, the browse state's error. The
    /// published episodes keep serving when they can.
    pub failed: bool,
    /// The active pill's episode rows: Latest merges every listed show's
    /// episodes, and a show pill holds that one show's, both newest first
    /// with undated episodes last.
    pub rows: Vec<GuiPodcastsRow>,
    /// The core's selected episode (row 10.1), as the frame painted it: the
    /// pick the select command last named while the active list still carries
    /// it, else that list's first row — the Hero default, so the Hero paints
    /// as soon as a list lands. `None` while the list holds no row.
    pub selected: Option<String>,
    /// The selected episode's Hero facts; `None` while no episode is selected.
    pub hero: Option<GuiPodcastsHero>,
}

impl App {
    /// The Podcasts projection the GUI Library panel paints (row 10.1): the
    /// pill row, the active pill's episode rows, the selected episode, and its
    /// Hero. A `lib_idx` that names no podcast library projects the empty
    /// projection, as the frame's gate excludes that destination anyway.
    #[must_use]
    pub fn gui_podcasts_projection(&mut self, lib_idx: usize) -> GuiPodcastsProjection {
        if !self.gui_podcast_library(lib_idx) {
            return GuiPodcastsProjection::default();
        }
        let pill = self.gui_podcast_effective_pill(lib_idx);
        let pills = self.gui_podcast_pills(lib_idx, &pill);
        let (rows, loading, failed) = self.gui_podcast_rows(lib_idx, &pill);
        let selected = self.gui_podcast_selected(lib_idx, &rows);
        let hero = selected
            .as_deref()
            .and_then(GuiPodcastEpisodeId::decode)
            .and_then(|episode| self.gui_podcast_hero(lib_idx, &episode));
        GuiPodcastsProjection {
            pills,
            loading,
            failed,
            rows,
            selected,
            hero,
        }
    }

    /// The pill the list and the pill row paint (row 10.1): the core's stored
    /// pill, with a show the published show list no longer carries falling
    /// back to Latest. A show list that holds no show admits the pill — it is
    /// still landing, and the list is what would name the pill stale.
    fn gui_podcast_effective_pill(&self, lib_idx: usize) -> GuiPodcastPill {
        let pill = self.gui_podcast_pill(lib_idx);
        let GuiPodcastPill::Show(show_id) = &pill else {
            return pill;
        };
        let stale = self
            .audiobookshelf_browse
            .get(lib_idx)
            .is_some_and(|state| {
                !state.shows.is_empty()
                    && !state
                        .shows
                        .iter()
                        .any(|show| &show.library_item_id == show_id)
            });
        if stale { GuiPodcastPill::Latest } else { pill }
    }

    /// The pill row (row 10.1): Latest first, then each listed show by title —
    /// the order the show list already holds — with the effective pill
    /// selected.
    fn gui_podcast_pills(
        &self,
        lib_idx: usize,
        pill: &GuiPodcastPill,
    ) -> Vec<GuiPodcastsPillOption> {
        let mut pills = vec![GuiPodcastsPillOption {
            pill: GuiPodcastPill::Latest,
            name: "Latest".to_string(),
            selected: matches!(pill, GuiPodcastPill::Latest),
        }];
        let shows = self
            .audiobookshelf_browse
            .get(lib_idx)
            .map(|state| state.shows.as_slice())
            .unwrap_or_default();
        pills.extend(shows.iter().map(|show| GuiPodcastsPillOption {
            pill: GuiPodcastPill::Show(show.library_item_id.clone()),
            name: show.title.clone(),
            selected: matches!(pill, GuiPodcastPill::Show(id) if id == &show.library_item_id),
        }));
        pills
    }

    /// The active pill's episode rows with the view's loading and failure
    /// states (row 10.1): Latest merges every listed show's cached episodes,
    /// a show pill takes that show's, and both orders are the TUI's date order
    /// — newest first, undated last. The loading state is the catalog's pages
    /// in flight, plus the active view's own episode fetches, the TUI's
    /// `pill_fetch_in_flight` rule.
    fn gui_podcast_rows(
        &self,
        lib_idx: usize,
        pill: &GuiPodcastPill,
    ) -> (Vec<GuiPodcastsRow>, bool, bool) {
        let Some(state) = self.audiobookshelf_browse.get(lib_idx) else {
            return (Vec::new(), true, false);
        };
        let episodes = match pill {
            GuiPodcastPill::Latest => state.visible_episodes(AudiobookshelfEpisodeFilter::All),
            GuiPodcastPill::Show(show_id) => state
                .detail_cache
                .get(show_id)
                .map(|episodes| {
                    state.visible_episodes_from(episodes, AudiobookshelfEpisodeFilter::All)
                })
                .unwrap_or_default(),
        };
        let fetching = match pill {
            GuiPodcastPill::Latest => !state.detail_loading_ids.is_empty(),
            GuiPodcastPill::Show(show_id) => state.detail_loading_ids.contains_key(show_id),
        };
        let rows = episodes
            .iter()
            .map(|episode| podcast_row(state, episode))
            .collect();
        (
            rows,
            fetching || !state.loading_pages.is_empty(),
            state.error.is_some(),
        )
    }

    /// The episode the Hero shows (row 10.1): the core's pick while the active
    /// list still carries it, else that list's first row — the Hero default.
    fn gui_podcast_selected(&self, lib_idx: usize, rows: &[GuiPodcastsRow]) -> Option<String> {
        let picked = self
            .gui_podcast_picked_episode(lib_idx)
            .map(GuiPodcastEpisodeId::from)
            .map(|episode| episode.encode());
        picked
            .filter(|picked| rows.iter().any(|row| &row.id == picked))
            .or_else(|| rows.first().map(|row| row.id.clone()))
    }

    /// The selected episode's Hero (row 10.1): the shared
    /// `hero_content_abs_episode` producer's facts and description, and the
    /// parent show's cover fetched through the Service-scoped cover chain —
    /// the same fetch the TUI's hero projection issues.
    fn gui_podcast_hero(
        &mut self,
        lib_idx: usize,
        episode: &GuiPodcastEpisodeId,
    ) -> Option<GuiPodcastsHero> {
        let item = self.gui_podcast_hero_item(lib_idx, episode)?;
        let content = hero_content_abs_episode(&item);
        let art = match &content.facts.artwork.source {
            Some(ArtworkSource::AudiobookshelfCover {
                library_item_id,
                book: false,
            }) => self.gui_podcast_episode_artwork(library_item_id),
            Some(_) | None => None,
        };
        Some(GuiPodcastsHero {
            title: content.facts.title.clone(),
            meta_rows: content.facts.meta_rows.clone(),
            duration_row: content.facts.duration_row,
            progress_row: content.facts.progress_row,
            overview: content.overview.clone(),
            art_shape: artwork_shape(content.facts.artwork.shape),
            art,
        })
    }

    /// The episode as the shared hero producer's input (row 10.1): the cached
    /// episode over its parent show's identity, title, and cover — the same
    /// catalog the TUI's podcast owner builds for its own hero
    /// (`PodcastContent::selected_episode_item`), the description included.
    /// `None` when the per-show cache no longer carries the episode.
    fn gui_podcast_hero_item(
        &self,
        lib_idx: usize,
        episode: &GuiPodcastEpisodeId,
    ) -> Option<AudiobookshelfQueueItem> {
        let state = self.audiobookshelf_browse.get(lib_idx)?;
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
    fn gui_podcast_episode_artwork(&mut self, show_id: &str) -> Option<GuiLibraryArtwork> {
        let cache_key = self.audiobookshelf_cover_key(show_id)?;
        Some(self.resolve_gui_home_art(&cache_key))
    }
}

/// One row of the active list (row 10.1): the TUI podcast row's facts —
/// the episode title, the parent show's title, and the publish date in the
/// right-aligned gutter — with the row's played state.
fn podcast_row(
    state: &AudiobookshelfBrowseState,
    episode: &AudiobookshelfDownloadedEpisode,
) -> GuiPodcastsRow {
    let show = show_of(state, &episode.library_item_id);
    let progress = state
        .progress
        .get(&(episode.library_item_id.clone(), episode.episode_id.clone()));
    GuiPodcastsRow {
        id: GuiPodcastEpisodeId::new(episode.library_item_id.clone(), episode.episode_id.clone())
            .encode(),
        title: episode.title.clone(),
        subtitle: show.map(|show| show.title.clone()),
        date: episode
            .published_at
            .map(fmt_publish_date_short)
            .filter(|date| !date.is_empty()),
        state: episode_row_state(progress, episode.duration_seconds),
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

/// One row's played state (row 10.1): the TUI podcast row's rule — a finished
/// episode is `Played`, an unfinished one with a resume position is `Active`
/// with the percent of its duration (no percent when the duration is unknown),
/// and every other episode is `Ordinary`.
fn episode_row_state(
    progress: Option<&AudiobookshelfProgress>,
    duration_seconds: Option<f64>,
) -> GuiHomeRowState {
    match progress {
        Some(progress) if progress.is_finished => GuiHomeRowState::Played,
        Some(progress) if progress.current_time_seconds > 0.0 => GuiHomeRowState::Active {
            progress: resume_percent(progress.current_time_seconds, duration_seconds),
        },
        Some(_) | None => GuiHomeRowState::Ordinary,
    }
}

/// The bounded resume percent of a partially played episode, the TUI row's
/// computation: the played fraction of the duration, clamped to `0..=100`, and
/// `None` when the duration is unknown or not positive.
fn resume_percent(current_time_seconds: f64, duration_seconds: Option<f64>) -> Option<u8> {
    let duration = duration_seconds.filter(|duration| *duration > 0.0)?;
    let percent = (current_time_seconds * 100.0 / duration).clamp(0.0, 100.0);
    u8::try_from(saturating_i64_from_f64(percent).clamp(0, 100)).ok()
}
