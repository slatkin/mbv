//! The library-event seam a Client uses to keep its own library stores
//! (`decouple-client-core` design D4a (c)). The library event handlers run in
//! the core; what a landed or failed fetch means for a Client's own stores is
//! the Client's business. The handlers take an optional `ClientLibraryStores`,
//! raise [`ClientLibraryEffect`] values, and apply them through it after the
//! event that raised them. A Client that keeps no such stores (the TUI) passes
//! `None` and nothing happens, as today.

use crate::app::gui_client_state::GuiClientState;
use crate::app::{App, FeedHomeVideoSelection};

/// Take one pending library event and apply it through `gui`, the GUI
/// Client's own stores (`decouple-client-core` design D4a (c)). The seam
/// types stay as private as the GUI state they carry, so the GUI Client calls
/// this instead of `App::drain_lib_event_with_stores`.
pub(crate) fn drain_lib_event_through(app: &mut App, gui: &mut GuiClientState) -> bool {
    app.drain_lib_event_with_stores(Some(gui))
}

/// What one Client does with the effects a library event raises. The
/// implementor (`GuiClientState`) reads and writes its own fields; `app` is
/// the shared core state, which the effect may also touch (the in-view row
/// artwork fetch reads the core's catalog).
pub(in crate::app) trait ClientLibraryStores {
    /// Whether this Client's own selection still wants the landed feed
    /// target, beside the shared selection the core keeps.
    fn wants_feed_target(&self, app: &App, lib_idx: usize, target: &FeedHomeVideoSelection)
    -> bool;

    /// Apply one effect that a library event raised.
    fn apply(&mut self, app: &mut App, effect: ClientLibraryEffect);

    /// Store the event that the core's handler carried out (design D4): one
    /// method per store, so a new carriage does not compile until its store
    /// installs it. A Client that keeps no such stores never receives one,
    /// because it never starts the event.
    fn apply_event(&mut self, seam: ClientLibraryEventSeam) {
        match seam {
            ClientLibraryEventSeam::GroupAlbums(event) => self.install_group_albums(event),
            ClientLibraryEventSeam::EmbyRootItems { lib_idx, items } => {
                self.install_emby_roots(lib_idx, items);
            }
            ClientLibraryEventSeam::ShowsNew {
                library_id,
                request,
            } => self.install_shows_new(library_id, request),
        }
    }

    /// Install one group's landed album list.
    fn install_group_albums(&mut self, _event: crate::app::gui_library::GuiGroupAlbumsEvent) {}

    /// Install one library's landed root list.
    fn install_emby_roots(
        &mut self,
        _lib_idx: usize,
        _items: crate::app::gui_library::GuiEmbyRootItems,
    ) {
    }

    /// Install one library's landed latest-episode fetch.
    fn install_shows_new(
        &mut self,
        _library_id: String,
        _request: crate::app::gui_library::GuiSectionRequest<mbv_emby_model::EmbyItem>,
    ) {
    }
}

/// The Client's store work one library event carried (`decouple-client-core`
/// design D4). The event's App-owned half runs first, so the store half is
/// collected here and applied afterwards, from the state that owns it: the
/// core never names that state.
#[derive(Debug)]
pub(in crate::app) enum ClientLibraryEventSeam {
    /// One group's album list landed.
    GroupAlbums(crate::app::gui_library::GuiGroupAlbumsEvent),
    /// One library's root list landed.
    EmbyRootItems {
        lib_idx: usize,
        items: crate::app::gui_library::GuiEmbyRootItems,
    },
    /// One library's latest-episode fetch landed.
    ShowsNew {
        library_id: String,
        request: crate::app::gui_library::GuiSectionRequest<mbv_emby_model::EmbyItem>,
    },
}

/// One piece of store work that a library event asks of the Client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::app) enum ClientLibraryEffect {
    /// The set of rows in view changed, so its artwork sources may need a
    /// fetch.
    RowArtSourcesChanged,
    /// A feed-view target's fetch returned, so its terminal failure mark
    /// retires.
    FeedTargetLanded {
        lib_idx: usize,
        selection: FeedHomeVideoSelection,
    },
    /// A feed-view target's fetch failed, so its terminal failure mark is
    /// recorded.
    FeedTargetFailed {
        lib_idx: usize,
        selection: FeedHomeVideoSelection,
    },
}
