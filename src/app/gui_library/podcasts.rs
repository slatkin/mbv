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

use std::borrow::Cow;

use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseKind;
use mbv_ui_msg::PodcastEpisodeTarget;

use super::collection::{
    GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet, GuiDateOrder,
};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::podcasts_projection::GuiPodcastEpisodeId;
use super::tabs::GuiDestinationKind;

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

    fn present(&self, app: &App) -> Option<impl Fn(&str) -> bool> {
        // A show list that holds no show keeps the stored set as it is: it is
        // still landing, and the list is what would name a show stale (row
        // 10.1).
        let state = app.audiobookshelf_browse.get(self.0)?;
        if state.shows.is_empty() {
            return None;
        }
        Some(move |key: &str| state.shows.iter().any(|show| show.library_item_id == key))
    }

    fn scopes(&self, app: &App) -> impl IntoIterator<Item = (String, String)> {
        app.gui_podcast_shows(self.0)
    }

    fn stored_set<'a>(&self, app: &'a App) -> &'a GuiCollectionSet {
        app.gui_podcast_libs
            .get(&self.0)
            .map_or(&GuiCollectionSet::ALL, |stored| &stored.selection.set)
    }

    fn stored_order(&self, app: &App) -> GuiDateOrder {
        app.gui_podcast_libs
            .get(&self.0)
            .map_or(GuiDateOrder::default(), |stored| stored.selection.order)
    }

    fn has_stored_selection(&self, app: &App) -> bool {
        app.gui_podcast_libs.contains_key(&self.0)
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
    /// nothing here, so no second chain walks beside the first. An open also
    /// retries a catalog whose load failed, because the frame path's root step
    /// honours that failure until an open clears it (design D4, Risks). A
    /// `lib_idx` that names no podcast library is an ignored command.
    pub fn gui_podcast_open(&mut self, lib_idx: usize) {
        if !self.gui_podcast_library(lib_idx) {
            Self::log_ignored_library_command(
                "podcastOpen",
                format_args!("the destination is no Podcasts library"),
            );
            return;
        }
        if !self.gui_abs_destination_active(lib_idx) {
            return;
        }
        self.clear_gui_podcast_catalog_failed(lib_idx);
        if self.gui_podcast_catalog_needs_load(lib_idx) {
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
    /// destination's one load pass — the show list's refresh while it is
    /// unloaded, the checked shows' scope commit, then All's one-shot shelf
    /// fetch and the bounded per-show fan-out (gui-one-destination-ensure
    /// design D3). The collection commands install their set and then enter
    /// here, so this is also where a stale frame's command for another podcast
    /// library stops (review of units 2.4 and 2.5): a `lib_idx` that names no
    /// podcast library, or no active destination, is a no-op, and an automatic
    /// repaint always names the active one.
    pub fn ensure_gui_podcast_selection(&mut self, lib_idx: usize) {
        if !self.gui_abs_destination_active(lib_idx) {
            return;
        }
        self.ensure_gui_destination(GuiDestinationKind::Podcasts { lib_idx });
    }

    /// Whether `lib_idx`'s show catalog is unloaded in a shape a show-list load
    /// may start from (gui-one-destination-ensure design D3, Risks): an empty
    /// list, a zero total, no page in flight, no replacement batch collecting,
    /// and no failed mark. The in-flight page marks count as unloaded's
    /// negation, as the books destination's rule has it: the startup chain's
    /// first page is already walking, and a load beside it would start a
    /// second chain.
    ///
    /// The failed mark is load-bearing: `handle_audiobookshelf_shows_fetched`
    /// records a failure as `state.error` and aborts the replacement batch,
    /// which leaves the list empty, the total zero, and no page in flight —
    /// exactly the shape above — so without the mark a repaint would retry the
    /// failed chain every frame. The frame path's root step and
    /// [`Self::gui_podcast_open`] both call this one rule, so the two starts
    /// cannot drift.
    pub(in crate::app::gui_library) fn gui_podcast_catalog_needs_load(
        &self,
        lib_idx: usize,
    ) -> bool {
        self.audiobookshelf_browse
            .get(lib_idx)
            .is_some_and(|state| {
                state.shows.is_empty()
                    && state.total == 0
                    && state.loading_pages.is_empty()
                    && state.replacement.is_none()
                    && state.error.is_none()
            })
    }

    /// Clear the failed mark a user open retries from (design D4, Risks): the
    /// loader's failure record is the browse state's `error` — the same fact
    /// the projection paints as the list's failed state — so the frame path's
    /// root step honours it and an open clears it. An automatic repaint never
    /// clears it.
    pub(in crate::app::gui_library) fn clear_gui_podcast_catalog_failed(&mut self, lib_idx: usize) {
        if let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx) {
            state.error = None;
        }
    }

    /// Commit the checked shows' scope the bounded fan-out reads — the
    /// selection step of the destination's one load pass (gui-one-destination-ensure
    /// design D3; gui-collection-dropdown design D2). Exactly one
    /// checked show scopes the fan-out to that show, and All or several checked
    /// shows need every listed show.
    pub(in crate::app) fn commit_gui_podcast_effective_scope(&mut self, lib_idx: usize) {
        let set = self.gui_podcast_effective_set(lib_idx);
        let single_show = match set.as_ref() {
            GuiCollectionSet::Checked(keys) if keys.len() == 1 => keys.first().cloned(),
            _ => None,
        };
        self.commit_gui_podcast_show_scope(lib_idx, single_show.as_deref());
    }

    /// Start the loads the committed selection depends on — the dependents step
    /// of the destination's one load pass (gui-one-destination-ensure design
    /// D3, row 10.1): All's one-shot shelf fetch, then every required show's
    /// episodes through the bounded fan-out, or the one checked show's. Every
    /// guard is idempotent (the shelf's request set, the fan-out's cached and
    /// in-flight skips), so a later pass loads nothing.
    pub(in crate::app) fn ensure_gui_podcast_dependents(&mut self, lib_idx: usize) {
        if self.gui_podcast_effective_set(lib_idx).is_all()
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
    /// shows stale. A set nothing drops is returned borrowed, so a per-frame
    /// reader clones nothing.
    pub(in crate::app) fn gui_podcast_effective_set(
        &self,
        lib_idx: usize,
    ) -> Cow<'_, GuiCollectionSet> {
        GuiPodcastsCollectionSite(lib_idx).effective_set(self)
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
    fn commit_gui_podcast_show_scope(&mut self, lib_idx: usize, show: Option<&str>) {
        if let Some(state) = self.audiobookshelf_browse.get_mut(lib_idx)
            && state.committed_show_pill.as_deref() != show
        {
            state.committed_show_pill = show.map(str::to_owned);
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
