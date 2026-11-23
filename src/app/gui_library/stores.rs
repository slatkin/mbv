//! The GUI Library's transitional `ClientLibraryStores` implementor
//! (`decouple-client-core` design D4a (c)): the `gui_*` store fields still
//! live on `App`, so this unit implementor reads and writes them through the
//! `app` parameter the core seam passes. It is the one Client that keeps
//! library stores (the TUI passes `None`), so the GUI's library event drain
//! hands it to every event.

use crate::app::dispatch::library::stores::{ClientLibraryEffect, ClientLibraryStores};
use crate::app::{App, FeedHomeVideoSelection};

/// The GUI Client's library stores, as the core's library event seam takes
/// them (design D4a (c)).
#[derive(Debug, Default)]
pub struct GuiLibraryStores;

impl ClientLibraryStores for GuiLibraryStores {
    fn wants_feed_target(
        &self,
        app: &App,
        lib_idx: usize,
        target: &FeedHomeVideoSelection,
    ) -> bool {
        app.gui_emby_feed_wants_target(lib_idx, target)
    }

    fn apply(&mut self, app: &mut App, effect: ClientLibraryEffect) {
        match effect {
            // The rows in view changed, so their artwork sources are stale:
            // the GUI re-queues the artwork the in-view rows lack (the call
            // this effect replaced, design D4a's table).
            ClientLibraryEffect::RowArtSourcesChanged => {
                app.gui_queue_in_view_row_fetches();
            }
            // A landed target retires its terminal failure mark, so a later
            // cache miss can fetch it again.
            ClientLibraryEffect::FeedTargetLanded { lib_idx, selection } => {
                if let Some(claim) = app
                    .gui_emby_libs
                    .get_mut(&lib_idx)
                    .and_then(|stored| stored.feed_claim.as_mut())
                {
                    claim.failed.remove(&selection);
                }
            }
            // A failed target records the mark, so the shown pass skips it
            // until a user press clears it and no repaint retries it.
            ClientLibraryEffect::FeedTargetFailed { lib_idx, selection } => {
                if let Some(claim) = app
                    .gui_emby_libs
                    .get_mut(&lib_idx)
                    .and_then(|stored| stored.feed_claim.as_mut())
                {
                    claim.failed.insert(selection);
                }
            }
        }
    }
}
