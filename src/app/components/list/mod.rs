//! Shared mechanics for embedded ordered media-list row flows.
//!
//! This module is intentionally independent of both concrete list shapes and
//! of `tui-treelistview`. Adapters retain their own state and implement only
//! the primitive hooks required by the composed traits.

mod cursor;
mod row_flow;
mod viewport;

pub use self::cursor::Cursored;
#[allow(unused_imports)]
pub use self::row_flow::{Row, RowFlow};
#[allow(unused_imports)]
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
    use super::{Cursored, Row, RowFlow, TestListState, Viewported};

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
