//! Shared mechanics for embedded ordered media-list row flows.
//!
//! This module is intentionally independent of both concrete list shapes and
//! of the tree-list widget crate. Adapters retain their own state and implement
//! only the primitive hooks required by the composed traits.
//!
//! The cursor, viewport, row-flow, and mark mechanics are front-end-neutral
//! Surface state and live in `mbv-surfaces` (extract-client-core design D2/D5);
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

#[cfg(test)]
mod tests {
    // The seam-root module stays so the focused nextest filter exercises the
    // root boundary. Its cursor/viewport composition test moved with the seam
    // to `mbv-surfaces`; the mark-carrier half of the carrier-default test
    // moved with it, and this half stays with `PaintRetainedState`, which
    // never left `mbv-components`.
    use super::PaintRetainedState;

    #[derive(PartialEq, Eq)]
    struct NonDefaultTarget;

    #[test]
    fn carriers_default_without_a_target_default_bound() {
        let _: PaintRetainedState<NonDefaultTarget> = PaintRetainedState::default();
        let _ = PaintRetainedState::<NonDefaultTarget>::new();
    }
}
