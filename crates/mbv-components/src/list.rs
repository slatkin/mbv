//! Shared mechanics for embedded ordered media-list row flows.
//!
//! This module is intentionally independent of both concrete list shapes and
//! of the tree-list widget crate. Adapters retain their own state and implement
//! only the primitive hooks required by the composed traits.
//!
//! The cursor, viewport, row-flow, and mark mechanics are front-end-neutral
//! Surface state and live in `mbv-surfaces` (add-gui-front-end design D2/D5);
//! this module re-exports them for the TUI adapters. Shared mechanics are kept
//! explicit: production callers use the seam, and declarations without a
//! production consumer are removed rather than hidden behind lint suppressions.

mod paint;
pub mod three_line;
pub mod tree_browser;

#[doc(inline)]
pub use self::paint::{PaintRetained, PaintRetainedState};
#[doc(inline)]
pub use self::three_line::ThreeLineFlatList;
#[doc(inline)]
pub use mbv_surfaces::list::{
    AggregateMarkState, Cursored, MarkSelection, MarkSelectionState, PagingPolicy, Row, RowFlow,
    ViewportAnchor, Viewported, max_viewport_offset_for_len, scrolled_viewport_offset,
};
