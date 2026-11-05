//! Ordered stable-target membership (moved with the seam from
//! `mbv-components`).

use mbv_surfaces::list::{MarkSelection, MarkSelectionState, Row, RowFlow};

#[derive(Default)]
struct TestMarks {
    marks: MarkSelectionState<u8>,
}

impl MarkSelection<u8> for TestMarks {
    fn mark_selection(&self) -> &MarkSelectionState<u8> {
        &self.marks
    }

    fn mark_selection_mut(&mut self) -> &mut MarkSelectionState<u8> {
        &mut self.marks
    }
}

#[test]
fn interleaved_addition_and_removal_preserves_membership_order() {
    let mut marks = TestMarks::default();
    assert!(marks.add_mark(4));
    assert!(marks.add_mark(1));
    assert!(marks.add_mark(3));
    assert!(marks.remove_mark(&1));
    assert!(marks.add_mark(2));
    assert_eq!(marks.marks.targets(), &[4, 3, 2]);
}

#[test]
fn action_targets_follow_display_order_not_click_order() {
    let mut marks = TestMarks::default();
    marks.add_mark(3);
    marks.add_mark(1);
    marks.add_mark(4);
    let flow = RowFlow::new(vec![
        Row::selectable(4),
        Row::structural(),
        Row::selectable(1),
        Row::selectable(3),
    ]);

    assert_eq!(marks.marks.targets(), &[3, 1, 4]);
    assert_eq!(marks.action_targets(&flow), vec![4, 1, 3]);
    assert_eq!(marks.marks.targets(), &[3, 1, 4]);
}
