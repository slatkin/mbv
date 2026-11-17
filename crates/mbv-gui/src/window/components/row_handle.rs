//! The mounted-handle recording the Queue menu cards' rows share: the
//! chooser's entries, the row menu's entries, the playback panel's target
//! choices, and the track chips' popover rows all report their regions
//! through a per-row `Signal` slot the parent panel reads, so the four
//! `onmounted` closures share one recorder and cannot drift apart.

use dioxus_native::prelude::*;

/// Record a mounted row's handle in `rows` at `index`, growing `rows` when
/// `index` sits past its end: the parent panel measures the rows through
/// these handles and reports their regions under its own report key.
pub(crate) fn record_row_handle(
    mut rows: Signal<Vec<Option<Event<MountedData>>>>,
    index: usize,
    event: Event<MountedData>,
) {
    let mut rows = rows.write();
    if rows.len() <= index {
        rows.resize(index + 1, None);
    }
    rows[index] = Some(event);
}
