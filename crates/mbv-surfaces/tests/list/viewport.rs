//! Viewport arithmetic over the shared row flow (moved with the seam from
//! `mbv-components`).

use rstest::rstest;

use super::helpers::TestListState;
use mbv_surfaces::list::{Cursored, PagingPolicy, Row, RowFlow, ViewportAnchor, Viewported};

/// A shape that opts into the leading-structural-raise policy (#731).
#[derive(Default)]
struct RaisingListState {
    selected: Option<u8>,
    offset: usize,
    anchor: ViewportAnchor,
}

impl Cursored<u8> for RaisingListState {
    fn selected_target(&self) -> Option<&u8> {
        self.selected.as_ref()
    }

    fn set_selected_target(&mut self, target: Option<&u8>) {
        self.selected = target.copied();
    }
}

impl Viewported<u8> for RaisingListState {
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

    fn raise_over_leading_structural_rows(&self) -> bool {
        true
    }
}

fn flow() -> RowFlow<u8> {
    RowFlow::new(vec![
        Row::selectable(1),
        Row::structural(),
        Row::selectable(2),
        Row::selectable(3),
        Row::structural(),
        Row::selectable(4),
        Row::selectable(5),
        Row::selectable(6),
    ])
}

#[test]
fn raising_over_structural_rows_keeps_the_group_heading_visible() {
    let rows = RowFlow::new(vec![
        Row::selectable(1),
        Row::structural(),
        Row::selectable(2),
        Row::selectable(3),
        Row::selectable(4),
        Row::selectable(5),
        Row::selectable(6),
    ]);
    let mut list = RaisingListState {
        selected: Some(2),
        offset: 4,
        ..Default::default()
    };

    assert_eq!(list.keep_cursor_visible(&rows, 3), 1);
    assert_eq!(list.offset, 1);
}

#[test]
fn preserves_offset_when_selection_is_visible() {
    let rows = flow();
    let mut list = TestListState {
        selected: Some(3),
        offset: 2,
        ..Default::default()
    };

    assert_eq!(list.keep_cursor_visible(&rows, 4), 2);
    assert_eq!(list.offset, 2);
}

#[rstest]
#[case(Some(1), 4, 0)]
#[case(Some(7), 3, 5)]
#[case(Some(2), 2, 2)]
fn scrolls_minimally_and_clamps_geometry(
    #[case] selected: Option<u8>,
    #[case] viewport_len: usize,
    #[case] expected_offset: usize,
) {
    let rows = flow();
    let mut list = TestListState {
        selected,
        offset: usize::MAX,
        ..Default::default()
    };

    assert_eq!(
        list.reconcile_viewport(&rows, viewport_len),
        expected_offset
    );
    assert_eq!(list.offset, expected_offset);
}

#[test]
fn geometry_clamping_preserves_offset_when_bounds_allow() {
    let rows = flow();
    let mut list = TestListState {
        selected: Some(2),
        offset: 3,
        ..Default::default()
    };

    assert_eq!(list.clamp_viewport(&rows, 3), 3);
    assert_eq!(list.offset, 3);
    assert_eq!(list.clamp_viewport(&rows, 99), 0);
    assert_eq!(list.offset, 0);
}

#[rstest]
#[case(-1, Some(6), 5)]
#[case(1, Some(1), 0)]
fn visible_viewport_paging_recovers_missing_selection_and_keeps_it_visible(
    #[case] direction: isize,
    #[case] expected_selected: Option<u8>,
    #[case] expected_offset: usize,
) {
    let rows = flow();
    let mut list = TestListState {
        selected: None,
        offset: 4,
        ..Default::default()
    };

    let selected = list.page(&rows, 3, direction, PagingPolicy::VisibleViewport);

    assert_eq!(selected, Some(if direction < 0 { 7 } else { 0 }));
    assert_eq!(list.selected, expected_selected);
    assert_eq!(list.offset, expected_offset);
}

#[rstest]
#[case(3, 5, 5)]
#[case(7, 1, 1)]
fn a_freely_scrolled_viewport_resolves_without_the_selection_pull(
    #[case] selection: u8,
    #[case] delta: i64,
    #[case] expected_offset: usize,
) {
    let rows = flow();
    let mut list = TestListState {
        selected: Some(selection),
        offset: 0,
        ..Default::default()
    };

    list.scroll_viewport(&rows, 3, delta);

    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
    assert_eq!(
        list.resolved_viewport_offset(&rows, 3, Some(usize::from(selection))),
        expected_offset
    );
}

#[rstest]
#[case(100, 5)]
#[case(-100, 0)]
#[case(2, 2)]
fn scroll_viewport_clamps_at_flow_bounds_and_releases_following(
    #[case] delta: i64,
    #[case] expected_offset: usize,
) {
    let rows = flow();
    let mut list = TestListState::default();

    assert_eq!(list.scroll_viewport(&rows, 3, delta), expected_offset);
    assert_eq!(list.offset, expected_offset);
    assert_eq!(list.viewport_anchor(), ViewportAnchor::Free);
}

/// Moved with the seam from `mbv-components` (`list.rs`): it exercises the
/// composed cursor-and-viewport boundary over the public seam API.
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
