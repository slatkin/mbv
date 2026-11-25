//! The GUI Library's Music collection store (add-gui-library-panel design D5):
//! per Grouped Music library, the selected collection set and the album list
//! each group loaded. The fetch runs off the Client core thread and lands in
//! the store rather than in `nav_stack`, so the TUI's navigation state and
//! fetches do not change. The selection is All or a set of checked groups
//! (gui-collection-dropdown design D1, D2, D11).

use mbv_config::GuiGroupPick;
use mbv_emby_model::EmbyItem;

use super::collection::{GuiCollectionNoun, GuiCollectionSelection, GuiCollectionSet};
use super::collection_command::{CollectionRejection, GuiCollectionSite};
use super::tabs::GuiDestinationKind;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::state::library_fetch_events::LibraryFetchEvent;
use mbv_client_core::{App, PAGE_SIZE};

/// One group's album list in the store (design D5): a group's first selection
/// starts its fetch, a loaded list lives for the session, and a failed list
/// stays failed until a selection change puts the group back in the selection
/// ([`GuiCore::install_gui_library_collection`] drops the failed marks, so the
/// ensure step starts the fetch again).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum GuiGroupAlbums {
    /// The group's album fetch is in flight.
    Loading,
    /// The whole album list, paged to completion.
    Loaded(Vec<EmbyItem>),
    /// The group's fetch failed.
    Failed,
}

/// Install the collection set of the Grouped Music library `lib_idx` over the
/// `App` the GUI core owns (gui-collection-dropdown design D2, D11): the one
/// entry every selection change goes through, so the Hero's album carries
/// across the change. While the new selection is All or several groups, the
/// session union pick takes the album the Hero showed under the previous
/// selection, so the Hero keeps it while the new union holds it; one checked
/// group resolves through its kept pick instead, so nothing carries. The
/// checked groups' album fetches start through the ensure step, one at a time.
pub(crate) fn install_library_collection(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    set: GuiCollectionSet,
) {
    // The previous selection as the projection paints it: the effective set,
    // not the raw stored entry, so a set the catalog trimmed (one of two
    // checked groups dropped) carries through the one group's kept pick.
    let previous = super::projection::effective_music_set(app, gui, lib_idx);
    let next_single = matches!(&set, GuiCollectionSet::Checked(keys) if keys.len() == 1);
    if !next_single && let Some(album_id) = hero_album_under(gui, lib_idx, &previous) {
        gui.music_union_picks.insert(lib_idx, album_id);
    }
    gui.library_groups
        .insert(lib_idx, GuiCollectionSelection { set });
    clear_failed_music_groups(app, gui, lib_idx);
    run_destination_open(app, gui, GuiDestinationKind::Music { lib_idx });
}

/// Run one destination's ordered load pass (gui-one-destination-ensure design
/// D1): the pass is the GUI core's, so the collection install reaches it by
/// handing the core the App it holds.
fn run_destination_open(app: &mut App, gui: &mut GuiClientState, kind: GuiDestinationKind) {
    let mut core =
        crate::gui_core::GuiCore::new(std::mem::replace(app, mbv_client_core::handoff_app()));
    // The caller's own state moves into the core and back, so the pass sees
    // the selection it just installed and the caller keeps the state.
    *core.gui_state_mut() = std::mem::take(gui);
    core.ensure_gui_destination(kind);
    // The core's two halves take back separately: the pass kept none of them.
    *gui = std::mem::take(core.gui_state_mut());
    *app = core.into_app();
}

/// Drop the failed album marks of `lib_idx`'s selected groups (design D5): a
/// selection change is the one refetch trigger, so a hard failure never retries
/// on every frame. The ensure step then starts the cleared groups' fetches
/// again, one at a time.
fn clear_failed_music_groups(app: &App, gui: &mut GuiClientState, lib_idx: usize) {
    let set = super::projection::effective_music_set(app, gui, lib_idx).into_owned();
    let group_ids = super::projection::selected_groups(app, gui, lib_idx, &set)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let Some(groups) = gui.music_albums.get_mut(&lib_idx) else {
        return;
    };
    for group_id in group_ids {
        if matches!(groups.get(&group_id), Some(GuiGroupAlbums::Failed)) {
            groups.remove(&group_id);
        }
    }
}

/// The album the Hero showed under `set` (gui-collection-dropdown design D11),
/// from the kept picks and the loaded store lists alone: one checked group's
/// kept pick, else that group's first loaded album; All or several groups'
/// union pick, and nothing else — a carry must never seed an album the Hero did
/// not show, so a several-checked selection with no union pick carries none and
/// resolution falls through to the union's first album. `None` while no album
/// is known.
fn hero_album_under(
    gui: &GuiClientState,
    lib_idx: usize,
    set: &GuiCollectionSet,
) -> Option<String> {
    match set {
        GuiCollectionSet::Checked(keys) if keys.len() == 1 => {
            if let Some(pick) = gui.library_picks.get(&(lib_idx, keys[0].clone())) {
                return Some(pick.clone());
            }
            match gui
                .music_albums
                .get(&lib_idx)
                .and_then(|groups| groups.get(&keys[0]))
            {
                Some(GuiGroupAlbums::Loaded(albums)) => {
                    albums.first().map(|album| album.id.clone())
                }
                Some(GuiGroupAlbums::Loading | GuiGroupAlbums::Failed) | None => None,
            }
        }
        _ => gui.music_union_picks.get(&lib_idx).cloned(),
    }
}

/// Why a collection command cannot apply to `lib_idx` at all (design D5, D6):
/// the library index names no tab, or the destination is not a Grouped Music
/// library. The key-level rule is [`collection_reject_reason`]'s own.
fn library_collection_target_reject_reason(app: &App, lib_idx: usize) -> Option<&'static str> {
    if lib_idx >= app.emby_library_count() {
        return Some("the library index names no tab");
    }
    if !app.is_grouped_music_library(lib_idx) {
        return Some("the destination is not a Grouped Music library");
    }
    None
}

/// Why the collection group `group_id` cannot apply to `lib_idx` (design D5,
/// D6): the library itself cannot serve a collection command, or the library's
/// group list holds no group `group_id` (a stale frame's key). A group list that
/// has not landed yet admits the key, because the list is what would name it
/// stale.
fn collection_reject_reason(app: &App, lib_idx: usize, group_id: &str) -> Option<&'static str> {
    if let Some(reason) = library_collection_target_reject_reason(app, lib_idx) {
        return Some(reason);
    }
    let level = super::launch::music_group_level(app, lib_idx)?;
    if level.items.iter().any(|group| group.id == group_id) {
        None
    } else {
        Some("the group is not one of the library's collections")
    }
}

/// Start `group_id`'s album fetch when the store holds no list for it over the
/// `App` the GUI core owns (design D5): the Client core thread never fetches,
/// and the completion lands in the store outside `nav_stack`.
pub(crate) fn fetch_group_albums(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    group_id: String,
) {
    if matches!(
        gui.music_albums
            .get(&lib_idx)
            .and_then(|groups| groups.get(&group_id)),
        Some(GuiGroupAlbums::Loading | GuiGroupAlbums::Loaded(_))
    ) {
        return;
    }
    let Some(client) = app.emby_snapshot() else {
        // The no-client condition is not a fetch failure (design D5): the
        // group stays un-fetched, with no store mark, and a later frame's
        // ensure starts the fetch once the client exists. A mark here would
        // leave the fetch terminal with no retry but a user press, and a
        // synchronous store change inside the Library section's build would
        // change the built content between collects with no mark.
        return;
    };
    gui.music_albums
        .entry(lib_idx)
        .or_default()
        .insert(group_id.clone(), GuiGroupAlbums::Loading);
    spawn_group_albums_fetch(app, client, lib_idx, group_id);
}

/// Refetch `group_id`'s album list over the `App` the GUI core owns (design
/// D8): with no loaded list the refetch clears the group's failed mark and
/// takes the first-load path, and with a loaded list it keeps that list in
/// place and starts the fetch, recording the group in the in-flight set so a
/// second refetch while that fetch runs starts nothing. No Emby client starts
/// nothing at all.
pub(crate) fn refetch_group_albums(
    app: &mut App,
    gui: &mut GuiClientState,
    lib_idx: usize,
    group_id: String,
) {
    let Some(client) = app.emby_snapshot() else {
        // The no-client condition is not a fetch failure (design D5, D8): the
        // refetch stores no mark and starts nothing.
        return;
    };
    let loaded = matches!(
        gui.music_albums
            .get(&lib_idx)
            .and_then(|groups| groups.get(&group_id)),
        Some(GuiGroupAlbums::Loaded(_))
    );
    if !loaded {
        clear_failed_group_albums(gui, lib_idx, &group_id);
        fetch_group_albums(app, gui, lib_idx, group_id);
        return;
    }
    if !gui
        .music_albums_refreshing
        .insert((lib_idx, group_id.clone()))
    {
        return;
    }
    spawn_group_albums_fetch(app, client, lib_idx, group_id);
}

/// Clear one group's failed mark (design D8): a refetch of a store with no
/// loaded list is the user's retry, so the first-load path starts the group
/// again.
fn clear_failed_group_albums(gui: &mut GuiClientState, lib_idx: usize, group_id: &str) {
    let Some(groups) = gui.music_albums.get_mut(&lib_idx) else {
        return;
    };
    if matches!(groups.get(group_id), Some(GuiGroupAlbums::Failed)) {
        groups.remove(group_id);
    }
}

/// Start `group_id`'s album fetch worker (design D5, D8): the one spawn body
/// the first load and the refetch share, so both send the same result event,
/// whose landing replaces the store's value or reports a failed refresh.
fn spawn_group_albums_fetch(
    app: &App,
    client: mbv_emby::EmbyClient,
    lib_idx: usize,
    group_id: String,
) {
    let tx = app.library_fetch_event_sender();
    std::thread::spawn(move || {
        let result = fetch_all_pages(&client, &group_id, None);
        tx.send(LibraryFetchEvent::GroupAlbums {
            lib_idx,
            group_id,
            result,
        });
    });
}

/// The Grouped Music destination's collection-command site
/// (gui-collection-dropdown design D5): the destination-level and stale-key
/// rejects, the group catalog, and the one selection install the shared
/// collection path calls.
pub(super) struct GuiMusicCollectionSite(pub(super) usize);

impl GuiCollectionSite for GuiMusicCollectionSite {
    fn noun(&self) -> GuiCollectionNoun {
        GuiCollectionNoun::Groups
    }

    fn present(&self, app: &App) -> Option<impl Fn(&str) -> bool> {
        let level = super::launch::music_group_level(app, self.0)?;
        if level.loading {
            return None;
        }
        Some(move |key: &str| level.items.iter().any(|group| group.id == key))
    }

    fn scopes(&self, app: &App) -> impl IntoIterator<Item = (String, String)> {
        super::projection::music_groups(app, self.0)
    }

    fn stored_set<'a>(&self, gui: &'a GuiClientState) -> &'a GuiCollectionSet {
        gui.library_groups
            .get(&self.0)
            .map_or(&GuiCollectionSet::ALL, |stored| &stored.set)
    }

    fn has_stored_selection(&self, gui: &GuiClientState) -> bool {
        gui.library_groups.contains_key(&self.0)
            || gui
                .library_picks
                .keys()
                .any(|(lib_idx, _)| *lib_idx == self.0)
    }

    fn saved_picks(&self, gui: &GuiClientState) -> Vec<GuiGroupPick> {
        let mut picks: Vec<GuiGroupPick> = gui
            .library_picks
            .iter()
            .filter(|((pick_lib, _), _)| *pick_lib == self.0)
            .map(|((_, group_id), album_id)| GuiGroupPick {
                group_id: group_id.clone(),
                album_id: album_id.clone(),
            })
            .collect();
        picks.sort_by(|a, b| a.group_id.cmp(&b.group_id));
        picks
    }

    fn install_picks(&self, _app: &mut App, gui: &mut GuiClientState, picks: &[GuiGroupPick]) {
        for pick in picks {
            gui.library_picks
                .entry((self.0, pick.group_id.clone()))
                .or_insert_with(|| pick.album_id.clone());
        }
    }

    fn reject_destination(&self, app: &App) -> Option<CollectionRejection> {
        library_collection_target_reject_reason(app, self.0)
            .map(|reason| CollectionRejection::Ignored(format!("library {}: {reason}", self.0)))
    }

    fn reject_key(&self, app: &App, key: &str) -> Option<CollectionRejection> {
        // A group list that has not landed admits the key: the list is what
        // would name it stale.
        collection_reject_reason(app, self.0, key)
            .map(|reason| CollectionRejection::Ignored(format!("library {}: {reason}", self.0)))
    }

    fn log_toggle(&self, key: &str) {
        tracing::info!(
            name: "gui_library.collections.toggle",
            target: "gui_library",
            library_index = self.0,
            group = %key,
            "the selected collection changed"
        );
    }

    fn install_set(&self, app: &mut App, gui: &mut GuiClientState, set: GuiCollectionSet) {
        install_library_collection(app, gui, self.0, set);
    }
}

/// Fetch every page of `parent_id`'s listing (design D5, row 8.1): the
/// `get_items_sorted` call `App::spawn_browse` makes with the given parent
/// and item-type filter and the name sort, paged until the server's total is
/// reached. The Music collection store and the plain Emby library's
/// root-list store share it.
pub(in crate::gui_core::gui_library) fn fetch_all_pages(
    client: &mbv_emby::EmbyClient,
    parent_id: &str,
    item_types: Option<&str>,
) -> Result<Vec<EmbyItem>, mbv_emby::EmbyError> {
    let mut albums: Vec<EmbyItem> = Vec::new();
    loop {
        let (page, total) = client.get_items_sorted(
            parent_id,
            item_types,
            false,
            albums.len(),
            PAGE_SIZE,
            "SortName",
            "Ascending",
        )?;
        if page.is_empty() {
            return Ok(albums);
        }
        albums.extend(page);
        if albums.len() >= total {
            return Ok(albums);
        }
    }
}
