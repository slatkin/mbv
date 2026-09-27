//! The Audiobookshelf Podcasts destination's embedded content owner
//! (reorganize-podcast-pill-navigation 3.1/3.2, design D1/D3/D4). A plain
//! type mirroring `feeds_content.rs` one column over: one episode
//! [`MediaListCarrier`] with grouped heading rows, one state-and-show pill
//! [`SelectorRow`], a Workspace-free hero for the selected episode, and the
//! tab's typed episode intents. No show browser, no show hero Workspace, no
//! selection modal, and no inline detail — every one is dead under the new
//! pill bar.

use mbv_core::api::{saturating_i64_from_f64, ticks_to_seconds, TICKS_PER_SECOND_F64};
use mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode;
use mbv_core::config::{
    AudiobookshelfPodcastFilter, AudiobookshelfSelectorKey, LibraryItemIdentity, SelectorIdentity,
};
use mbv_core::playback_queue::{AudiobookshelfQueueItem, QueueItem};

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
};
use super::library_panel::hero::hero_content_abs_episode;
use super::library_panel::owner::{LaunchSelector, LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaListTitleReveal, MediaListTrailing, MediaSemanticState,
};
use super::msg::{
    Msg, PodcastEpisodeIntent, PodcastEpisodeTarget, ShellRequest, TerminalObserverEvent,
};
use crate::app::render::current_time_secs;
use crate::app::state::types::audiobookshelf_browse::{
    podcast_display_rows, AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter, PillSelection,
    PodcastDisplayRow,
};
use crate::app::ui_util::{fmt_publish_date_short, trunc_str};

/// Shared max pill label length (`feeds_content.rs`): this owner is the one
/// producer of the Selector row's labels, and show pills truncate like the
/// Feeds tab's group labels (design D1: same selector contract).
const MAX_GROUP_LABEL: usize = super::feeds_content::MAX_GROUP_LABEL;

/// The number of state pills between Latest and the show pills in the painted
/// bar (selector indices account for the leading Latest pill).
const STATE_PILL_COUNT: usize = AudiobookshelfEpisodeFilter::ALL.len();

/// Plain owner for one Audiobookshelf podcast library. Content is projected
/// by the shell; the pill selection and the list selection remain local
/// interaction state.
pub(in crate::app) struct PodcastContent {
    pub(in crate::app) state: AudiobookshelfBrowseState,
    latest_items: Vec<AudiobookshelfQueueItem>,
    latest_marker: bool,
    /// The active pill, stored by value and remembered across tab switches
    /// (design D3); reset to `All` on construction. The painted active
    /// index is derived from this value every frame — never stored.
    pill: PillSelection,
    initialized: bool,
    focused: bool,
    /// The one shared canonical owner of the active pill's grouped-episode
    /// projection. Targets are the stable `(library_item_id, episode_id)`
    /// identities, so heading insertion and page arrivals cannot shift
    /// targeting.
    episodes: MediaListCarrier<PodcastEpisodeTarget>,
    /// The projection's image state for the current hero: set by the shell,
    /// read by the painters through the panel content.
    hero_image: HeroImageState,
    /// The Wide hero's overview scroll offset (paint-local presentation
    /// state): the panel turns it from the `HeroPane` wheel and the hero
    /// header paints from it. `hero_scroll_target` remembers the episode it
    /// was measured on, so a different selected episode starts at the top of
    /// its description instead of inheriting the previous one's offset.
    hero_scroll: usize,
    hero_scroll_target: Option<PodcastEpisodeTarget>,
    /// Injectable grouping clock (`None` = wall clock): the injected
    /// `now_secs` seam `podcast_display_rows` takes, so tests keep the
    /// age-group projection deterministic.
    now_secs: Option<u64>,
}

impl PodcastContent {
    pub(in crate::app) fn new() -> Self {
        Self {
            state: AudiobookshelfBrowseState::new(
                mbv_core::audiobookshelf::AudiobookshelfLibrary {
                    id: String::new(),
                    name: String::new(),
                    media_type: "podcast".into(),
                },
            ),
            latest_items: Vec::new(),
            latest_marker: false,
            // The remembered pill starts at `All` and survives tab switches
            // for the session (design D3; mbv restart resets it because the
            // owner is reconstructed).
            pill: PillSelection::State(AudiobookshelfEpisodeFilter::All),
            initialized: false,
            focused: false,
            episodes: {
                let mut episodes = MediaListCarrier::new();
                // The episode browser reads as its parent podcasts at rest
                // and reveals the episode title on the row the user selected
                // (design D1: the destination declares the policy once; the
                // shared row painter applies it).
                episodes.set_title_reveal(MediaListTitleReveal::OnSelection);
                episodes
            },
            hero_image: HeroImageState::None,
            hero_scroll: 0,
            hero_scroll_target: None,
            now_secs: None,
        }
    }

    /// Replace the shell-owned snapshot while preserving the pill and list
    /// selections (an ordinary refresh keeps the active view authoritative;
    /// `MediaList::set_content` preserves a still-present selected target).
    pub(in crate::app) fn set_content(
        &mut self,
        snapshot: &AudiobookshelfBrowseState,
        _images_enabled: bool,
    ) {
        self.state = snapshot.clone();
        // A refresh that drops the selected show's pill resets the selection
        // to the `All` state pill: the value's show identity no longer
        // exists, and exactly one pill must stay active (design D3).
        let pill_reset = match &self.pill {
            PillSelection::Show(id) => !self.show_exists(id),
            PillSelection::State(_) | PillSelection::Latest => false,
        };
        if pill_reset {
            self.pill = PillSelection::State(AudiobookshelfEpisodeFilter::All);
        }
        self.rebuild_rows();
        if !self.initialized || pill_reset {
            // The active pill's view starts on its first episode; a saved
            // show position is never an episode target and is ignored.
            self.episodes.select_first();
            self.initialized = true;
        }
        self.sync_hero_scroll();
    }

    fn on_latest(&self) -> bool {
        self.pill == PillSelection::Latest
    }

    pub(in crate::app) fn latest_selected(&self) -> bool {
        self.on_latest()
    }

    pub(in crate::app) fn set_latest_marker(&mut self, marker: bool) {
        self.latest_marker = marker;
    }

    pub(in crate::app) fn set_latest_items(&mut self, latest: &[QueueItem]) {
        self.latest_items = latest
            .iter()
            .filter_map(|item| match item {
                QueueItem::Audiobookshelf(
                    mbv_core::playback_queue::AudiobookshelfItem::Episode(item),
                ) => Some(item.clone()),
                _ => None,
            })
            .collect();
        self.rebuild_rows();
    }

    fn show_exists(&self, id: &str) -> bool {
        self.state
            .shows
            .iter()
            .any(|show| show.library_item_id == id)
    }

    /// The active pill's scoped episode view: a show pill ignores play
    /// state; a state pill filters every fetched show's episodes (spec:
    /// state and show selections never combine).
    fn active_episodes(&self) -> Vec<AudiobookshelfDownloadedEpisode> {
        if self.on_latest() {
            return self
                .latest_items
                .iter()
                .map(|item| AudiobookshelfDownloadedEpisode {
                    library_item_id: item.library_item_id.clone(),
                    episode_id: item.episode_id.clone(),
                    title: item.title.clone(),
                    description: item.description.clone(),
                    published_at: item.pub_date_secs,
                    duration_seconds: item
                        .duration_ticks
                        .map(|ticks| ticks_to_seconds(i64::try_from(ticks).unwrap_or(i64::MAX))),
                })
                .collect();
        }
        match &self.pill {
            PillSelection::Latest => unreachable!(),
            PillSelection::State(filter) => self
                .state
                .visible_episodes(*filter)
                .into_iter()
                .cloned()
                .collect(),
            PillSelection::Show(id) => self.state.detail_cache.get(id).cloned().unwrap_or_default(),
        }
    }

    /// Whether the active pill's episodes are still being fetched. The lazy
    /// fetch policy itself is row 3.3; this projects the in-flight marks.
    fn pill_fetch_in_flight(&self) -> bool {
        match &self.pill {
            PillSelection::Latest => false,
            PillSelection::State(_) => !self.state.detail_loading_ids.is_empty(),
            PillSelection::Show(id) => self.state.detail_loading_ids.contains_key(id),
        }
    }

    /// Project the active pill's grouped episode view into the canonical row
    /// vocabulary (design D8: grouping is the shared `podcast_display_rows`
    /// builder; indices are unchanged and targeting is the stable episode
    /// identity). Every row is a split row naming its parent podcast.
    fn rebuild_rows(&mut self) {
        let now = self.now_secs.unwrap_or_else(current_time_secs);
        let view = self.active_episodes();
        let state = &self.state;
        let rows: Vec<MediaListRow<PodcastEpisodeTarget>> = podcast_display_rows(&view, now)
            .into_iter()
            .map(|row| match row {
                PodcastDisplayRow::Spacer => MediaListRow::Spacer,
                PodcastDisplayRow::Heading(group) => MediaListRow::Heading {
                    text: group.label().to_string(),
                },
                PodcastDisplayRow::Entry(index) => {
                    let episode = &view[index];
                    let progress = state
                        .progress
                        .get(&(episode.library_item_id.clone(), episode.episode_id.clone()));
                    MediaListRow::Item {
                        target: PodcastEpisodeTarget::new(
                            episode.library_item_id.clone(),
                            episode.episode_id.clone(),
                        ),
                        primary: state
                            .shows
                            .iter()
                            .find(|show| show.library_item_id == episode.library_item_id)
                            .map(|show| show.title.clone())
                            .or_else(|| {
                                self.latest_items
                                    .iter()
                                    .find(|item| item.library_item_id == episode.library_item_id)
                                    .and_then(|item| item.show_title.clone())
                            })
                            .unwrap_or_default(),
                        secondary: Some(episode.title.clone()),
                        // The episode's publish date in the row's fixed
                        // right-aligned gutter (`17 Sep`): the one thing
                        // that distinguishes two episodes of a show once the
                        // title is revealed only on the selected row.
                        trailing: episode
                            .published_at
                            .map(fmt_publish_date_short)
                            .filter(|date| !date.is_empty())
                            .map(MediaListTrailing::Gutter),
                        // Library lists carry no time column (only the Queue
                        // list and the sessions modal show one).
                        duration: None,
                        kind: MediaKind::Media,
                        semantic_state: match progress {
                            Some(progress) if progress.is_finished => MediaSemanticState::Played,
                            Some(progress) if progress.current_time_seconds > 0.0 => {
                                #[expect(
                                    clippy::cast_possible_truncation,
                                    clippy::cast_sign_loss,
                                    reason = "progress percentage through f64; no lossless integer-path conversion exists (approved, issue #804)"
                                )]
                                let percent = episode
                                    .duration_seconds
                                    .filter(|duration| *duration > 0.0)
                                    .map(|duration| {
                                        (progress.current_time_seconds * 100.0 / duration)
                                            .clamp(0.0, 100.0)
                                            as u16
                                    });
                                MediaSemanticState::active(percent)
                            }
                            _ => MediaSemanticState::Ordinary,
                        },
                    }
                }
            })
            .collect();
        // Ordinary refresh: an unchanged projection preserves the shared
        // owner's painted frame instead of re-issuing it (the shell pushes
        // the same snapshot every sync, and re-setting it would invalidate
        // the painted frame).
        if self.episodes.rows() != rows.as_slice() {
            self.episodes.set_content(rows);
        }
    }

    /// The active pill's painted index, derived from the stored value each
    /// frame (design D3: never a stored position — the show list grows and
    /// re-sorts as pages land, so an index would silently rebind the view).
    fn active_pill_index(&self) -> Option<usize> {
        match &self.pill {
            PillSelection::Latest => Some(0),
            PillSelection::State(filter) => Some(
                1 + AudiobookshelfEpisodeFilter::ALL
                    .iter()
                    .position(|candidate| candidate == filter)
                    .unwrap_or(0),
            ),
            PillSelection::Show(id) => self
                .state
                .shows
                .iter()
                .position(|show| &show.library_item_id == id)
                .map(|position| 1 + STATE_PILL_COUNT + position),
        }
    }

    /// Commit a pill selection by value and re-anchor the list at its first
    /// row (a discrete pill change is a new view; design D3).
    fn set_pill(&mut self, pill: PillSelection) {
        if self.pill == pill {
            return;
        }
        self.pill = pill;
        self.rebuild_rows();
        self.episodes.select_first();
        self.sync_hero_scroll();
    }

    /// Offer one operation to the shared list owner, then re-anchor the
    /// hero's overview scroll (selection movement is owner-local here).
    fn delegate_episodes(&mut self, operation: MediaListOperation<PodcastEpisodeTarget>) {
        self.episodes.delegate_operation(operation);
        self.sync_hero_scroll();
    }

    /// Drop the retained hero overview scroll when the selected episode
    /// changed since the offset was measured. The hero describes exactly one
    /// episode, so a different selection starts at the top of its
    /// description; every path that can move the selection calls this.
    fn sync_hero_scroll(&mut self) {
        let selected = self.episodes.selected_target().cloned();
        if self.hero_scroll_target != selected {
            self.hero_scroll = 0;
            self.hero_scroll_target = selected;
        }
    }

    /// `[`/`]`: one uniform walk of the whole bar — state pills then show
    /// pills in painted order, wrapping at either end (design D4). There is
    /// no per-kind key or behaviour: the bar is one selector. A committed
    /// step resolves the value and sends the same effect the pointer pick
    /// sends (one path, both inputs).
    fn cycle_pill(&mut self, delta: i64) -> Option<Msg> {
        let count = 1 + STATE_PILL_COUNT + self.state.shows.len();
        let current = self.active_pill_index().unwrap_or(0);
        let next = if delta < 0 {
            if current == 0 {
                count - 1
            } else {
                current - 1
            }
        } else {
            (current + 1) % count
        };
        let pill = if next == 0 {
            PillSelection::Latest
        } else if next <= STATE_PILL_COUNT {
            PillSelection::State(AudiobookshelfEpisodeFilter::ALL[next - 1])
        } else {
            PillSelection::Show(
                self.state.shows[next - 1 - STATE_PILL_COUNT]
                    .library_item_id
                    .clone(),
            )
        };
        let changed = self.pill != pill;
        self.set_pill(pill);
        self.pill_effect_msg(changed)
    }

    /// The shell effect a committed pill keeps alive: a show pill carries
    /// its resolved identity — the per-show episode fetch (design D5's
    /// once-per-session scope), position persistence, and re-projection. A
    /// state pill sends the same message shape a plain row interaction
    /// sends — click-to-focus, position persistence, and re-projection with
    /// no show identity — and the shell's handler scopes the fan-out to
    /// every listed show.
    fn pill_effect_msg(&self, changed: bool) -> Option<Msg> {
        if self.on_latest() {
            return Some(Msg::Shell(Box::new(
                ShellRequest::AudiobookshelfPodcastLatestSelected,
            )));
        }
        if !changed {
            return None;
        }
        let library_item_id = match &self.pill {
            PillSelection::Show(library_item_id) => Some(library_item_id.clone()),
            PillSelection::State(_) => None,
            PillSelection::Latest => unreachable!(),
        };
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfPodcastShowMove { library_item_id },
        )))
    }

    /// Keyboard list movement resolves like a row click: the landed cursor
    /// persists and re-projects through the same request the pointer path
    /// sends (click-to-focus + saved position), scoped to the active pill —
    /// a show pill's identity rides along so the shell keeps its fan-out
    /// scope (design D5).
    fn move_effect(&self) -> Option<Msg> {
        if self.on_latest() {
            // Latest movement is entirely component-local: the shelf row
            // target is captured by `launch_snapshot`, while cursor/hero
            // updates need no show-scope fetch, saved browse position, or
            // shell re-projection.
            return None;
        }
        Some(Msg::Shell(Box::new(
            ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: match &self.pill {
                    PillSelection::Show(id) => Some(id.clone()),
                    PillSelection::State(_) => None,
                    PillSelection::Latest => unreachable!(),
                },
            },
        )))
    }

    pub(in crate::app) fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
    #[cfg(test)]
    pub(in crate::app) fn set_now_secs(&mut self, now_secs: u64) {
        self.now_secs = Some(now_secs);
        self.rebuild_rows();
    }

    #[cfg(test)]
    pub(in crate::app) fn pill(&self) -> &PillSelection {
        &self.pill
    }

    #[cfg(test)]
    pub(in crate::app) fn selected_episode_target(&self) -> Option<PodcastEpisodeTarget> {
        self.episodes.selected_target().cloned()
    }

    /// The selected episode as the existing hero producer's input: the
    /// downloaded episode over its parent show's identity (title, author,
    /// cover).
    fn selected_episode_item(&self) -> Option<AudiobookshelfQueueItem> {
        let target = self.episodes.selected_target()?;
        if self.on_latest() {
            return self
                .latest_items
                .iter()
                .find(|item| {
                    item.library_item_id == target.library_item_id()
                        && item.episode_id == target.episode_id()
                })
                .cloned();
        }
        let episode = self
            .state
            .episode_by_identity(target.library_item_id(), target.episode_id())?;
        let show = self
            .state
            .shows
            .iter()
            .find(|show| show.library_item_id == episode.library_item_id);
        Some(AudiobookshelfQueueItem {
            library_item_id: episode.library_item_id.clone(),
            episode_id: episode.episode_id.clone(),
            title: episode.title.clone(),
            show_title: show.map(|show| show.title.clone()),
            author: show.and_then(|show| show.author.clone()),
            description: episode.description.clone(),
            duration_ticks: episode.duration_seconds.map(|seconds| {
                let ticks = saturating_i64_from_f64((seconds * TICKS_PER_SECOND_F64).trunc());
                u64::try_from(ticks).unwrap_or(0)
            }),
            position_ticks: 0,
            played: false,
            pub_date_secs: episode.published_at,
            is_finished: false,
            cover_path: show.and_then(|show| show.cover_path.clone()),
        })
    }

    pub(in crate::app) fn hero_data(&mut self) -> Option<HeroContentData> {
        let mut data = hero_content_abs_episode(&self.selected_episode_item()?);
        data.facts.artwork.image = self.hero_image.clone();
        Some(data)
    }

    pub(in crate::app) fn set_hero_image(&mut self, image: HeroImageState) {
        self.hero_image = image;
    }

    pub(in crate::app) fn content(&mut self) -> LibraryPanelContent<'_> {
        // Hero first: it only reads the projected snapshot, while the list
        // slot borrows the shared carrier mutably for the rest of the frame.
        let hero = self.selected_episode_item().map(|item| {
            let data = hero_content_abs_episode(&item);
            let mut facts = data.facts;
            facts.artwork.image = self.hero_image.clone();
            HeroContent {
                facts,
                overview: data.overview,
                credits: data.credits,
                // No Workspace (design D1): the hero focuses episode
                // information only.
                workspace: None,
            }
        });
        let has_shows = !self.state.shows.is_empty();
        // One Selector bar carries Latest, state pills, then one pill per
        // show in show order (design D3). Both selections are owner-local.
        let selector = Some(SelectorRow {
            pills: std::iter::once("Latest".to_string())
                .chain(
                    AudiobookshelfEpisodeFilter::ALL
                        .iter()
                        .map(|filter| filter.label().to_string()),
                )
                .chain(
                    self.state
                        .shows
                        .iter()
                        .map(|show| trunc_str(&show.title, MAX_GROUP_LABEL)),
                )
                .collect(),
            markers: super::selector_markers(
                1 + STATE_PILL_COUNT + self.state.shows.len(),
                self.latest_marker,
            ),
            active: self.active_pill_index(),
        });
        let list = if !has_shows && (!self.on_latest() || self.latest_items.is_empty()) {
            ListSlot::Empty {
                loading: !self.state.loading_pages.is_empty(),
                text: self
                    .state
                    .error
                    .clone()
                    .unwrap_or_else(|| "No podcasts".into()),
            }
        } else if self.episodes.rows().is_empty() {
            ListSlot::Empty {
                loading: self.pill_fetch_in_flight(),
                text: " No episodes".into(),
            }
        } else {
            ListSlot::Media(&mut self.episodes)
        };
        LibraryPanelContent {
            selector,
            list,
            hero,
        }
    }
}

impl Default for PodcastContent {
    fn default() -> Self {
        Self::new()
    }
}

mod panel;

#[cfg(test)]
mod tests;
