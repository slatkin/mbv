//! Shared mechanics for embedded ordered media-list row flows.
//!
//! This module is intentionally independent of both concrete list shapes and
//! of the tree-list widget crate. Adapters retain their own state and implement
//! only the primitive hooks required by the composed traits.
//!
//! Shared mechanics are kept explicit: production callers use the seam, and
//! declarations without a production consumer are removed rather than hidden
//! behind lint suppressions.

mod cursor;
mod expandable;
mod marks;
mod paint;
mod row_flow;
pub mod three_line;
pub mod tree_browser;
mod viewport;

pub use self::cursor::Cursored;
pub use self::expandable::{AggregateMarkState, Expandable};
pub use self::marks::{MarkSelection, MarkSelectionState};
pub use self::paint::{PaintRetained, PaintRetainedState};
pub use self::row_flow::{Row, RowFlow};
pub use self::three_line::{ThreeLineFlatList, ThreeLineItem, ThreeLineRole, ThreeLineSpan};
pub use self::viewport::{PagingPolicy, Viewported};

#[cfg(test)]
#[derive(Default)]
pub(super) struct TestListState {
    pub(super) selected: Option<u8>,
    pub(super) offset: usize,
}

#[cfg(test)]
impl Cursored<u8> for TestListState {
    fn selected_target(&self) -> Option<&u8> {
        self.selected.as_ref()
    }

    fn set_selected_target(&mut self, target: Option<&u8>) {
        self.selected = target.copied();
    }
}

#[cfg(test)]
impl Viewported<u8> for TestListState {
    fn viewport_offset(&self) -> usize {
        self.offset
    }

    fn set_viewport_offset(&mut self, offset: usize) {
        self.offset = offset;
    }
}

#[cfg(test)]
mod tests {
    // Keep a module at the seam root so the focused nextest filter also
    // exercises the public composition boundary, not only private submodules.
    use super::{
        Cursored, MarkSelectionState, PaintRetainedState, Row, RowFlow, TestListState, Viewported,
    };

    #[derive(PartialEq, Eq)]
    struct NonDefaultTarget;

    #[test]
    fn carriers_default_without_a_target_default_bound() {
        let _: MarkSelectionState<NonDefaultTarget> = Default::default();
        let _: PaintRetainedState<NonDefaultTarget> = Default::default();
        let _ = MarkSelectionState::<NonDefaultTarget>::new();
        let _ = PaintRetainedState::<NonDefaultTarget>::new();
    }

    #[test]
    fn composed_cursor_and_viewport_share_the_same_flow_positions() {
        let flow = RowFlow::new(vec![
            Row::structural(),
            Row::selectable(1),
            Row::structural(),
            Row::selectable(2),
        ]);
        let mut state = TestListState::default();

        assert_eq!(state.first(&flow), Some(1));
        assert_eq!(state.index(&flow), Some(1));
        assert_eq!(state.keep_cursor_visible(&flow, 1), 1);
        assert_eq!(state.offset, 1);
    }
}
