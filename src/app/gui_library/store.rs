//! The GUI Library's Music collection store (add-gui-library-panel design D5):
//! per Grouped Music library, the album list each selected group loaded. The
//! fetch runs off the Client core thread and lands in the store rather than in
//! `nav_stack`, so the TUI's navigation state and fetches do not change.

use mbv_emby_model::EmbyItem;

use crate::app::state::events::LibEvent;
use crate::app::{App, PAGE_SIZE};

/// One selected group's album list in the store (design D5): a group's first
/// selection starts its fetch, a loaded list lives for the session, and a
/// failed list keeps the other groups' albums on screen.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app) enum GuiGroupAlbums {
    /// The group's album fetch is in flight.
    Loading,
    /// The whole album list, paged to completion.
    Loaded(Vec<EmbyItem>),
    /// The group's fetch failed; the other selected groups keep their albums.
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
    /// Replace the selected collections of the Grouped Music library `lib_idx`
    /// (design D5, D6) and start the album fetch of every selected group the
    /// store does not hold. The store keeps a loaded group, so a deselected
    /// group selects again without a fetch. An empty set never reaches here:
    /// at least one pill stays selected.
    pub(in crate::app) fn set_gui_library_collections(
        &mut self,
        lib_idx: usize,
        group_ids: Vec<String>,
    ) {
        if let Some(reason) = self.gui_collections_reject_reason(lib_idx, &group_ids) {
            App::log_ignored_library_command(
                "setCollections",
                format_args!("library {lib_idx}: {reason}"),
            );
            return;
        }
        self.gui_library_groups.insert(lib_idx, group_ids.clone());
        tracing::info!(
            name: "gui_library.collections.set",
            target: "gui_library",
            library_index = lib_idx,
            groups = ?group_ids,
            "the selected collections changed"
        );
        for group_id in group_ids {
            self.fetch_gui_group_albums(lib_idx, group_id);
        }
    }

    /// Why the selected-collection set cannot apply to `lib_idx` (design D5,
    /// D6): the library index names no tab, the set is empty — the pill rules
    /// keep at least one pill selected, so a command that carries none is a
    /// stale frame's — or the destination is not a Grouped Music library.
    fn gui_collections_reject_reason(
        &self,
        lib_idx: usize,
        group_ids: &[String],
    ) -> Option<&'static str> {
        if lib_idx >= self.libs.len() {
            return Some("the library index names no tab");
        }
        if group_ids.is_empty() {
            return Some("the selected set is empty");
        }
        if !self.is_grouped_music_library(lib_idx) {
            return Some("the destination is not a Grouped Music library");
        }
        None
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
            self.gui_music_albums.insert(key, GuiGroupAlbums::Failed);
            return;
        };
        self.gui_music_albums.insert(key, GuiGroupAlbums::Loading);
        let tx = self.channels.lib_tx.clone();
        std::thread::spawn(move || {
            let result = fetch_all_albums(&client, &group_id);
            let _ = tx.send(LibEvent::GuiGroupAlbums(GuiGroupAlbumsEvent {
                lib_idx,
                group_id,
                result,
            }));
        });
    }

    /// Land one group's album fetch (design D5): a loaded list replaces the
    /// loading mark, and a failure marks only that group, so the other
    /// selected groups keep their albums.
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

/// Fetch every page of `group_id`'s album listing (design D5): the
/// `get_items_sorted` call `App::spawn_browse` makes with the group as parent
/// and the name sort, paged until the server's total is reached.
fn fetch_all_albums(
    client: &mbv_emby::EmbyClient,
    group_id: &str,
) -> Result<Vec<EmbyItem>, mbv_emby::EmbyError> {
    let mut albums: Vec<EmbyItem> = Vec::new();
    loop {
        let (page, total) = client.get_items_sorted(
            group_id,
            None,
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
