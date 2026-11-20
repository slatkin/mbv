//! The Library lists' scroll stores (`gui-change-driven-sections` design D9,
//! the port of `lib/library/scroll.ts`). A list's native scroll offset and its
//! measured viewport height live here, in a store of their own, apart from the
//! interaction state in [`super::state`]. A scroll step then moves only this
//! store: the rows a list derives from the content, the open nodes, the
//! filter, and the filter state keep their identity, and the list repaints
//! only its visible window.

use dioxus_native::prelude::*;

use super::list::{GeometryRow, clamp_scroll_top};

/// One list's scroll position: the native offset in pixels, and the viewport
/// height the list measured (`0` until it measures).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ListScroll {
    /// The list's native scroll offset, in pixels.
    pub scroll_top: f32,
    /// The viewport height the list measured, in pixels.
    pub viewport_height: f32,
}

/// The state a list's scroll store starts with: unscrolled and unmeasured.
#[must_use]
pub fn initial_list_scroll() -> ListScroll {
    ListScroll::default()
}

/// Write a scroll offset into a list's store, held inside the list's range
/// (`gui-pointer-target-interaction` D6): the one settle a scroll key, a
/// measurement, and a native scroll report share, so the store never holds an
/// offset its rows cannot paint. An unchanged offset marks no subscriber.
pub fn set_scroll_top(store: Signal<ListScroll>, rows: &[impl GeometryRow], scroll_top: f32) {
    let held = *store.peek();
    let clamped = ListScroll {
        scroll_top: clamp_scroll_top(rows, scroll_top, held.viewport_height),
        viewport_height: held.viewport_height,
    };
    if clamped != held {
        let mut store = store;
        store.set(clamped);
    }
}

/// Return a list's scroll offset to the top, keeping its measured viewport
/// (gui-flat-list-sort design D7): a list whose sort changed starts at its
/// first row. An unchanged offset marks no subscriber.
pub fn reset_scroll_top(store: Signal<ListScroll>) {
    let held = *store.peek();
    if held.scroll_top == 0.0 {
        return;
    }
    let mut store = store;
    store.set(ListScroll {
        scroll_top: 0.0,
        viewport_height: held.viewport_height,
    });
}

/// The six lists' scroll stores (`scroll.ts`'s module-level stores): one
/// signal per destination's list. The root builds them with the interaction
/// state ([`super::state::LibraryState`]) and provides them as context.
#[derive(Clone)]
pub struct LibraryScrolls {
    /// The Shows list's scroll store.
    pub shows: Signal<ListScroll>,
    /// The Audiobooks list's scroll store.
    pub audiobooks: Signal<ListScroll>,
    /// The Home list's scroll store.
    pub home: Signal<ListScroll>,
    /// The Emby library list's scroll store.
    pub emby: Signal<ListScroll>,
    /// The Feeds list's scroll store.
    pub feeds: Signal<ListScroll>,
    /// The Podcasts list's scroll store.
    pub podcasts: Signal<ListScroll>,
}

impl Default for LibraryScrolls {
    fn default() -> Self {
        Self::new()
    }
}

impl LibraryScrolls {
    /// The scroll stores a window start builds: all unscrolled and unmeasured.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shows: Signal::new(initial_list_scroll()),
            audiobooks: Signal::new(initial_list_scroll()),
            home: Signal::new(initial_list_scroll()),
            emby: Signal::new(initial_list_scroll()),
            feeds: Signal::new(initial_list_scroll()),
            podcasts: Signal::new(initial_list_scroll()),
        }
    }
}

impl std::fmt::Debug for LibraryScrolls {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LibraryScrolls")
            .field("shows", &self.shows.peek().clone())
            .finish_non_exhaustive()
    }
}
