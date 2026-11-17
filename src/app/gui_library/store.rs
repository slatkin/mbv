//! The GUI Library's Music collection store (add-gui-library-panel design D5):
//! per Grouped Music library, the selected collection set and the album list
//! each group loaded. The fetch runs off the Client core thread and lands in
//! the store rather than in `nav_stack`, so the TUI's navigation state and
//! fetches do not change. The selection is All or a set of checked groups
//! (gui-collection-dropdown design D1, D2, D11).

use mbv_emby_model::EmbyItem;

use super::collection::GuiCollectionSet;
use crate::app::state::events::LibEvent;
use crate::app::{App, PAGE_SIZE};

/// One group's album list in the store (design D5): a group's first selection
/// starts its fetch, a loaded list lives for the session, and a failed list
/// stays failed until the group is selected again.
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

impl App {
    /// Select the one collection group of the Grouped Music library `lib_idx`
    /// as a checked set of that one group (gui-collection-dropdown design
    /// D1): the pill press's single-group path, which the dropdown's toggle
    /// commands replace in a later unit. The store keeps the choice per
    /// library and starts the groups' album fetches when the store holds no
    /// list for them, so a group selected again fetches nothing.
    pub(in crate::app) fn select_gui_library_collection(
        &mut self,
        lib_idx: usize,
        group_id: String,
    ) {
        if let Some(reason) = self.gui_collection_reject_reason(lib_idx, &group_id) {
            App::log_ignored_library_command(
                "selectCollection",
                format_args!("library {lib_idx}: {reason}"),
            );
            return;
        }
        tracing::info!(
            name: "gui_library.collections.set",
            target: "gui_library",
            library_index = lib_idx,
            group = %group_id,
            "the selected collection changed"
        );
        self.install_gui_library_collection(lib_idx, GuiCollectionSet::Checked(vec![group_id]));
    }

    /// Install the collection set of the Grouped Music library `lib_idx`
    /// (gui-collection-dropdown design D2, D11): the one entry every
    /// selection change goes through, so the Hero's album carries across the
    /// change. While the new selection is All or several groups, the session
    /// union pick takes the album the Hero showed under the previous
    /// selection, so the Hero keeps it while the new union holds it; one
    /// checked group resolves through its kept pick instead, so nothing
    /// carries. The checked groups' album fetches start through the ensure
    /// step, one at a time.
    pub(in crate::app) fn install_gui_library_collection(
        &mut self,
        lib_idx: usize,
        set: GuiCollectionSet,
    ) {
        // The previous selection as the projection paints it: the effective
        // set, not the raw stored entry, so a set the catalog trimmed (one
        // of two checked groups dropped) carries through the one group's
        // kept pick.
        let previous = self.gui_effective_music_set(lib_idx);
        let next_single = matches!(&set, GuiCollectionSet::Checked(keys) if keys.len() == 1);
        if !next_single && let Some(album_id) = self.gui_hero_album_under(lib_idx, &previous) {
            self.gui_music_union_picks.insert(lib_idx, album_id);
        }
        self.gui_library_groups.insert(lib_idx, set);
        self.ensure_gui_music_selection(lib_idx);
    }

    /// The album the Hero showed under `set` (gui-collection-dropdown design
    /// D11), from the kept picks and the loaded store lists alone: one
    /// checked group's kept pick, else that group's first loaded album; All
    /// or several groups' union pick, and nothing else — a carry must never
    /// seed an album the Hero did not show, so a several-checked selection
    /// with no union pick carries none and resolution falls through to the
    /// union's first album. `None` while no album is known.
    fn gui_hero_album_under(&self, lib_idx: usize, set: &GuiCollectionSet) -> Option<String> {
        match set {
            GuiCollectionSet::Checked(keys) if keys.len() == 1 => {
                let key = (lib_idx, keys[0].clone());
                if let Some(pick) = self.gui_library_picks.get(&key) {
                    return Some(pick.clone());
                }
                match self.gui_music_albums.get(&key) {
                    Some(GuiGroupAlbums::Loaded(albums)) => {
                        albums.first().map(|album| album.id.clone())
                    }
                    Some(GuiGroupAlbums::Loading | GuiGroupAlbums::Failed) | None => None,
                }
            }
            _ => self.gui_music_union_picks.get(&lib_idx).cloned(),
        }
    }

    /// Why the selected collection cannot apply to `lib_idx` (design D5, D6):
    /// the library index names no tab, the destination is not a Grouped Music
    /// library, or the library's group list holds no group `group_id` (a stale
    /// frame's pick). A group list that has not landed yet admits the pick,
    /// because the list is what would name it stale.
    fn gui_collection_reject_reason(&self, lib_idx: usize, group_id: &str) -> Option<&'static str> {
        if lib_idx >= self.libs.len() {
            return Some("the library index names no tab");
        }
        if !self.is_grouped_music_library(lib_idx) {
            return Some("the destination is not a Grouped Music library");
        }
        let level = self.gui_music_group_level(lib_idx)?;
        if level.items.iter().any(|group| group.id == group_id) {
            None
        } else {
            Some("the group is not one of the library's collections")
        }
    }

    /// Start the shown groups' album fetches when the store holds no list for
    /// them (design D5, gui-collection-dropdown design D1): the frame path
    /// calls this for the Music destination, so All's and every checked
    /// group's list loads without a pick. One fetch runs at a time per
    /// library: the next group's fetch starts on a later frame, when the
    /// previous one landed. A failed fetch is not retried here: it stays
    /// failed until the user selects the group again, so a paint never
    /// re-issues it.
    pub fn ensure_gui_music_selection(&mut self, lib_idx: usize) {
        let group_ids = self.gui_selected_groups(lib_idx);
        if group_ids.iter().any(|group_id| {
            matches!(
                self.gui_music_albums.get(&(lib_idx, group_id.clone())),
                Some(GuiGroupAlbums::Loading)
            )
        }) {
            return;
        }
        let Some(group_id) = group_ids.into_iter().find(|group_id| {
            !self
                .gui_music_albums
                .contains_key(&(lib_idx, group_id.clone()))
        }) else {
            return;
        };
        self.fetch_gui_group_albums(lib_idx, group_id);
    }

    /// Start `group_id`'s album fetch when the store holds no list for it
    /// (design D5): the Client core thread never fetches, and the completion
    /// lands in the store outside `nav_stack`.
    pub(in crate::app) fn fetch_gui_group_albums(&mut self, lib_idx: usize, group_id: String) {
        let key = (lib_idx, group_id.clone());
        if matches!(
            self.gui_music_albums.get(&key),
            Some(GuiGroupAlbums::Loading | GuiGroupAlbums::Loaded(_))
        ) {
            return;
        }
        let Some(client) = self.emby_snapshot() else {
            // The no-client failure lands through the same event a real fetch
            // failure takes (design D5): the started fetch stays `Loading`
            // until the event drains, so a build that starts it and the next
            // collect's clean build hold the same store state — a synchronous
            // `Failed` insert here would change the Library section's built
            // content between collects with no mark, and the window would
            // never see the change.
            self.gui_music_albums.insert(key, GuiGroupAlbums::Loading);
            let tx = self.channels.lib_tx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(LibEvent::GuiGroupAlbums(GuiGroupAlbumsEvent {
                    lib_idx,
                    group_id,
                    result: Err(mbv_emby::EmbyError::resolve(
                        "the Emby Service holds no client",
                    )),
                }));
            });
            return;
        };
        self.gui_music_albums.insert(key, GuiGroupAlbums::Loading);
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let result = fetch_all_pages(&client, &group_id, None);
            let _ = tx.send(LibEvent::GuiGroupAlbums(GuiGroupAlbumsEvent {
                lib_idx,
                group_id,
                result,
            }));
        });
    }

    /// Land one group's album fetch (design D5): a loaded list replaces the
    /// loading mark, and a failure marks only that group.
    pub(in crate::app) fn handle_gui_group_albums(&mut self, event: GuiGroupAlbumsEvent) {
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
        self.gui_music_albums.insert((lib_idx, group_id), state);
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
