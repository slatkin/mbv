//! Shared test shapes for the list seam (moved with the seam from
//! `mbv-components`).

use mbv_surfaces::list::{Cursored, ViewportAnchor, Viewported};

/// A minimal list shape over `u8` targets used by the seam tests.
#[derive(Default, Debug)]
pub struct TestListState {
    pub selected: Option<u8>,
    pub offset: usize,
    pub anchor: ViewportAnchor,
}

impl Cursored<u8> for TestListState {
    fn selected_target(&self) -> Option<&u8> {
        self.selected.as_ref()
    }

    fn set_selected_target(&mut self, target: Option<&u8>) {
        self.selected = target.copied();
    }
}

impl Viewported<u8> for TestListState {
    fn viewport_offset(&self) -> usize {
        self.offset
    }

    fn set_viewport_offset(&mut self, offset: usize) {
        self.offset = offset;
    }

    fn viewport_anchor(&self) -> ViewportAnchor {
        self.anchor
    }

    fn set_viewport_anchor(&mut self, anchor: ViewportAnchor) {
        self.anchor = anchor;
    }
}
