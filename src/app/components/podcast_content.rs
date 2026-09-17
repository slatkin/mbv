//! The Audiobookshelf Podcasts destination's embedded content owner
//! (reorganize-podcast-pill-navigation 3.1/3.2, design D1/D3/D4). A plain
//! type mirroring `feeds_content.rs` one column over: one episode
//! [`MediaListCarrier`] with grouped heading rows, one state-and-show pill
//! [`SelectorRow`], a Workspace-free hero for the selected episode, and the
//! tab's typed episode intents. No show browser, no show hero Workspace, no
//! selection modal, and no inline detail — every one is dead under the new
//! pill bar.

use tuirealm::event::{Key, KeyEvent, KeyModifiers};

use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::audiobookshelf::AudiobookshelfDownloadedEpisode;
use mbv_core::playback_queue::AudiobookshelfQueueItem;

use super::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
};
use super::library_panel::hero::hero_content_abs_episode;
use super::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use super::library_panel::HeroContentData;
use super::media_list::{
    MediaKind, MediaListCarrier, MediaListOperation, MediaListRow, MediaListSurfaceInput,
    MediaSemanticState,
};
use super::msg::{
    Msg, PodcastEpisodeIntent, PodcastEpisodeTarget, ShellRequest, TerminalObserverEvent,
};
use crate::app::render::current_time_secs;
use crate::app::types_audiobookshelf_browse::{
    podcast_display_rows, AudiobookshelfBrowseState, AudiobookshelfEpisodeFilter, PillSelection,
    PodcastDisplayRow,
};
use crate::app::ui_util::{list_duration_secs, trunc_str};

/// Shared max pill label length (`feeds_content.rs`): this owner is the one
/// producer of the Selector row's labels, and show pills truncate like the
/// Feeds tab's group labels (design D1: same selector contract).
const MAX_GROUP_LABEL: usize = super::feeds_content::MAX_GROUP_LABEL;

/// The number of state pills that precede the show pills in the painted
/// bar (design D3: `SelectorPicked` branches on exactly this count, as
/// Feeds branches on `WatchedFilter::COUNT`).
const STATE_PILL_COUNT: usize = AudiobookshelfEpisodeFilter::ALL.len();

/// Plain owner for one Audiobookshelf podcast library. Content is projected
/// by the shell; the pill selection and the list selection remain local
/// interaction state.
pub(in crate::app) struct PodcastContent {
    pub(in crate::app) state: AudiobookshelfBrowseState,
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
            // The remembered pill starts at `All` and survives tab switches
            // for the session (design D3; mbv restart resets it because the
            // owner is reconstructed).
            pill: PillSelection::State(AudiobookshelfEpisodeFilter::All),
            initialized: false,
            focused: false,
            episodes: MediaListCarrier::new(),
            hero_image: HeroImageState::None,
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
            PillSelection::Show(id) => !self
                .state
                .shows
                .iter()
                .any(|show| &show.library_item_id == id),
            PillSelection::State(_) => false,
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
    }

    /// The active pill's scoped episode view: a show pill ignores play
    /// state; a state pill filters every fetched show's episodes (spec:
    /// state and show selections never combine).
    fn active_episodes(&self) -> Vec<AudiobookshelfDownloadedEpisode> {
        match &self.pill {
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
            PillSelection::State(_) => !self.state.detail_loading_ids.is_empty(),
            PillSelection::Show(id) => self.state.detail_loading_ids.contains(id),
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
                            .unwrap_or_default(),
                        secondary: Some(episode.title.clone()),
                        trailing: None,
                        duration: episode
                            .duration_seconds
                            .and_then(|seconds| list_duration_secs(seconds.round() as i64)),
                        kind: MediaKind::Media,
                        semantic_state: match progress {
                            Some(progress) if progress.is_finished => MediaSemanticState::Played,
                            Some(progress) if progress.current_time_seconds > 0.0 => {
                                let percent = episode
                                    .duration_seconds
                                    .filter(|duration| *duration > 0.0)
                                    .map(|duration| {
                                        ((progress.current_time_seconds * 100.0 / duration) as u16)
                                            .min(100)
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
        if self.state.shows.is_empty() {
            return None;
        }
        match &self.pill {
            PillSelection::State(filter) => Some(
                AudiobookshelfEpisodeFilter::ALL
                    .iter()
                    .position(|candidate| candidate == filter)
                    .unwrap_or(0),
            ),
            PillSelection::Show(id) => self
                .state
                .shows
                .iter()
                .position(|show| &show.library_item_id == id)
                .map(|position| STATE_PILL_COUNT + position),
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
    }

    /// `[`/`]`: one uniform walk of the whole bar — state pills then show
    /// pills in painted order, wrapping at either end (design D4). There is
    /// no per-kind key or behaviour: the bar is one selector. A committed
    /// step resolves the value and sends the same effect the pointer pick
    /// sends (one path, both inputs).
    fn cycle_pill(&mut self, delta: i64) -> Option<Msg> {
        let count = STATE_PILL_COUNT + self.state.shows.len();
        let next = (self.active_pill_index().unwrap_or(0) as i64 + delta).rem_euclid(count as i64)
            as usize;
        let pill = if next < STATE_PILL_COUNT {
            PillSelection::State(AudiobookshelfEpisodeFilter::ALL[next])
        } else {
            PillSelection::Show(
                self.state.shows[next - STATE_PILL_COUNT]
                    .library_item_id
                    .clone(),
            )
        };
        let changed = self.pill != pill;
        self.set_pill(pill);
        self.pill_effect_msg(changed)
    }

    /// The shell effect every committed pill keeps alive until the loading
    /// unit (row 3.3) reshapes the fetch triggers. A show pill carries its
    /// resolved identity: the per-show episode fetch, position persistence,
    /// and re-projection. A state pill sends the same message shape a plain
    /// row click sends — click-to-focus, position persistence, and
    /// re-projection with no show identity — the state-pill fan-out
    /// triggers themselves are row 3.3.
    fn pill_effect_msg(&self, changed: bool) -> Option<Msg> {
        if !changed {
            return None;
        }
        let library_item_id = match &self.pill {
            PillSelection::Show(library_item_id) => Some(library_item_id.clone()),
            PillSelection::State(_) => None,
        };
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id,
        }))
    }

    /// Keyboard list movement resolves like a row click: the landed cursor
    /// persists and re-projects through the same request the pointer path
    /// sends (click-to-focus + saved position, no show identity).
    fn move_effect() -> Option<Msg> {
        Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
            library_item_id: None,
        }))
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

    #[cfg(test)]
    pub(in crate::app) fn episode_rows(&self) -> &[MediaListRow<PodcastEpisodeTarget>] {
        self.episodes.rows()
    }

    /// The selected episode as the existing hero producer's input: the
    /// downloaded episode over its parent show's identity (title, author,
    /// cover). The hero facts themselves are corrected by row 3.5.
    fn selected_episode_item(&self) -> Option<AudiobookshelfQueueItem> {
        let target = self.episodes.selected_target()?;
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
            duration_ticks: episode
                .duration_seconds
                .map(|seconds| (seconds * TICKS_PER_SECOND as f64) as u64),
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
        // One Selector bar carries the state pills first, then one pill per
        // show in show order (design D3). Both selections are owner-local.
        let selector = has_shows.then(|| SelectorRow {
            pills: AudiobookshelfEpisodeFilter::ALL
                .iter()
                .map(|filter| filter.label().to_string())
                .chain(
                    self.state
                        .shows
                        .iter()
                        .map(|show| trunc_str(&show.title, MAX_GROUP_LABEL)),
                )
                .collect(),
            active: self.active_pill_index(),
        });
        let list = if !has_shows {
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
            controls: None,
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

impl LibraryContentOwner for PodcastContent {
    fn clear_selection(&mut self) {
        self.episodes.clear_selection();
    }

    fn set_selection_origin(
        &mut self,
        origin: crate::app::components::media_list::SelectionOrigin,
    ) {
        self.episodes.set_selection_origin(origin);
    }

    fn selection_summary(&self) -> Option<crate::app::components::media_list::SelectionSummary> {
        Some(self.episodes.selection_summary())
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        self.content()
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        match event {
            LibrarySlotEvent::SelectorPicked(index) => {
                // One selector resolved in the owner (design D3): state
                // pills first, then the show pills in painted order.
                let pill = if let Some(filter) = AudiobookshelfEpisodeFilter::ALL.get(index) {
                    PillSelection::State(*filter)
                } else {
                    PillSelection::Show(
                        self.state
                            .shows
                            .get(index - STATE_PILL_COUNT)?
                            .library_item_id
                            .clone(),
                    )
                };
                let changed = self.pill != pill;
                self.set_pill(pill);
                let effect = self.pill_effect_msg(changed);
                // A resolved pill pick claims the pointer gesture that
                // delivered it.
                effect.or(Some(Msg::TerminalEvent(
                    TerminalObserverEvent::MouseClaimed,
                )))
            }
            LibrarySlotEvent::List(input) => match input {
                MediaListSurfaceInput::Wheel { at, delta } => {
                    // The claim gate mirrors the mounted component: a wheel
                    // outside the painted active list is unclaimed.
                    if !self.episodes.claims_current_point(at) {
                        return None;
                    }
                    self.episodes.delegate_operation(
                        MediaListSurfaceInput::Wheel { at, delta }
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    );
                    Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
                }
                MediaListSurfaceInput::Click(at)
                | MediaListSurfaceInput::ToggleClick(at)
                | MediaListSurfaceInput::RangeClick(at)
                | MediaListSurfaceInput::ContextClick(at) => {
                    let target = self.episodes.resolve_current_point(at)?.clone();
                    self.episodes.delegate_operation(
                        input
                            .into_operation(Some(target))
                            .expect("resolved media-list pointer target"),
                    );
                    // Click-to-focus (task 4.5): pull panel focus to the
                    // Library and persist the tab's slot, as the Feeds row
                    // click does.
                    Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                        library_item_id: None,
                    }))
                }
                MediaListSurfaceInput::DoubleClick(at) => {
                    // Resolve once, then delegate the target-bearing activation.
                    let target = self.episodes.resolve_current_point(at)?.clone();
                    self.episodes
                        .delegate_operation(MediaListOperation::Activate(target.clone()));
                    Some(Msg::Shell(
                        ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                            PodcastEpisodeIntent::OpenOrPlay(Some(target)),
                        ),
                    ))
                }
                _ => {
                    self.episodes.delegate_operation(
                        input
                            .into_operation(None)
                            .expect("resolved media-list pointer target"),
                    );
                    None
                }
            },
            // No Workspace and no hero-pane input of its own (design D1).
            LibrarySlotEvent::WorkspaceSelectorPicked(_)
            | LibrarySlotEvent::ControlPicked(_)
            | LibrarySlotEvent::HeroPane(_) => None,
            LibrarySlotEvent::HeroActivate => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episodes.selected_target().cloned(),
                )),
            )),
        }
    }

    fn on_key(&mut self, key: &KeyEvent) -> Option<Msg> {
        if self.episodes.handle_visual_key(key).is_some() {
            return Some(Msg::Shell(ShellRequest::SelectionProjection(
                self.episodes.selection_summary(),
            )));
        }
        if !self.focused {
            return None;
        }
        match key.code {
            Key::Up | Key::Char('k') => {
                self.episodes.delegate_operation(
                    MediaListSurfaceInput::Move(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Self::move_effect()
            }
            Key::Down | Key::Char('j') => {
                self.episodes.delegate_operation(
                    MediaListSurfaceInput::Move(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Self::move_effect()
            }
            Key::PageUp => {
                self.episodes.delegate_operation(
                    MediaListSurfaceInput::Page(-1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Self::move_effect()
            }
            Key::PageDown => {
                self.episodes.delegate_operation(
                    MediaListSurfaceInput::Page(1)
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Self::move_effect()
            }
            Key::Home => {
                self.episodes.delegate_operation(
                    MediaListSurfaceInput::First
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Self::move_effect()
            }
            Key::End => {
                self.episodes.delegate_operation(
                    MediaListSurfaceInput::Last
                        .into_operation(None)
                        .expect("resolved media-list pointer target"),
                );
                Self::move_effect()
            }
            Key::Char('[') if key.modifiers.is_empty() => self.cycle_pill(-1),
            Key::Char(']') if key.modifiers.is_empty() => self.cycle_pill(1),
            Key::Enter => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    self.episodes.selected_target().cloned(),
                )),
            )),
            Key::Char(' ') => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(
                    PodcastEpisodeIntent::FocusOrPlay(self.episodes.selected_target().cloned()),
                ),
            )),
            Key::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::Enqueue(
                    self.episodes.selected_target().cloned(),
                )),
            )),
            _ => None,
        }
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        self.hero_data()
    }
    fn set_hero_image(&mut self, image: HeroImageState) {
        self.set_hero_image(image);
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::msg::LeafKeyResult;
    use mbv_core::audiobookshelf::{
        AudiobookshelfLibrary, AudiobookshelfProgress, AudiobookshelfShow,
    };
    use ratatui::backend::TestBackend;
    use ratatui::layout::Position;
    use ratatui::Terminal;
    use tuirealm::component::Component;

    const DAY: u64 = 24 * 60 * 60;
    const NOW: u64 = 30 * DAY;

    fn library() -> AudiobookshelfLibrary {
        AudiobookshelfLibrary {
            id: "lib".into(),
            name: "Podcasts".into(),
            media_type: "podcast".into(),
        }
    }

    fn show(id: &str, title: &str) -> AudiobookshelfShow {
        AudiobookshelfShow {
            library_item_id: id.into(),
            title: title.into(),
            author: None,
            description: None,
            cover_path: None,
        }
    }

    fn episode(
        show: &str,
        id: &str,
        published_at: Option<u64>,
        duration_seconds: Option<f64>,
    ) -> AudiobookshelfDownloadedEpisode {
        AudiobookshelfDownloadedEpisode {
            library_item_id: show.into(),
            episode_id: id.into(),
            title: id.into(),
            description: None,
            published_at,
            duration_seconds,
        }
    }

    /// Two shows: Alpha's cache holds a dated played episode and an undated
    /// in-progress one; Beta's cache holds one unplayed episode.
    fn state() -> AudiobookshelfBrowseState {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(
            0,
            20,
            2,
            vec![show("alpha", "Alpha Show"), show("beta", "Beta Show")],
        );
        state.cache_detail(
            "alpha".into(),
            vec![
                episode("alpha", "dated", Some(NOW - DAY), Some(3600.0)),
                episode("alpha", "undated", None, Some(1800.0)),
            ],
        );
        state.cache_detail("beta".into(), vec![episode("beta", "beta-one", None, None)]);
        state
            .progress
            .insert(("alpha".into(), "dated".into()), finished_progress());
        state.progress.insert(
            ("alpha".into(), "undated".into()),
            AudiobookshelfProgress {
                library_item_id: "alpha".into(),
                episode_id: "undated".into(),
                current_time_seconds: 300.0,
                is_finished: false,
            },
        );
        state
    }

    fn finished_progress() -> AudiobookshelfProgress {
        AudiobookshelfProgress {
            library_item_id: "alpha".into(),
            episode_id: "dated".into(),
            current_time_seconds: 3600.0,
            is_finished: true,
        }
    }

    fn owner() -> PodcastContent {
        let mut owner = PodcastContent::new();
        owner.set_now_secs(NOW);
        owner.set_content(&state(), false);
        owner
    }

    fn item_rows(owner: &PodcastContent) -> Vec<(&str, &str)> {
        owner
            .episodes
            .rows()
            .iter()
            .filter_map(|row| match row {
                MediaListRow::Item {
                    target, secondary, ..
                } => Some((
                    target.episode_id(),
                    secondary.as_deref().unwrap_or_default(),
                )),
                _ => None,
            })
            .collect()
    }

    fn pills(content: &LibraryPanelContent) -> Vec<String> {
        content.selector.as_ref().expect("pill bar").pills.clone()
    }

    #[test]
    fn pill_selection_starts_at_all_on_construction() {
        let owner = PodcastContent::new();
        assert_eq!(
            owner.pill,
            PillSelection::State(AudiobookshelfEpisodeFilter::All)
        );
    }

    #[test]
    fn content_has_one_selector_row_for_state_and_show_pills() {
        let mut owner = owner();
        let content = owner.content();
        assert_eq!(
            pills(&content),
            ["All", "Unplayed", "Played", "Alpha Show", "Beta Show"]
        );
        assert_eq!(content.selector.unwrap().active, Some(0));
        assert!(content.controls.is_none());

        // `]` walks the whole bar uniformly, state pills included.
        owner.set_focused(true);
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        let content = owner.content();
        assert_eq!(content.selector.unwrap().active, Some(1));
        assert_eq!(
            owner.pill,
            PillSelection::State(AudiobookshelfEpisodeFilter::Unplayed)
        );
    }

    #[test]
    fn show_pills_truncate_like_feed_group_labels() {
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(
            0,
            20,
            1,
            vec![show("long", "A Very Long Podcast Show Name Indeed Indeed")],
        );
        let mut owner = PodcastContent::new();
        owner.set_now_secs(NOW);
        owner.set_content(&state, false);
        assert_eq!(
            pills(&owner.content())[3],
            trunc_str(
                "A Very Long Podcast Show Name Indeed Indeed",
                MAX_GROUP_LABEL
            )
        );
    }

    #[test]
    fn painted_active_index_is_derived_from_the_stored_value() {
        let mut owner = owner();
        owner.set_focused(true);
        // Land on the Beta show pill (painted index 4).
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        assert_eq!(owner.pill, PillSelection::Show("beta".into()));

        // A new show page that sorts before Beta shifts Beta's painted
        // position; the value keeps identifying the same pill.
        let mut state = state();
        state.append_page(1, 20, 3, vec![show("aardvark", "Aardvark Show")]);
        owner.set_content(&state, false);
        assert_eq!(owner.pill, PillSelection::Show("beta".into()));
        assert_eq!(owner.content().selector.unwrap().active, Some(5));
    }

    #[test]
    fn keyboard_pill_walk_wraps_at_both_ends() {
        let mut owner = owner();
        owner.set_focused(true);
        // `[` from `All` wraps to the last show pill.
        owner.on_key(&KeyEvent::new(Key::Char('['), KeyModifiers::NONE));
        assert_eq!(owner.pill, PillSelection::Show("beta".into()));
        // `]` wraps back to the first state pill.
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        assert_eq!(
            owner.pill,
            PillSelection::State(AudiobookshelfEpisodeFilter::All)
        );
    }

    #[test]
    fn remembered_pill_survives_an_ordinary_refresh() {
        let mut owner = owner();
        owner.set_focused(true);
        owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        owner.set_content(&state(), false);
        assert_eq!(
            owner.pill,
            PillSelection::State(AudiobookshelfEpisodeFilter::Unplayed)
        );
    }

    #[test]
    fn a_refresh_that_drops_the_show_pill_resets_to_all() {
        let mut owner = owner();
        owner.set_focused(true);
        // Walk to the Beta show pill, then refresh without it.
        for _ in 0..4 {
            owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        }
        assert_eq!(owner.pill, PillSelection::Show("beta".into()));
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(0, 20, 1, vec![show("alpha", "Alpha Show")]);
        owner.set_content(&state, false);
        assert_eq!(
            owner.pill,
            PillSelection::State(AudiobookshelfEpisodeFilter::All)
        );
        assert_eq!(owner.content().selector.unwrap().active, Some(0));
    }

    #[test]
    fn episode_rows_are_split_rows_with_played_and_in_progress_state() {
        let owner = owner();
        let rows = owner.episodes.rows().to_vec();
        let dated = rows
            .iter()
            .find_map(|row| match row {
                MediaListRow::Item { target, .. } if target.episode_id() == "dated" => {
                    Some(row.clone())
                }
                _ => None,
            })
            .expect("dated episode row");
        match dated {
            MediaListRow::Item {
                primary,
                secondary,
                duration,
                semantic_state,
                kind,
                ..
            } => {
                assert_eq!(primary, "Alpha Show", "the split row names its podcast");
                assert_eq!(secondary.as_deref(), Some("dated"));
                assert_eq!(duration.as_deref(), Some("1:00:00"));
                assert_eq!(semantic_state, MediaSemanticState::Played);
                assert_eq!(kind, MediaKind::Media);
            }
            _ => panic!("expected an episode item row"),
        }
        let in_progress = rows
            .iter()
            .find_map(|row| match row {
                MediaListRow::Item {
                    target,
                    semantic_state,
                    ..
                } if target.episode_id() == "undated" => Some(semantic_state.clone()),
                _ => None,
            })
            .expect("in-progress episode row");
        assert_eq!(
            in_progress,
            MediaSemanticState::active(Some(16)),
            "in-progress progress renders the shared in-progress badge"
        );
    }

    #[test]
    fn grouped_rows_carry_stable_episode_targets() {
        let owner = owner();
        let rows = owner.episodes.rows();
        assert!(matches!(rows.first(), Some(MediaListRow::Heading { .. })));
        assert!(matches!(rows.last(), Some(MediaListRow::Item { .. })));
        assert!(rows
            .iter()
            .any(|row| matches!(row, MediaListRow::Heading { .. })));
        // Heading insertion never changes episode targeting: the dated and
        // undated rows carry the provider-native identity pair.
        assert_eq!(
            item_rows(&owner),
            [
                ("dated", "dated"),
                ("undated", "undated"),
                ("beta-one", "beta-one")
            ],
            "the All view lists every fetched show's episodes"
        );
    }

    #[test]
    fn show_pill_scopes_the_view_to_that_show() {
        let mut owner = owner();
        owner.on_slot_event(LibrarySlotEvent::SelectorPicked(4));
        assert_eq!(
            item_rows(&owner),
            [("beta-one", "beta-one")],
            "a show pill ignores play state and excludes other shows"
        );
    }

    #[test]
    fn empty_and_loading_list_slots() {
        let mut owner = PodcastContent::new();
        let empty = AudiobookshelfBrowseState::new(library());
        owner.set_content(&empty, false);
        match owner.content().list {
            ListSlot::Empty { loading, text } => {
                assert!(!loading);
                assert_eq!(text, "No podcasts");
            }
            _ => panic!("expected the no-podcasts placeholder"),
        }

        // Shows present but the active pill's episodes not fetched yet: the
        // scoped empty state, loading while the show's fetch is in flight.
        let mut state = AudiobookshelfBrowseState::new(library());
        state.append_page(0, 20, 1, vec![show("alpha", "Alpha Show")]);
        state.detail_loading_ids.insert("alpha".into());
        owner.set_content(&state, false);
        match owner.content().list {
            ListSlot::Empty { loading, text } => {
                assert!(loading);
                assert_eq!(text, " No episodes");
            }
            _ => panic!("expected the scoped empty placeholder"),
        }
    }

    #[test]
    fn hero_has_no_workspace_and_presents_the_selected_episode() {
        let mut owner = owner();
        let content = owner.content();
        let hero = content.hero.expect("selected episode hero");
        assert!(hero.workspace.is_none(), "the hero has no Workspace");
        assert_eq!(
            hero.facts.title, "dated",
            "the hero presents the selected episode"
        );
        assert_eq!(hero.overview, None);
        assert!(matches!(content.list, ListSlot::Media(_)));
    }

    #[test]
    fn keyboard_pill_walk_sends_the_same_effect_a_pointer_pick_sends() {
        let mut owner = owner();
        owner.set_focused(true);
        // Three `]` steps land on the Alpha show pill; the walk resolves the
        // value and requests that show's fetch/persistence/re-projection.
        let mut message = None;
        for _ in 0..3 {
            message = owner.on_key(&KeyEvent::new(Key::Char(']'), KeyModifiers::NONE));
        }
        assert!(matches!(
            message,
            Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: Some(ref id)
            })) if id == "alpha"
        ));
        // A state-pill commit sends the same shape a plain row click sends.
        assert!(matches!(
            owner.on_key(&KeyEvent::new(Key::Char('['), KeyModifiers::NONE)),
            Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: None
            }))
        ));
    }

    #[test]
    fn keyboard_list_movement_persists_like_a_row_click() {
        let mut owner = owner();
        owner.set_focused(true);
        for key in [
            Key::Down,
            Key::Up,
            Key::PageDown,
            Key::PageUp,
            Key::End,
            Key::Home,
        ] {
            assert!(
                matches!(
                    owner.on_key(&KeyEvent::new(key, KeyModifiers::NONE)),
                    Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                        library_item_id: None
                    }))
                ),
                "{key:?} movement persists like a row click"
            );
        }
    }

    #[test]
    fn unfocused_owner_leaves_movement_and_unhandled_keys_to_the_router() {
        let mut owner = owner();
        assert_eq!(
            owner.on_key_result(&KeyEvent::new(Key::Down, KeyModifiers::NONE)),
            LeafKeyResult::Unhandled,
            "movement while unfocused is not swallowed"
        );
        assert_eq!(
            owner.on_key_result(&KeyEvent::new(Key::Left, KeyModifiers::NONE)),
            LeafKeyResult::Unhandled
        );
        // Focused but unhandled keys stay Unhandled too.
        owner.set_focused(true);
        assert_eq!(
            owner.on_key_result(&KeyEvent::new(Key::Left, KeyModifiers::NONE)),
            LeafKeyResult::Unhandled
        );
    }

    #[test]
    fn enter_emits_open_or_play_with_the_selected_episode_target() {
        let mut owner = owner();
        owner.set_focused(true);
        assert!(matches!(
            owner.on_key(&KeyEvent::new(Key::Enter, KeyModifiers::NONE)),
            Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    Some(target)
                ))
            )) if target.episode_id() == "dated"
        ));
    }

    #[test]
    fn show_pill_pick_resolves_the_show_identity_for_the_shell() {
        let mut owner = owner();
        assert!(matches!(
            owner.on_slot_event(LibrarySlotEvent::SelectorPicked(3)),
            Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: Some(ref id)
            })) if id == "alpha"
        ));
        // A state-pill pick re-projects and persists through the same
        // request shape a plain row click sends (no show identity); the
        // fan-out triggers themselves are row 3.3.
        assert!(matches!(
            owner.on_slot_event(LibrarySlotEvent::SelectorPicked(1)),
            Some(Msg::Shell(ShellRequest::AudiobookshelfPodcastShowMove {
                library_item_id: None
            }))
        ));
        // A pick beyond the painted bar resolves nothing.
        assert_eq!(
            owner.on_slot_event(LibrarySlotEvent::SelectorPicked(99)),
            None
        );
    }

    #[test]
    fn pointer_click_resolves_the_episode_target_and_unknown_point_is_noop() {
        let mut owner = owner();
        owner.set_focused(true);

        let area = ratatui::layout::Rect::new(0, 0, 30, 4);
        owner.episodes.wide_mut().set_geometry(area, area);
        let mut terminal = Terminal::new(TestBackend::new(30, 4)).unwrap();
        terminal
            .draw(|frame| owner.episodes.wide_mut().view(frame, area))
            .unwrap();

        assert!(matches!(
            owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::DoubleClick(
                Position { x: 0, y: 1 }
            ))),
            Some(Msg::Shell(
                ShellRequest::AudiobookshelfPodcastEpisodeIntent(PodcastEpisodeIntent::OpenOrPlay(
                    Some(target)
                ))
            )) if target.episode_id() == "dated"
        ));
        assert_eq!(
            owner.on_slot_event(LibrarySlotEvent::List(MediaListSurfaceInput::Click(
                Position { x: 0, y: 99 }
            ))),
            None
        );
    }

    #[test]
    fn hero_data_feeds_the_existing_episode_producer_for_the_shell_projection() {
        let mut owner = owner();
        let data = owner.hero_data().expect("selected episode hero data");
        assert_eq!(data.facts.title, "dated");
        assert!(matches!(
            data.facts.artwork.shape,
            super::super::library_panel::content::ArtworkShape::Square
        ));
    }
}
