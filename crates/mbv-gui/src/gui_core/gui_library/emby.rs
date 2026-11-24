//! The GUI Library's plain Emby-library data path (add-gui-library-panel
//! design D12, row 8.1): the per-library selection the core owns (the picked
//! row, a feed-view library's checked folder set, and that library's claim on
//! its fetched results with their failed targets — gui-collection-dropdown
//! design D2, D3), and the open, select, collection-toggle, and All actions
//! the core's commands apply. A feed-view library's checked set reads and
//! fills the shared `feed_home_video` cache without writing its `selection`,
//! so the TUI's own pill selection stays the TUI's. The root-list store
//! follows the Music collection store's precedent (design D5): the TUI's root
//! path cannot serve a full flat list — a library over `LIBRARY_PILL_THRESHOLD`
//! is bucketed by letter range and a smaller one pages as its cursor moves —
//! so the store fetches the whole root list with `get_items_sorted`, paged to
//! completion, outside `nav_stack`. Every action runs the TUI's existing
//! loaders, so the two Clients cannot drift. The TUI never reads any of it.
//!
//! Every step is a free function over the `App` the GUI core owns and the GUI
//! Client's own state beside it, with the `GuiCore` entries as one-line calls
//! (design D4), so the load sites, the collection site, and the projections
//! reach them through the two values they already hold.

use std::borrow::Cow;
use std::collections::HashSet;

use mbv_emby_model::EmbyItem;
use mbv_ui_model::feed::FeedHomeVideoTarget;

use super::collection::{GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::tabs::GuiDestinationKind;
use crate::gui_core::GuiCore;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::App;
use mbv_client_core::state::gui_client_events::GuiClientEvent;
use mbv_ui_model::feed::FeedHomeVideoSelection;

/// One plain Emby library's root list in the store (row 8.1): a library's
/// first open starts its fetch, a loaded list lives for the session, and a
/// failed list stays failed until the library is opened again.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GuiEmbyRootItems {
    /// The root-list fetch is in flight.
    Loading,
    /// The whole root list, paged to completion, in the server's `SortName`
    /// order.
    Loaded(Vec<EmbyItem>),
    /// The root-list fetch failed.
    Failed,
}

/// The GUI Library's own state per Emby library (row 8.1): the row the Hero
/// shows, by stable Service id, and a feed-view library's claim on that
/// library's fetched results. A feed-view library's checked folder set is the
/// GUI's own store beside it
/// ([`emby_feed_effective_set`]), read over the
/// library tab's shared `feed_home_video` cache without writing its
/// `selection` — the TUI's selection stays the TUI's. A stored row the active
/// list no longer carries falls back to the list's first row in the
/// projection, as every selection's stale arm does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiEmbyLibrarySelection {
    /// The row the Hero shows. `None` before a select command names one.
    pub picked_row: Option<String>,
    /// The GUI's claim on a feed-view library's fetched results (design D2,
    /// D3): the frame path's shown pass marks it, so the result arm accepts a
    /// target the GUI's checked set wants while the stored set stays absent
    /// until a user press or the launch restore installs one — a claim is not
    /// a choice, so a painted frame never blocks the saved folders. A plain
    /// Emby library never sets it.
    pub feed_claim: Option<GuiEmbyFeedClaim>,
}

/// The GUI's claim on one feed-view library's fetched results (design D2,
/// D3), held beside that library's [`GuiEmbyLibrarySelection`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuiEmbyFeedClaim {
    /// Targets whose fetch failed terminally: the shown pass's ensure step
    /// offers them again only after a user press clears the mark, so an
    /// automatic repaint never retries them.
    pub failed: HashSet<FeedHomeVideoSelection>,
}

impl GuiEmbyFeedClaim {
    /// Whether `target`'s fetch failed, probed through the borrowed target so
    /// the per-frame ensure walk clones no folder id.
    fn is_failed(&self, target: FeedHomeVideoTarget<'_>) -> bool {
        self.failed.iter().any(|mark| match (mark, target) {
            (FeedHomeVideoSelection::All, FeedHomeVideoTarget::All) => true,
            (FeedHomeVideoSelection::Folder(mark_id), FeedHomeVideoTarget::Folder(target_id)) => {
                mark_id == target_id
            }
            (FeedHomeVideoSelection::All, FeedHomeVideoTarget::Folder(_))
            | (FeedHomeVideoSelection::Folder(_), FeedHomeVideoTarget::All) => false,
        })
    }
}

/// The Emby feed-group destination's collection-command site
/// (gui-collection-dropdown design D5): the feed-view guard, the root
/// listing's staleness rule, the folder catalog, and the selection install
/// the shared collection path calls.
pub(super) struct GuiEmbyFeedCollectionSite(pub(super) usize);

impl GuiCollectionSite for GuiEmbyFeedCollectionSite {
    fn noun(&self) -> GuiCollectionNoun {
        GuiCollectionNoun::Folders
    }

    fn present(&self, app: &App) -> Option<impl Fn(&str) -> bool> {
        let root = app.emby_library_root_level(self.0)?;
        if root.loading {
            return None;
        }
        Some(move |key: &str| {
            root.items
                .iter()
                .any(|item| item.is_folder && item.id == key)
        })
    }

    fn scopes(&self, app: &App) -> impl IntoIterator<Item = (String, String)> {
        super::emby_projection::root_folder_names(app, self.0)
            .into_iter()
            .map(|(id, name)| (id.to_owned(), name.to_owned()))
    }

    fn stored_set<'a>(&self, gui: &'a GuiClientState) -> &'a GuiCollectionSet {
        gui.emby_feed_sets
            .get(&self.0)
            .map_or(&GuiCollectionSet::ALL, |stored| &stored.set)
    }

    fn has_stored_selection(&self, gui: &GuiClientState) -> bool {
        // Only a user press or the launch restore stores a set: the shown
        // pass's claim is not a selection, so a painted frame never blocks
        // the saved folders.
        gui.emby_feed_sets.contains_key(&self.0)
    }

    fn reject_destination(&self, app: &App) -> Option<CollectionRejection> {
        emby_feed_reject_reason(app, self.0)
            .map(|reason| CollectionRejection::Ignored(format!("library {}: {reason}", self.0)))
    }

    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection> {
        if emby_folder_known(app, self.0, key) {
            None
        } else {
            Some(CollectionRejection::Stale(key.to_string()))
        }
    }

    fn install_set(&self, app: &mut App, gui: &mut GuiClientState, set: GuiCollectionSet) {
        install_emby_feed_set(app, gui, self.0, set);
    }
}

impl GuiCore {
    /// Open the Emby library `lib_idx` (row 8.1; gui-one-destination-ensure
    /// design D4).
    pub fn gui_emby_open(&mut self, lib_idx: usize) {
        open_emby(&mut self.app, &mut self.gui, lib_idx);
    }

    /// Select the row the Hero shows (row 8.1).
    pub fn gui_emby_select(&mut self, lib_idx: usize, item_id: &str) {
        select_emby(&mut self.app, &mut self.gui, lib_idx, item_id);
    }

    /// Play the plain Emby-library row a command names (design D12, row 8.1).
    pub(in crate::gui_core::gui_library) fn play_gui_emby_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        play_emby_item(&mut self.app, &self.gui, lib_idx, item_id);
    }

    /// Play the plain Emby-library row next (design D12, row 8.1).
    pub(in crate::gui_core::gui_library) fn play_next_gui_emby_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        play_next_emby_item(&mut self.app, &self.gui, lib_idx, item_id);
    }

    /// Add the plain Emby-library row to the queue (design D12, row 8.1).
    pub(in crate::gui_core::gui_library) fn enqueue_gui_emby_item(
        &mut self,
        lib_idx: usize,
        item_id: &str,
    ) {
        enqueue_emby_item(&mut self.app, &self.gui, lib_idx, item_id);
    }
}

/// Whether `lib_idx` names a plain Emby library destination (row 8.1): every
/// GUI Emby-library step's destination guard — an Emby library that is neither
/// a Grouped Music library nor a TV (Shows) library.
pub(in crate::gui_core::gui_library) fn emby_library(app: &App, lib_idx: usize) -> bool {
    app.emby_library_collection_type(lib_idx)
        .is_some_and(|collection_type| {
            !app.is_grouped_music_library(lib_idx) && collection_type != "tvshows"
        })
}

/// Open the Emby library `lib_idx` (row 8.1; gui-one-destination-ensure design
/// D4): the open clears the failed root mark a plain library's list may hold —
/// a failed list retries on an open, not on a repaint — and then runs the
/// destination's load pass, whose root step starts the root-list store's fetch
/// (the TUI path cannot serve the full flat list the letter rail reads, the
/// module doc) and whose selection step starts a feed-view library's checked
/// folders' fetches. The TUI's own activation, the tab settlement's
/// `activate_library_position`, still starts the root listing or the
/// saved-position restore here. A `lib_idx` that names no plain Emby library is
/// an ignored command.
pub(in crate::gui_core::gui_library) fn open_emby(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
) {
    if !emby_library(app, lib_idx) {
        super::log_ignored_library_command(
            "embyOpen",
            format_args!("the destination is no plain Emby library"),
        );
        return;
    }
    if app.emby_library_level(lib_idx).is_none() {
        app.activate_library_position(lib_idx);
    }
    if !app.is_feed_home_video_library(lib_idx) {
        // A failed list retries on an open (the failure "stays failed until
        // the library is opened again"), so the open clears the failed mark the
        // root step's fetch would otherwise honour.
        if matches!(gui.emby_roots.get(&lib_idx), Some(GuiEmbyRootItems::Failed)) {
            gui.emby_roots.remove(&lib_idx);
        }
    }
    // The ordered load pass is the GUI core's, so the open step hands the core
    // the App and the state it holds, and takes both back.
    let mut core = GuiCore::new(std::mem::replace(app, mbv_client_core::handoff_app()));
    *core.gui_state_mut() = std::mem::take(gui);
    core.ensure_gui_destination(GuiDestinationKind::Library { lib_idx });
    *gui = std::mem::take(core.gui_state_mut());
    *app = core.into_app();
}

/// Select the row the Hero shows (row 8.1): the selection the core owns, so
/// the frame carries it back. A plain library's row must be one the root-list
/// store holds — or one whose list is still landing, which the list is what
/// would name stale — and a feed-view library's row must be one the merged
/// checked-folder list holds. A stale frame's id changes nothing, as every
/// item-named command's stale arm does.
pub(in crate::gui_core::gui_library) fn select_emby(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    item_id: &str,
) {
    if !emby_library(app, lib_idx) {
        super::log_ignored_library_command(
            "embySelect",
            format_args!("the destination is no plain Emby library"),
        );
        return;
    }
    if !emby_row_known(app, gui, lib_idx, item_id) {
        super::log_stale_library_item("embySelect", item_id);
        return;
    }
    let unchanged = gui
        .emby_libs
        .get(&lib_idx)
        .and_then(|selection| selection.picked_row.as_deref())
        == Some(item_id);
    if unchanged {
        return;
    }
    gui.emby_libs.entry(lib_idx).or_default().picked_row = Some(item_id.to_string());
    // A pick move is library navigation: it closes the image warm gate, so fast
    // arrow traffic warms no neighbour art (the TUI's own rule).
    app.stamp_navigation();
}

/// Whether the active list can still name `item_id` (row 8.1): a feed-view
/// library's merged checked-folder list, else the root-list store's loaded
/// list. A loading or not-yet-started list admits the pick, because the list is
/// what would name it stale; a failed list names nothing. The feed-group arm
/// reads the merged list without building its date-sorted order — the
/// membership walk alone answers.
fn emby_row_known(app: &App, gui: &GuiClientState, lib_idx: usize, item_id: &str) -> bool {
    if app.is_feed_home_video_library(lib_idx) {
        let set = emby_feed_effective_set(app, gui, lib_idx);
        let (items, loading) = super::emby_projection::feed_gathered(app, lib_idx, set.as_ref());
        return loading || items.iter().any(|item| item.id == item_id);
    }
    match gui.emby_roots.get(&lib_idx) {
        Some(GuiEmbyRootItems::Loaded(items)) => items.iter().any(|item| item.id == item_id),
        Some(GuiEmbyRootItems::Failed) => false,
        Some(GuiEmbyRootItems::Loading) | None => true,
    }
}

/// Why a collection command cannot apply to `lib_idx` at all (row 8.1;
/// gui-collection-dropdown design D5): the index names no plain Emby library,
/// or that library presents no feed-view folders.
fn emby_feed_reject_reason(app: &App, lib_idx: usize) -> Option<&'static str> {
    if !emby_library(app, lib_idx) {
        return Some("the destination is no plain Emby library");
    }
    if !app.is_feed_home_video_library(lib_idx) {
        return Some("the destination is no feed-view library");
    }
    None
}

/// Store the feed-view library's set and start the fetches the set is missing
/// (row 8.1; gui-collection-dropdown design D5): the one write the toggle and
/// All presses share.
pub(in crate::gui_core::gui_library) fn install_emby_feed_set(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    set: GuiCollectionSet,
) {
    gui.emby_feed_sets
        .insert(lib_idx, GuiCollectionSelection { set });
    // A user press (or the launch restore) is the one refetch trigger (design
    // D3): clearing the failed marks here lets the ensure step start the
    // selection's failed targets again, and an automatic repaint never does.
    if let Some(claim) = gui
        .emby_libs
        .get_mut(&lib_idx)
        .and_then(|selection| selection.feed_claim.as_mut())
    {
        claim.failed.clear();
    }
    ensure_emby_feed_fetch(app, gui, lib_idx);
}

/// Start the first fetch the GUI's checked set is missing (design D3): the
/// first target — All, or the first checked folder — with no cache entry and no
/// terminal failure mark, while no fetch is outstanding. `fetching` names the
/// one fetch that runs, so the next target's fetch starts on a later frame once
/// it clears: one fetch at a time, as the TUI's own selection fetch. The shared
/// `FeedHomeVideoState::selection` is never written here, so the TUI keeps its
/// own selection. The targets iterate the checked set's borrowed keys, and All
/// is one cache probe, so the per-frame ensure clones nothing for the walk.
pub(in crate::gui_core::gui_library) fn ensure_emby_feed_fetch(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
) {
    // The GUI owns this library's results from the first frame it shows: the
    // claim marks them as the GUI's own, which the result arm's second
    // acceptance rule reads. The claim is not a stored selection, so a painted
    // frame never blocks the launch restore. A Client that never shows the
    // destination claims nothing, so the TUI's own stale-drop rule alone
    // governs its results.
    gui.emby_libs
        .entry(lib_idx)
        .or_default()
        .feed_claim
        .get_or_insert_with(GuiEmbyFeedClaim::default);
    let Some(state) = app.emby_library_feed_home_video(lib_idx) else {
        return;
    };
    if state.fetching.is_some() {
        return;
    }
    // A failed target waits for a user press: the press clears its mark, so an
    // automatic repaint never retries it and no retry storm starts.
    let failed = |target: FeedHomeVideoTarget<'_>| {
        emby_feed_claim(gui, lib_idx).is_some_and(|claim| claim.is_failed(target))
    };
    let next = match emby_feed_effective_set(app, gui, lib_idx).as_ref() {
        GuiCollectionSet::All => (state.items_for(FeedHomeVideoTarget::All).is_none()
            && !failed(FeedHomeVideoTarget::All))
        .then_some(FeedHomeVideoSelection::All),
        GuiCollectionSet::Checked(keys) => keys
            .iter()
            .find(|key| {
                state
                    .items_for(FeedHomeVideoTarget::Folder(key.as_str()))
                    .is_none()
                    && !failed(FeedHomeVideoTarget::Folder(key.as_str()))
            })
            .map(|key| FeedHomeVideoSelection::Folder(key.clone())),
    };
    let Some(target) = next else {
        return;
    };
    app.spawn_feed_home_video_fetch(lib_idx, target);
}

/// The GUI's claim on `lib_idx`'s fetched results (design D2, D3): `None`
/// before the frame path's shown pass marks the library, which is what leaves
/// the TUI's own stale-drop rule alone in a Client that never shows the
/// destination.
fn emby_feed_claim(gui: &GuiClientState, lib_idx: usize) -> Option<&GuiEmbyFeedClaim> {
    gui.emby_libs
        .get(&lib_idx)
        .and_then(|selection| selection.feed_claim.as_ref())
}

/// The folder catalog the checked set resolves against (design D1, D3): the
/// root listing's folders' stable ids, or `None` while the listing has not
/// landed — the set is then kept as it is, because the listing is what would
/// name a folder gone.
fn emby_folder_catalog(app: &App, lib_idx: usize) -> Option<Vec<String>> {
    let root = app.emby_library_root_level(lib_idx)?;
    if root.loading {
        return None;
    }
    Some(
        root.items
            .iter()
            .filter(|item| item.is_folder)
            .map(|item| item.id.clone())
            .collect(),
    )
}

/// Whether the GUI may bind a press to `folder_id` (row 8.1): the root listing
/// holds the folder, or the listing has not landed yet — it is what would name
/// the folder gone.
fn emby_folder_known(app: &App, lib_idx: usize, folder_id: &str) -> bool {
    emby_folder_catalog(app, lib_idx).is_none_or(|present| present.iter().any(|id| id == folder_id))
}

/// The effective feed-group selection the pills, the list, and the fetches read
/// (design D1): the stored set with every checked folder the root listing no
/// longer holds dropped, so a vanished folder's check falls back to All when
/// none survive. All stays All. A set nothing drops is returned borrowed, so a
/// per-frame reader clones nothing.
pub(crate) fn emby_feed_effective_set<'a>(
    app: &'a App,
    gui: &'a GuiClientState,
    lib_idx: usize,
) -> Cow<'a, GuiCollectionSet> {
    GuiEmbyFeedCollectionSite(lib_idx).effective_set(app, gui)
}

/// Whether `target` is the GUI state's effective feed-group selection (design
/// D3): the result arm's second acceptance rule, once the Client's own state
/// has decided the library is claimed (that read is the state's, not the
/// `App`'s — design D4a (c)). All is one compare, and a checked set is the key
/// set's own compare, so no target list is built.
#[must_use]
pub(crate) fn emby_feed_wants_target_in_set(
    app: &App,
    gui: &GuiClientState,
    lib_idx: usize,
    target: &FeedHomeVideoSelection,
) -> bool {
    match emby_feed_effective_set(app, gui, lib_idx).as_ref() {
        GuiCollectionSet::All => target == &FeedHomeVideoSelection::All,
        GuiCollectionSet::Checked(keys) => {
            matches!(
                target,
                FeedHomeVideoSelection::Folder(wanted) if keys.contains(wanted)
            )
        }
    }
}

/// Start `lib_idx`'s root-list fetch when the store holds no list (row 8.1):
/// the Client core thread never fetches, and the completion lands in the store
/// outside `nav_stack`. A failed fetch is not retried here: it stays failed
/// until the library is opened again, so a paint never re-issues it.
pub(crate) fn fetch_emby_root_items(app: &mut App, gui: &mut GuiClientState, lib_idx: usize) {
    if matches!(
        gui.emby_roots.get(&lib_idx),
        Some(GuiEmbyRootItems::Loading | GuiEmbyRootItems::Loaded(_) | GuiEmbyRootItems::Failed)
    ) {
        return;
    }
    let Some(client) = app.emby_snapshot() else {
        gui.emby_roots.insert(lib_idx, GuiEmbyRootItems::Failed);
        return;
    };
    let (item_types, _unplayed_only, sort_by, sort_order) =
        app.default_library_level_fields(lib_idx);
    debug_assert_eq!(
        sort_by.as_str(),
        "SortName",
        "the root-list store reads the name sort"
    );
    debug_assert_eq!(sort_order.as_str(), "Ascending");
    let Some(parent_id) = app.emby_library_id(lib_idx).map(str::to_string) else {
        return;
    };
    gui.emby_roots.insert(lib_idx, GuiEmbyRootItems::Loading);
    let tx = app.gui_client_event_sender();
    std::thread::spawn(move || {
        let result = super::store::fetch_all_pages(&client, &parent_id, item_types.as_deref());
        tx.send(GuiClientEvent::EmbyRootItems { lib_idx, result });
    });
}

/// Play the plain Emby-library row a command names (design D12, row 8.1): the
/// shared library play path, whose folder-play rule expands a folder or a
/// boxset into its playable content and whose admission and route checks are
/// the Owner's queue's own. A stale id plays nothing.
pub(in crate::gui_core::gui_library) fn play_emby_item(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) {
    // The row resolution is the GUI's, so it reads its own stores beside the
    // core's; the play itself is the shared core path.
    let item = super::emby_projection::emby_item(app, gui, lib_idx, item_id);
    match item {
        Some(item) => app.play_or_activate_lib_item(lib_idx, item),
        None => super::log_stale_library_item("embyPlay", item_id),
    }
}

/// Play the plain Emby-library row next (design D12, row 8.1): the shared
/// play-next admission, so the row enters directly after the active slot.
pub(in crate::gui_core::gui_library) fn play_next_emby_item(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) {
    let item = super::emby_projection::emby_item(app, gui, lib_idx, item_id);
    match item {
        Some(item) => app.play_lib_item_next(lib_idx, item),
        None => super::log_stale_library_item("embyPlayNext", item_id),
    }
}

/// Add the plain Emby-library row to the queue (design D12, row 8.1): the
/// shared add path.
pub(in crate::gui_core::gui_library) fn enqueue_emby_item(
    app: &mut App,
    gui: &GuiClientState,
    lib_idx: usize,
    item_id: &str,
) {
    let item = super::emby_projection::emby_item(app, gui, lib_idx, item_id);
    match item {
        Some(item) => app.enqueue_lib_item(lib_idx, item),
        None => super::log_stale_library_item("embyQueue", item_id),
    }
}

/// The root items the GUI store loaded for `lib_idx` (row 8.1): the list the
/// letter rail and the projection read. Empty while the list is loading or its
/// fetch failed.
pub(crate) fn stored_emby_root_items(
    gui: &GuiClientState,
    lib_idx: usize,
) -> impl Iterator<Item = &EmbyItem> {
    match gui.emby_roots.get(&lib_idx) {
        Some(GuiEmbyRootItems::Loaded(items)) => items.iter(),
        Some(GuiEmbyRootItems::Loading | GuiEmbyRootItems::Failed) | None => [].iter(),
    }
}
