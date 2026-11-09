//! The GUI Library's Music collection store (add-gui-library-panel design D5):
//! per Grouped Music library, the one selected group and the album list each
//! group loaded. The fetch runs off the Client core thread and lands in the
//! store rather than in `nav_stack`, so the TUI's navigation state and fetches
//! do not change.

use mbv_emby_model::EmbyItem;

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
    /// (design D5, D6): one group is always selected, as in the TUI. The store
    /// keeps the choice per library and starts the group's album fetch when
    /// the store holds no list for it, so a group selected again fetches
    /// nothing.
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
        self.gui_library_groups.insert(lib_idx, group_id.clone());
        self.fetch_gui_group_albums(lib_idx, group_id);
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

    /// Start the shown group's album fetch when the store holds no list for it
    /// (design D5). The frame path calls this for the Music destination, so
    /// the default first group loads without a pick. A failed fetch is not
    /// retried here: it stays failed until the user selects the group again,
    /// so a paint never re-issues it.
    pub fn ensure_gui_music_selection(&mut self, lib_idx: usize) {
        let Some(group_id) = self.gui_selected_group(lib_idx) else {
            return;
        };
        if self
            .gui_music_albums
            .contains_key(&(lib_idx, group_id.clone()))
        {
            return;
        }
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
