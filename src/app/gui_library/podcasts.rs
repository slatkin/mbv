//! The GUI Library's Podcasts data path (add-gui-library-panel design D12,
//! row 10.1): the per-podcast-library selection the core owns — the checked
//! shows and the picked episode — and the open, toggle, and All actions the
//! core's collection commands apply. The
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

use super::collection::{GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::ensure::catalog_unloaded;
use super::podcasts_projection::GuiPodcastEpisodeId;
use super::tabs::GuiDestinationKind;

use crate::app::App;
use crate::app::gui_client_state::GuiClientState;
use crate::gui::GuiCore;

/// The GUI Library's Podcasts selection (row 10.1): per podcast library, the
/// stored collection selection — the checked shows (gui-collection-dropdown
/// design D1, D2) — and the episode the Hero shows. All is the set default,
/// so All with no pick is the selection every launch starts on (the spec's
/// persistence rule — session memory only, nothing saved). The list's sort
/// lives in the list-sort store. All fans out to
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
    /// The checked shows.
    pub selection: GuiCollectionSelection,
    /// The episode the Hero shows. `None` before a select command names one.
    pub picked_episode: Option<PodcastEpisodeTarget>,
}

impl Default for GuiPodcastsSelection {
    /// All with no pick: the selection every launch starts on (the spec's
    /// persistence rule — session memory only, nothing saved).
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
        super::podcasts_projection::podcast_shows(app, self.0)
    }

    fn stored_set<'a>(&self, gui: &'a GuiClientState) -> &'a GuiCollectionSet {
        gui.podcast_libs
            .get(&self.0)
            .map_or(&GuiCollectionSet::ALL, |stored| &stored.selection.set)
    }

    fn has_stored_selection(&self, gui: &GuiClientState) -> bool {
        gui.podcast_libs.contains_key(&self.0)
    }

    fn reject_destination(&self, app: &App) -> Option<CollectionRejection> {
        if podcast_library(app, self.0) {
            None
        } else {
            Some(CollectionRejection::Ignored(
                "the destination is no Podcasts library".to_string(),
            ))
        }
    }

    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection> {
        if podcast_show_known(app, self.0, key) {
            None
        } else {
            Some(CollectionRejection::Stale(key.to_string()))
        }
    }

    fn install_set(&self, app: &mut App, gui: &mut GuiClientState, set: GuiCollectionSet) {
        install_podcast_set(app, gui, self.0, set);
    }
}

/// Whether `lib_idx` names a Podcasts destination (row 10.1): an
/// Audiobookshelf library whose `media_type` resolves to the podcast
/// browse kind. Every GUI Podcasts step's destination guard, and the
/// mirror of the Audiobooks guard: a book library stays Audiobooks.
pub(in crate::app::gui_library) fn podcast_library(app: &App, lib_idx: usize) -> bool {
    app.audiobookshelf_libraries
        .get(lib_idx)
        .is_some_and(|library| {
            AudiobookshelfBrowseKind::from_media_type(&library.media_type)
                == AudiobookshelfBrowseKind::Podcast
        })
}

impl GuiCore {
    /// Open the Podcasts destination `lib_idx` (row 10.1; gui-one-destination-ensure
    /// design D4): the open clears a failed catalog mark, then runs the
    /// destination's load pass. A `lib_idx` that names no podcast library is an
    /// ignored command.
    pub fn gui_podcast_open(&mut self, lib_idx: usize) {
        open_podcast(&mut self.app, &mut self.gui, lib_idx);
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
            super::log_stale_library_item("podcastSelect", row);
            return;
        };
        select_podcast(&mut self.app, &mut self.gui, lib_idx, &episode.target());
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
            super::log_stale_library_item("podcastPlay", item_id);
            return;
        };
        play_podcast_episode(&mut self.app, lib_idx, &episode.target());
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
            super::log_stale_library_item("podcastQueue", item_id);
            return;
        };
        enqueue_podcast_episode(&mut self.app, lib_idx, &episode.target());
    }
}

/// Open the Podcasts destination `lib_idx` (row 10.1): the open clears a failed
/// catalog mark, then runs the destination's one load pass.
pub(in crate::app::gui_library) fn open_podcast(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
) {
    if !podcast_library(app, lib_idx) {
        super::log_ignored_library_command(
            "podcastOpen",
            format_args!("the destination is no Podcasts library"),
        );
        return;
    }
    clear_podcast_catalog_failed(app, lib_idx);
    run_podcast_loads(app, gui, lib_idx);
}

/// The one write the toggle and the All press take: the shared collection
/// path installs the change, the store records it, and the loads the
/// selection needs start (gui-collection-dropdown design D2, D5).
pub(in crate::app::gui_library) fn install_podcast_set(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    set: GuiCollectionSet,
) {
    gui.podcast_libs.entry(lib_idx).or_default().selection.set = set;
    run_podcast_loads(app, gui, lib_idx);
}

/// Run the Podcasts destination's one ordered load pass (gui-one-
/// destination-ensure design D1): the GUI core owns the pass, so a step that
/// holds an `App` hands it to the core.
fn run_podcast_loads(app: &mut App, gui: &mut GuiClientState, lib_idx: usize) {
    let mut core = GuiCore::new(std::mem::replace(app, crate::app::handoff_app()));
    *core.gui_state_mut() = std::mem::take(gui);
    core.ensure_gui_destination(GuiDestinationKind::Podcasts { lib_idx });
    *gui = std::mem::take(core.gui_state_mut());
    *app = core.into_app();
}

/// Select the episode the Hero shows (row 10.1): the episode's own fields and
/// its show's facts are already cached, so no load starts.
fn select_podcast(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    target: &PodcastEpisodeTarget,
) {
    if !podcast_library(app, lib_idx) {
        super::log_ignored_library_command(
            "podcastSelect",
            format_args!("the destination is no Podcasts library"),
        );
        return;
    }
    if !podcast_episode_known(app, lib_idx, target) {
        super::log_stale_library_item("podcastSelect", target.episode_id());
        return;
    }
    gui.podcast_libs.entry(lib_idx).or_default().picked_episode = Some(target.clone());
}

/// Whether `lib_idx`'s show catalog is unloaded in a shape a show-list load
/// may start from ([`catalog_unloaded`]).
pub(in crate::app::gui_library) fn podcast_catalog_needs_load(app: &App, lib_idx: usize) -> bool {
    app.audiobookshelf_browse.get(lib_idx).is_some_and(|state| {
        catalog_unloaded(
            state.shows.is_empty(),
            state.total,
            &state.loading_pages,
            state.replacement.is_none(),
            state.error.is_none(),
        )
    })
}

/// Clear the failed mark a user open retries from (design D4, Risks): the
/// loader's failure record is the browse state's `error`.
pub(in crate::app::gui_library) fn clear_podcast_catalog_failed(app: &mut App, lib_idx: usize) {
    if let Some(state) = app.audiobookshelf_browse.get_mut(lib_idx) {
        state.error = None;
    }
}

/// Commit the checked shows' scope the bounded fan-out reads (gui-one-
/// destination-ensure design D3; gui-collection-dropdown design D2).
pub(in crate::app) fn commit_podcast_effective_scope(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
) {
    let set = podcast_effective_set(app, gui, lib_idx);
    let single_show = match set.as_ref() {
        GuiCollectionSet::Checked(keys) if keys.len() == 1 => keys.first().cloned(),
        _ => None,
    };
    commit_podcast_show_scope(app, lib_idx, single_show.as_deref());
}

/// Start the loads the committed selection depends on (gui-one-destination-
/// ensure design D3, row 10.1): All's one-shot shelf fetch, then every required
/// show's episodes through the bounded fan-out, or the one checked show's.
pub(in crate::app) fn ensure_podcast_dependents(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
) {
    if podcast_effective_set(app, gui, lib_idx).is_all()
        && let Some(library_id) = app
            .audiobookshelf_libraries
            .get(lib_idx)
            .map(|library| library.id.clone())
    {
        app.ensure_audiobookshelf_latest_shelf(&library_id);
    }
    app.start_audiobookshelf_podcast_fan_out(lib_idx);
}

/// Play the episode a command names (row 10.1).
pub(in crate::app::gui_library) fn play_podcast_episode(
    app: &mut App,
    lib_idx: usize,
    target: &PodcastEpisodeTarget,
) {
    if app
        .selected_audiobookshelf_queue_item_target(lib_idx, target)
        .is_none()
    {
        super::log_stale_library_item("podcastPlay", target.episode_id());
        return;
    }
    app.play_selected_audiobookshelf_episode_target(lib_idx, target);
}

/// Add the episode a command names to the queue (row 10.1).
pub(in crate::app::gui_library) fn enqueue_podcast_episode(
    app: &mut App,
    lib_idx: usize,
    target: &PodcastEpisodeTarget,
) {
    let Some(item) = app.selected_audiobookshelf_queue_item_target(lib_idx, target) else {
        super::log_stale_library_item("podcastQueue", target.episode_id());
        return;
    };
    if !app.player.can_admit_audiobookshelf() {
        super::audiobooks::flash_abs_owner_unavailable(app);
        return;
    }
    let _ = app.submit_queue_item(item, false);
}

/// The effective selection the list and the pill row paint (row 10.1;
/// gui-collection-dropdown design D1).
pub(in crate::app) fn podcast_effective_set<'a>(
    app: &'a App,
    gui: &'a GuiClientState,
    lib_idx: usize,
) -> Cow<'a, GuiCollectionSet> {
    GuiPodcastsCollectionSite(lib_idx).effective_set(app, gui)
}

/// The episode the Hero shows (row 10.1): the stored pick.
#[must_use]
pub fn podcast_picked_episode(
    gui: &GuiClientState,
    lib_idx: usize,
) -> Option<&PodcastEpisodeTarget> {
    gui.podcast_libs.get(&lib_idx)?.picked_episode.as_ref()
}

/// Commit the checked shows' scope to the browse state the bounded fan-out
/// reads (row 10.1; gui-collection-dropdown design D2).
fn commit_podcast_show_scope(app: &mut App, lib_idx: usize, show: Option<&str>) {
    if let Some(state) = app.audiobookshelf_browse.get_mut(lib_idx)
        && state.committed_show_pill.as_deref() != show
    {
        state.committed_show_pill = show.map(str::to_owned);
    }
}

/// Whether the published show list can still name `show_id` (row 10.1): a
/// list that holds no show admits the press, and a loaded list admits only a
/// show it carries.
fn podcast_show_known(app: &App, lib_idx: usize, show_id: &str) -> bool {
    let Some(state) = app.audiobookshelf_browse.get(lib_idx) else {
        return false;
    };
    state.shows.is_empty()
        || state
            .shows
            .iter()
            .any(|show| show.library_item_id == show_id)
}

/// Whether the published per-show cache can still name `target` (row 10.1): a
/// cache that holds no episode admits the pick, and a landed cache admits only
/// an episode it carries.
fn podcast_episode_known(app: &App, lib_idx: usize, target: &PodcastEpisodeTarget) -> bool {
    let Some(state) = app.audiobookshelf_browse.get(lib_idx) else {
        return false;
    };
    state.detail_cache.is_empty()
        || state
            .episode_by_identity(target.library_item_id(), target.episode_id())
            .is_some()
}
