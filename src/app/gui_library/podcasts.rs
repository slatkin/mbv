//! The GUI Library's Podcasts data path (add-gui-library-panel design D12,
//! row 10.1): the per-podcast-library selection the core owns — the checked
//! shows, the date order, and the picked episode — and the open, toggle,
//! All, and order actions the core's collection commands apply. The
//! selection is All or a set of checked shows (gui-collection-dropdown
//! design D1, D2), so the dropdown replaces the pill row in a later unit.
//!
//! The show catalog and the
//! per-show episodes already land App-side: the shared startup's show-list
//! chain pages into the browse state, and the bounded fan-out caches each
//! show's episodes. So this module names the checked shows and triggers the
//! loads the TUI's own loaders run. The TUI never reads any of it.

use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind;
use mbv_ui_msg::PodcastEpisodeTarget;

use super::collection::{
    GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet, GuiDateOrder,
};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::podcasts_projection::GuiPodcastEpisodeId;

use crate::app::App;

/// The GUI Library's Podcasts selection (row 10.1): per podcast library, the
/// stored collection selection — the checked shows and the list's date order
/// (gui-collection-dropdown design D1, D2, D4) — and the episode the Hero
/// shows. All is the set default and Newest the order default, so All on
/// Newest with no pick is the selection every launch starts on (the spec's
/// persistence rule — session memory only, nothing saved). All fans out to
/// every listed show, the same loads as today's Latest, and one checked show
/// scopes the fan-out to it. The set's keys are the shows' stable
/// `library_item_id`s. The episode is named by the
/// provider's stable `(library_item_id, episode_id)` identity — the identity
/// the progress map and the queue-item resolver use — because an episode id
/// alone names no row across shows. A pick the checked shows' list no longer
/// holds falls back to that list's first row in the projection, as every
/// selection's stale arm does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuiPodcastsSelection {
    /// The checked shows and the list's date order.
    pub selection: GuiCollectionSelection,
    /// The episode the Hero shows. `None` before a select command names one.
    pub picked_episode: Option<PodcastEpisodeTarget>,
}

impl Default for GuiPodcastsSelection {
    /// All on Newest with no pick: the selection every launch starts on (the
    /// spec's persistence rule — session memory only, nothing saved).
    fn default() -> Self {
        Self {
            selection: GuiCollectionSelection::default(),
            picked_episode: None,
        }
    }
}

/// The Podcasts destination's collection-command site
/// (gui-collection-dropdown design D5): the destination guard, the show-list
/// staleness rule, the show catalog, and the selection install the shared
/// collection path calls.
pub(super) struct GuiPodcastsCollectionSite(pub(super) usize);

impl GuiCollectionSite for GuiPodcastsCollectionSite {
    fn noun(&self) -> GuiCollectionNoun {
        GuiCollectionNoun::Shows
    }

    fn has_order(&self) -> bool {
        true
    }

    fn present(&self, app: &App) -> Option<Vec<String>> {
        // A show list that holds no show keeps the stored set as it is: it is
        // still landing, and the list is what would name a show stale (row
        // 10.1).
        let present = app.gui_podcast_present_shows(self.0);
        (!present.is_empty()).then_some(present)
    }

    fn scopes(&self, app: &App) -> Vec<(String, String)> {
        app.gui_podcast_shows(self.0)
    }

    fn selection(&self, app: &App) -> GuiCollectionSelection {
        app.gui_podcast_libs
            .get(&self.0)
            .map(|stored| stored.selection.clone())
            .unwrap_or_default()
    }

    fn reject_destination(&self, app: &App) -> Option<CollectionRejection> {
        if app.gui_podcast_library(self.0) {
            None
        } else {
            Some(CollectionRejection::Ignored(
                "the destination is no Podcasts library".to_string(),
            ))
        }
    }

    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection> {
        if app.gui_podcast_show_known(self.0, key) {
            None
        } else {
            Some(CollectionRejection::Stale(key.to_string()))
        }
    }

    fn install_set(&self, app: &mut App, set: GuiCollectionSet) {
        app.install_gui_podcast_set(self.0, set);
    }

    fn install_order(&self, app: &mut App, order: GuiDateOrder) {
        app.gui_podcast_libs
            .entry(self.0)
            .or_default()
            .selection
            .order = order;
    }
}

impl App {
    /// Whether `lib_idx` names a Podcasts destination (row 10.1): an
    /// Audiobookshelf library whose `media_type` resolves to the podcast
    /// browse kind. Every GUI Podcasts method's destination guard, and the
    /// mirror of the Audiobooks guard: a book library stays Audiobooks.
    pub(in crate::app::gui_library) fn gui_podcast_library(&self, lib_idx: usize) -> bool {
        self.audiobookshelf_libraries
            .get(lib_idx)
            .is_some_and(|library| {
                AudiobookshelfBrowseKind::from_media_type(&library.media_type)
                    == AudiobookshelfBrowseKind::Podcast
            })
    }

    /// Open the Podcasts destination `lib_idx` (row 10.1): an unloaded show
    /// catalog starts the TUI's own show-list load — the refresh path the F5
    /// press runs, whose replacement batch is single-flight and whose
    /// completion publishes the whole catalog — and the selection's loads
    /// start through the frame path's ensure below. A catalog already
    /// loading, loaded, or owned by the startup's own fetch chain loads
    /// nothing here, so no second chain walks beside the first. A `lib_idx`
    /// that names no podcast library is an ignored command.
    pub fn gui_podcast_open(&mut self, lib_idx: usize) {
        if !self.gui_podcast_library(lib_idx) {
            Self::log_ignored_library_command(
                "podcastOpen",
                format_args!("the destination is no Podcasts library"),
            );
            return;
        }
        if self.tab.audiobookshelf_index() != Some(lib_idx) {
            return;
        }
        let unloaded = self
            .audiobookshelf_browse
            .get(lib_idx)
            .is_some_and(|state| {
                state.shows.is_empty() && state.total == 0 && state.replacement.is_none()
            });
        if unloaded {
            // The show list only: the All shelf and the per-show episodes
            // are the selection's loads, which the ensure starts through the
            // same shared guards the shell's projection pass uses.
            self.audiobookshelf_refresh(false);
        }
        self.ensure_gui_podcast_selection(lib_idx);
    }

    /// The one write the toggle and the All press take: the shared collection
    /// path installs the change, the store records it, and the loads the
    /// selection needs start (gui-collection-dropdown design D2, D5).
    fn install_gui_podcast_set(&mut self, lib_idx: usize, set: GuiCollectionSet) {
        self.gui_podcast_libs
            .entry(lib_idx)
            .or_default()
            .selection
            .set = set;
        self.ensure_gui_podcast_selection(lib_idx);
    }

    /// Select the episode the Hero shows (row 10.1): the selection the core
    /// owns, so the frame carries it back. The episode's own fields and its
    /// show's facts are already cached, so no load starts. An episode the
    /// published per-show cache does not carry is a stale frame's pick and
    /// changes nothing, as every item-named command's stale arm does; a cache
    /// still landing admits the pick, because the list is what would name it
    /// stale.
    pub fn gui_podcast_select(&mut self, lib_idx: usize, target: &PodcastEpisodeTarget) {
        if !self.gui_podcast_library(lib_idx) {
            Self::log_ignored_library_command(
                "podcastSelect",
                format_args!("the destination is no Podcasts library"),
            );
            return;
        }
        if !self.gui_podcast_episode_known(lib_idx, target) {
            Self::log_stale_library_item("podcastSelect", target.episode_id());
            return;
        }
        self.gui_podcast_libs
            .entry(lib_idx)
            .or_default()
            .picked_episode = Some(target.clone());
    }

    /// The frame-path trigger the GUI's frame runs for the active Podcasts
    /// destination (row 10.1), the Audiobooks frame path's counterpart: the
    /// checked shows' scope is committed to the browse state the
    /// fan-out reads, and the loads the selection needs start — All's
    /// one-shot shelf fetch and every listed show's episodes, or the one
    /// checked show's episodes. Every guard here is idempotent (the shelf's request
    /// set, the fan-out's cached and in-flight skips, the scope's
    /// compare-before-write), so the frame path runs it on every frame. A
    /// `lib_idx` that names no podcast library, or a destination that is not
    /// the active one, is a no-op, so the frame path can call it unguarded.
    pub fn ensure_gui_podcast_selection(&mut self, lib_idx: usize) {
        if !self.gui_podcast_library(lib_idx) || self.tab.audiobookshelf_index() != Some(lib_idx) {
            return;
        }
        let set = self.gui_podcast_effective_set(lib_idx);
        self.commit_gui_podcast_show_scope(lib_idx, &set);
        if set.is_all()
            && let Some(library_id) = self
                .audiobookshelf_libraries
                .get(lib_idx)
                .map(|library| library.id.clone())
        {
            self.ensure_audiobookshelf_latest_shelf(&library_id);
        }
        self.start_audiobookshelf_podcast_fan_out(lib_idx);
    }

    /// Play the episode a command names (row 10.1): the TUI's ordinary episode
    /// play — the queue is replaced with the episode, whose saved position is
    /// the resume position — with the Owner's Audiobookshelf admission
    /// honoured. A refusal flashes the TUI's owner-unavailable toast; a stale
    /// identity plays nothing.
    pub(in crate::app::gui_library) fn play_gui_podcast_episode(
        &mut self,
        lib_idx: usize,
        target: &PodcastEpisodeTarget,
    ) {
        if self
            .selected_audiobookshelf_queue_item_target(lib_idx, target)
            .is_none()
        {
            Self::log_stale_library_item("podcastPlay", target.episode_id());
            return;
        }
        self.play_selected_audiobookshelf_episode_target(lib_idx, target);
    }

    /// Add the episode a command names to the queue (row 10.1): the TUI's
    /// enqueue path — one append at the tail — with the Owner's
    /// Audiobookshelf admission honoured. A refusal flashes the TUI's
    /// owner-unavailable toast; a stale identity queues nothing.
    pub(in crate::app::gui_library) fn enqueue_gui_podcast_episode(
        &mut self,
        lib_idx: usize,
        target: &PodcastEpisodeTarget,
    ) {
        let Some(item) = self.selected_audiobookshelf_queue_item_target(lib_idx, target) else {
            Self::log_stale_library_item("podcastQueue", target.episode_id());
            return;
        };
        if !self.player.can_admit_audiobookshelf() {
            self.flash_abs_owner_unavailable();
            return;
        }
        let _ = self.submit_queue_item(item, false);
    }

    /// Select the Podcasts episode a row id names (design D12, row 10.1): the
    /// frame carries the episode's identity as one opaque string, so a string
    /// that is no encoding of it is a stale frame's row and changes nothing.
    pub(in crate::app::gui_library) fn select_gui_podcast_row(
        &mut self,
        lib_idx: usize,
        row: &str,
    ) {
        let Some(episode) = GuiPodcastEpisodeId::decode(row) else {
            Self::log_stale_library_item("podcastSelect", row);
            return;
        };
        self.gui_podcast_select(lib_idx, &episode.target());
    }

    /// Play the Podcasts episode a row id names (design D12, row 10.1): Enter
    /// on the row, or the double-click a window sends with this same
    /// command. The episode's identity is the frame's opaque row id; a string
    /// that is no encoding of it changes nothing.
    pub(in crate::app::gui_library) fn play_gui_podcast_row(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        let Some(episode) = GuiPodcastEpisodeId::decode(item_id) else {
            Self::log_stale_library_item("podcastPlay", item_id);
            return;
        };
        self.play_gui_podcast_episode(lib_idx, &episode.target());
    }

    /// Add the Podcasts episode a row id names to the queue (design D12, row
    /// 10.1): `a` on the row. The episode's identity is the frame's opaque row
    /// id; a string that is no encoding of it changes nothing.
    pub(in crate::app::gui_library) fn enqueue_gui_podcast_row(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        let Some(episode) = GuiPodcastEpisodeId::decode(item_id) else {
            Self::log_stale_library_item("podcastQueue", item_id);
            return;
        };
        self.enqueue_gui_podcast_episode(lib_idx, &episode.target());
    }

    /// The effective selection the list and the pill row paint (row 10.1;
    /// gui-collection-dropdown design D1): the stored set, or All when none is
    /// stored. A checked show the published list no longer carries drops (the
    /// spec's vanish rule), and when none of the checked shows survive the
    /// selection becomes All; a show list that holds no show admits the stored
    /// set as it is — it is still landing, and the list is what would name its
    /// shows stale.
    pub(in crate::app) fn gui_podcast_effective_set(&self, lib_idx: usize) -> GuiCollectionSet {
        GuiPodcastsCollectionSite(lib_idx).effective_set(self)
    }

    /// The published show list's ids (row 10.1): the retention's present
    /// list, in the show list's own order.
    fn gui_podcast_present_shows(&self, lib_idx: usize) -> Vec<String> {
        self.audiobookshelf_browse
            .get(lib_idx)
            .map(|state| {
                state
                    .shows
                    .iter()
                    .map(|show| show.library_item_id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    /// The date order the list sorts by (row 10.1; gui-collection-dropdown
    /// design D4): the stored order, or Newest for a library the session
    /// never changed it — the launch default.
    #[must_use]
    pub fn gui_podcast_date_order(&self, lib_idx: usize) -> GuiDateOrder {
        self.gui_podcast_libs
            .get(&lib_idx)
            .map_or(GuiDateOrder::default(), |stored| stored.selection.order)
    }

    /// The episode the Hero shows (row 10.1): the stored pick. The projection
    /// falls back to the checked shows' first row when the pick left the
    /// list, so the Hero's default is never blank.
    #[must_use]
    pub fn gui_podcast_picked_episode(&self, lib_idx: usize) -> Option<&PodcastEpisodeTarget> {
        self.gui_podcast_libs.get(&lib_idx)?.picked_episode.as_ref()
    }

    /// Commit the checked shows' scope to the browse state the bounded
    /// fan-out reads (row 10.1; gui-collection-dropdown design D2): exactly
    /// one checked show scopes the fan-out to that show, and All or several
    /// checked shows need every listed show — the same loads as today's
    /// Latest. The write happens only when
    /// the stored scope differs, so the frame path's ensure does not rewrite
    /// the browse state on every frame. The scope is the one shared write both
    /// Clients make; the browse state's own selected episode and show stay the
    /// TUI's.
    fn commit_gui_podcast_show_scope(&mut self, lib_idx: usize, set: &GuiCollectionSet) {
        let scope = match set {
            GuiCollectionSet::Checked(keys) if keys.len() == 1 => keys.first().cloned(),
            _ => None,
        };
        if let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx)
            && state.committed_show_pill != scope
        {
            state.committed_show_pill = scope;
        }
    }

    /// Whether the published show list can still name `show_id` (row 10.1): a
    /// list that holds no show admits the press — it is still landing, and the
    /// list is what would name the show stale — and a loaded list admits only
    /// a show it carries.
    fn gui_podcast_show_known(&self, lib_idx: usize, show_id: &str) -> bool {
        let Some(state) = self.audiobookshelf_browse.get(lib_idx) else {
            return false;
        };
        state.shows.is_empty()
            || state
                .shows
                .iter()
                .any(|show| show.library_item_id == show_id)
    }

    /// Whether the published per-show cache can still name `target` (row
    /// 10.1): a cache that holds no episode admits the pick, and a landed
    /// cache admits only an episode it carries.
    fn gui_podcast_episode_known(&self, lib_idx: usize, target: &PodcastEpisodeTarget) -> bool {
        let Some(state) = self.audiobookshelf_browse.get(lib_idx) else {
            return false;
        };
        state.detail_cache.is_empty()
            || state
                .episode_by_identity(target.library_item_id(), target.episode_id())
                .is_some()
    }
}
