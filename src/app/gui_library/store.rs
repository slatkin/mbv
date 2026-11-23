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
use crate::app::state::events::LibEvent;
use crate::app::{App, PAGE_SIZE};

/// One group's album list in the store (design D5): a group's first selection
/// starts its fetch, a loaded list lives for the session, and a failed list
/// stays failed until a selection change puts the group back in the selection
/// ([`GuiCore::install_gui_library_collection`] drops the failed marks, so the
/// ensure step starts the fetch again).
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum GuiGroupAlbums {
    /// The group's album fetch is in flight.
    Loading,
    /// The whole album list, paged to completion.
    Loaded(Vec<EmbyItem>),
    /// The group's fetch failed.
    Failed,
}

/// The completion of one group's album fetch, carried on the shared library
/// event channel. Only the GUI Client's store starts one, so the TUI never
/// receives it.
#[derive(Debug)]
pub(in crate::app) struct GuiGroupAlbumsEvent {
    pub(in crate::app) lib_idx: usize,
    pub(in crate::app) group_id: String,
    pub(in crate::app) result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
}

/// Install the collection set of the Grouped Music library `lib_idx` over the
/// `App` the GUI core owns (gui-collection-dropdown design D2, D11): the one
/// entry every selection change goes through, so the Hero's album carries
/// across the change. While the new selection is All or several groups, the
/// session union pick takes the album the Hero showed under the previous
/// selection, so the Hero keeps it while the new union holds it; one checked
/// group resolves through its kept pick instead, so nothing carries. The
/// checked groups' album fetches start through the ensure step, one at a time.
pub(in crate::app) fn install_library_collection(
    app: &mut App,
    lib_idx: usize,
    set: GuiCollectionSet,
) {
    // The previous selection as the projection paints it: the effective set,
    // not the raw stored entry, so a set the catalog trimmed (one of two
    // checked groups dropped) carries through the one group's kept pick.
    let previous = super::projection::effective_music_set(app, lib_idx);
    let next_single = matches!(&set, GuiCollectionSet::Checked(keys) if keys.len() == 1);
    if !next_single && let Some(album_id) = hero_album_under(app, lib_idx, &previous) {
        app.gui.music_union_picks.insert(lib_idx, album_id);
    }
    app.gui
        .library_groups
        .insert(lib_idx, GuiCollectionSelection { set });
    clear_failed_music_groups(app, lib_idx);
    run_destination_open(app, GuiDestinationKind::Music { lib_idx });
}

/// Run one destination's ordered load pass (gui-one-destination-ensure design
/// D1): the pass is the GUI core's, so the collection install reaches it by
/// handing the core the App it holds.
fn run_destination_open(app: &mut App, kind: GuiDestinationKind) {
    let mut core =
        crate::gui::GuiCore::new(std::mem::replace(app, crate::app::app_stub::handoff_app()));
    core.ensure_gui_destination(kind);
    *app = core.into_app();
}

/// Drop the failed album marks of `lib_idx`'s selected groups (design D5): a
/// selection change is the one refetch trigger, so a hard failure never retries
/// on every frame. The ensure step then starts the cleared groups' fetches
/// again, one at a time.
fn clear_failed_music_groups(app: &mut App, lib_idx: usize) {
    let set = super::projection::effective_music_set(app, lib_idx).into_owned();
    let group_ids = super::projection::selected_groups(app, lib_idx, &set)
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let Some(groups) = app.gui.music_albums.get_mut(&lib_idx) else {
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
fn hero_album_under(app: &App, lib_idx: usize, set: &GuiCollectionSet) -> Option<String> {
    match set {
        GuiCollectionSet::Checked(keys) if keys.len() == 1 => {
            if let Some(pick) = app.gui.library_picks.get(&(lib_idx, keys[0].clone())) {
                return Some(pick.clone());
            }
            match app
                .gui
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
        _ => app.gui.music_union_picks.get(&lib_idx).cloned(),
    }
}

/// Why a collection command cannot apply to `lib_idx` at all (design D5, D6):
/// the library index names no tab, or the destination is not a Grouped Music
/// library. The key-level rule is [`collection_reject_reason`]'s own.
fn library_collection_target_reject_reason(app: &App, lib_idx: usize) -> Option<&'static str> {
    if lib_idx >= app.libs.len() {
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
pub(in crate::app) fn fetch_group_albums(app: &mut App, lib_idx: usize, group_id: String) {
    if matches!(
        app.gui
            .music_albums
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
    app.gui
        .music_albums
        .entry(lib_idx)
        .or_default()
        .insert(group_id.clone(), GuiGroupAlbums::Loading);
    let tx = app.channels.lib_tx.clone();
    std::thread::spawn(move || {
        let result = fetch_all_pages(&client, &group_id, None);
        let _ = tx.send(LibEvent::GuiGroupAlbums(GuiGroupAlbumsEvent {
            lib_idx,
            group_id,
            result,
        }));
    });
}

/// Land one group's album fetch over the `App` the GUI core owns (design D5): a
/// loaded list replaces the loading mark, and a failure marks only that group.
pub(in crate::app) fn handle_group_albums(app: &mut App, event: GuiGroupAlbumsEvent) {
    let GuiGroupAlbumsEvent {
        lib_idx,
        group_id,
        result,
    } = event;
    let state = match result {
        Ok(albums) => GuiGroupAlbums::Loaded(albums),
        Err(error) => {
            tracing::info!(
                name: "gui_library.group_albums.failed",
                target: "gui_library",
                library_index = lib_idx,
                group = %group_id,
                error = %error,
                "the group's album list failed to load"
            );
            GuiGroupAlbums::Failed
        }
    };
    app.gui
        .music_albums
        .entry(lib_idx)
        .or_default()
        .insert(group_id, state);
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

    fn stored_set<'a>(&self, app: &'a App) -> &'a GuiCollectionSet {
        app.gui
            .library_groups
            .get(&self.0)
            .map_or(&GuiCollectionSet::ALL, |stored| &stored.set)
    }

    fn has_stored_selection(&self, app: &App) -> bool {
        app.gui.library_groups.contains_key(&self.0)
            || app
                .gui
                .library_picks
                .keys()
                .any(|(lib_idx, _)| *lib_idx == self.0)
    }

    fn saved_picks(&self, app: &App) -> Vec<GuiGroupPick> {
        let mut picks: Vec<GuiGroupPick> = app
            .gui
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

    fn install_picks(&self, app: &mut App, picks: &[GuiGroupPick]) {
        for pick in picks {
            app.gui
                .library_picks
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

    fn install_set(&self, app: &mut App, set: GuiCollectionSet) {
        install_library_collection(app, self.0, set);
    }
}

/// Fetch every page of `parent_id`'s listing (design D5, row 8.1): the
/// `get_items_sorted` call `App::spawn_browse` makes with the given parent
/// and item-type filter and the name sort, paged until the server's total is
/// reached. The Music collection store and the plain Emby library's
/// root-list store share it.
pub(in crate::app::gui_library) fn fetch_all_pages(
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
