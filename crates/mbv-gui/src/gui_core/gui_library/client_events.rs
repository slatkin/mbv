//! The GUI Library's Client-only fetch results (design D5, D8): the group's
//! album list, a plain Emby library's root list, a Shows library's New
//! episodes, and an Emby library's favourites. Only the GUI Client starts these
//! fetches, so their results never cross the shared library event channel. Each worker sends on the channel
//! `GuiCore` owns, and the GUI's tick drains it in
//! `ClientTickHost::drain_client_events` after `drain_lib_events`, waking the
//! tick with the same `ClientWaker`.

use super::GuiEmbyRootItems;
use super::request::GuiSectionRequest;
use super::store::GuiGroupAlbums;
use crate::gui_core::gui_client_state::GuiClientState;
use mbv_client_core::state::library_fetch_events::LibraryFetchEvent;
use mbv_emby_model::EmbyItem;

/// What one installed Client-only fetch result left in the store (design D8).
/// A first load lands its value, and a refresh keeps the value the store
/// already held when it failed, so the caller that holds the `App` reports the
/// failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClientEventLanding {
    /// The result installed in the store.
    Installed,
    /// A refresh failed, so the store kept the value it held.
    RefreshFailed,
}

/// Install one drained result in the Client's own stores (design D5, D8): a
/// first load takes the same mark the event handler applied while this result
/// crossed the shared library channel, and a refresh in flight replaces the
/// stored value on success and keeps it on failure. Either landing removes the
/// key from its in-flight set.
pub fn install_client_event(
    gui: &mut GuiClientState,
    event: LibraryFetchEvent,
) -> ClientEventLanding {
    match event {
        LibraryFetchEvent::GroupAlbums {
            lib_idx,
            group_id,
            result,
        } => {
            let refreshing = gui
                .music_albums_refreshing
                .remove(&(lib_idx, group_id.clone()));
            let mark = group_albums_mark(result);
            if refreshing && matches!(mark, GuiGroupAlbums::Failed) {
                return ClientEventLanding::RefreshFailed;
            }
            gui.music_albums
                .entry(lib_idx)
                .or_default()
                .insert(group_id, mark);
            ClientEventLanding::Installed
        }
        LibraryFetchEvent::EmbyRootItems { lib_idx, result } => {
            let refreshing = gui.emby_roots_refreshing.remove(&lib_idx);
            let mark = emby_roots_mark(lib_idx, result);
            if refreshing && matches!(mark, GuiEmbyRootItems::Failed) {
                return ClientEventLanding::RefreshFailed;
            }
            gui.emby_roots.insert(lib_idx, mark);
            ClientEventLanding::Installed
        }
        LibraryFetchEvent::ShowsNew { library_id, result } => {
            let refreshing = gui.shows_new_refreshing.remove(&library_id);
            let failed = result.is_err();
            let landed = shows_new_request(result);
            if refreshing && failed {
                return ClientEventLanding::RefreshFailed;
            }
            gui.shows_new.insert(library_id, landed);
            ClientEventLanding::Installed
        }
        LibraryFetchEvent::Favourites { library_id, result } => {
            let refreshing = gui.favourites_refreshing.remove(&library_id);
            let failed = result.is_err();
            let landed = favourites_request(result);
            if refreshing && failed {
                return ClientEventLanding::RefreshFailed;
            }
            gui.favourites.insert(library_id, landed);
            ClientEventLanding::Installed
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

/// Land one Emby library's favourites fetch (emby-favourites design D6):
/// success and failure both land the request, so a failed first load paints an
/// empty list and no frame starts the same fetch again. A failed in-place
/// refresh keeps the rows the store already held, which the caller decides
/// from `RefreshFailed`.
fn favourites_request(
    result: Result<Vec<EmbyItem>, mbv_emby::EmbyError>,
) -> GuiSectionRequest<EmbyItem> {
    match result {
        Ok(items) => GuiSectionRequest::landed(items),
        Err(error) => {
            tracing::info!(
                name: "gui_library.favourites.failed",
                target: "gui_library",
                error = %error,
                "the library's favourites failed to load"
            );
            GuiSectionRequest::landed(Vec::new())
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
