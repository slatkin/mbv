//! The GUI Library's Client-only fetch results (design D5): the group's album
//! list, a plain Emby library's root list, and a Shows library's New episodes.
//! Only the GUI Client starts these fetches, so their results never cross the
//! shared library event channel. Each worker sends on the channel `GuiCore`
//! owns, and the GUI's tick drains it in `ClientTickHost::drain_client_events`
//! after `drain_lib_events`, waking the tick with the same `ClientWaker`.

use super::GuiEmbyRootItems;
use super::request::GuiSectionRequest;
use super::store::GuiGroupAlbums;
use crate::gui::gui_client_state::GuiClientState;
use mbv_client_core::state::gui_client_events::GuiClientEvent;
use mbv_emby_model::EmbyItem;

/// Install one drained result in the Client's own stores: the same rule the
/// event handler applied while this result crossed the shared library channel.
pub fn install_client_event(gui: &mut GuiClientState, event: GuiClientEvent) {
    match event {
        GuiClientEvent::GroupAlbums {
            lib_idx,
            group_id,
            result,
        } => {
            gui.music_albums
                .entry(lib_idx)
                .or_default()
                .insert(group_id, group_albums_mark(result));
        }
        GuiClientEvent::EmbyRootItems { lib_idx, result } => {
            gui.emby_roots
                .insert(lib_idx, emby_roots_mark(lib_idx, result));
        }
        GuiClientEvent::ShowsNew { library_id, result } => {
            gui.shows_new.insert(library_id, shows_new_request(result));
        }
    }
}

/// The store mark one group's album fetch lands (design D5): a loaded list
/// replaces the loading mark, and a failure marks only that group.
pub(crate) fn group_albums_mark(
    result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
) -> GuiGroupAlbums {
    match result {
        Ok(albums) => GuiGroupAlbums::Loaded(albums),
        Err(error) => {
            tracing::info!(
                name: "gui_library.group_albums.failed",
                target: "gui_library",
                error = %error,
                "the group's album list failed to load"
            );
            GuiGroupAlbums::Failed
        }
    }
}

/// The root-list mark one plain Emby library's fetch lands: a loaded list
/// replaces the loading mark, and a failure marks only that library.
fn emby_roots_mark(
    lib_idx: usize,
    result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
) -> GuiEmbyRootItems {
    match result {
        Ok(items) => GuiEmbyRootItems::Loaded(items),
        Err(error) => {
            tracing::info!(
                name: "gui_library.emby_root.failed",
                target: "gui_library",
                library_index = lib_idx,
                error = %error,
                "the library's root list failed to load"
            );
            GuiEmbyRootItems::Failed
        }
    }
}

/// Land one TV library's New fetch: success and failure both land the
/// request, so an empty or failed list paints the empty state and no later
/// launch step or frame starts the same fetch again.
fn shows_new_request(
    result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
) -> GuiSectionRequest<EmbyItem> {
    match result {
        Ok(items) => GuiSectionRequest::landed(items),
        Err(error) => {
            tracing::info!(
                name: "gui_library.shows_new.failed",
                target: "gui_library",
                error = %error,
                "the library's latest episodes failed to load"
            );
            GuiSectionRequest::landed(Vec::new())
        }
    }
}
