//! The library-event seam a Client uses to keep its own library stores
//! (`decouple-client-core` design D4a (c)). The library event handlers run in
//! the core; what a landed or failed fetch means for a Client's own stores is
//! the Client's business. The handlers take an optional `ClientLibraryStores`,
//! raise [`ClientLibraryEffect`] values, and apply them through it after the
//! event that raised them. A Client that keeps no such stores (the TUI) passes
//! `None` and nothing happens, as today.

use crate::app::{App, FeedHomeVideoSelection};

/// Take one pending library event and apply it through `stores`, the Client's
/// own stores (`decouple-client-core` design D4a (c)). The seam types stay
/// neutral, so the GUI Client calls this instead of
/// `App::drain_lib_event_with_stores` and the core names no Client's state.
pub fn drain_lib_event_through(app: &mut App, stores: &mut dyn ClientLibraryStores) -> bool {
    app.drain_lib_event_with_stores(Some(stores))
}

/// What one Client does with the effects a library event raises. The
/// implementor (a Client's own state) reads and writes its own fields; `app`
/// is the shared core state, which the effect may also touch (the in-view row
/// artwork fetch reads the core's catalog).
pub trait ClientLibraryStores {
    /// Whether this Client's own selection still wants the landed feed
    /// target, beside the shared selection the core keeps.
    fn wants_feed_target(&self, app: &App, lib_idx: usize, target: &FeedHomeVideoSelection)
    -> bool;

    /// Apply one effect that a library event raised.
    fn apply(&mut self, app: &mut App, effect: ClientLibraryEffect);
}

/// One piece of store work that a library event asks of the Client.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientLibraryEffect {
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
