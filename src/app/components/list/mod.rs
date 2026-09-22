//! Shared mechanics for embedded ordered media-list row flows.
//!
//! This module is intentionally independent of both concrete list shapes and
//! of `tui-treelistview`. Adapters retain their own state and implement only
//! the primitive hooks required by the composed traits.

mod cursor;
mod row_flow;
mod viewport;

pub use self::cursor::Cursored;
pub use self::row_flow::{Row, RowFlow};
#[allow(unused_imports)]
pub use self::viewport::{PagingPolicy, Viewported};

#[cfg(test)]
mod tests {
    // Keep a module at the seam root so the focused nextest filter also
    // exercises the public composition boundary, not only private submodules.
    use super::{Cursored, Row, RowFlow, Viewported};

    #[derive(Default)]
    struct CursorViewport {
        selected: Option<u8>,
        offset: usize,
    }

    impl Cursored<u8> for CursorViewport {
        fn selected_target(&self) -> Option<&u8> {
            self.selected.as_ref()
        }

        fn set_selected_target(&mut self, target: Option<&u8>) {
            self.selected = target.copied();
        }
    }

    impl Viewported<u8> for CursorViewport {
        fn viewport_offset(&self) -> usize {
            self.offset
        }

        fn set_viewport_offset(&mut self, offset: usize) {
            self.offset = offset;
        }
    }

    #[test]
    fn composed_cursor_and_viewport_share_the_same_flow_positions() {
        let flow = RowFlow::new(vec![
            Row::structural(),
            Row::selectable(1),
            Row::structural(),
            Row::selectable(2),
        ]);
        let mut state = CursorViewport::default();

        assert_eq!(state.first(&flow), Some(1));
        assert_eq!(state.index(&flow), Some(1));
        assert_eq!(state.keep_cursor_visible(&flow, 1), 1);
        assert_eq!(state.offset, 1);
    }
}
