use ratatui::layout::Position;
use rstest::rstest;

use super::{ThreeLineFlatList, ThreeLineItem, ThreeLineRole, ThreeLineSpan};

fn item(target: u8) -> ThreeLineItem<u8> {
    ThreeLineItem::new(
        target,
        std::array::from_fn(|line| {
            vec![ThreeLineSpan::new(
                format!("{target}-{line}"),
                ThreeLineRole::Name,
            )]
        }),
    )
}

#[test]
fn test_three_line_selection_survives_reorder_and_replacement() {
    let mut list = ThreeLineFlatList::new(1);
    list.set_content(vec![item(1), item(2), item(3)]);
    assert!(list.select_target(&2));
    list.set_content(vec![item(3), item(2), item(1)]);
    assert_eq!(list.selected_target(), Some(&2));
    list.set_content(vec![item(3), item(1)]);
    assert_eq!(list.selected_target(), Some(&3));
    list.move_selection(1);
    assert_eq!(list.selected_target(), Some(&1));
}

#[rstest]
#[case(0, 0)]
#[case(1, 0)]
#[case(2, 0)]
#[case(3, 1)]
#[case(6, 1)]
#[case(7, 2)]
fn test_three_line_visible_item_capacity(#[case] height: u16, #[case] expected: usize) {
    let list: ThreeLineFlatList<u8> = ThreeLineFlatList::new(1);
    assert_eq!(list.gap(), 1);
    assert_eq!(list.items().len(), 0);
    assert_eq!(list.visible_items(height), expected);
}

#[test]
fn test_three_line_paint_invalidation_preserves_selection() {
    let mut list = ThreeLineFlatList::new(1);
    list.set_content(vec![item(1), item(2), item(3)]);
    let area = ratatui::layout::Rect::new(0, 0, 12, 7);
    let (offset, visible, _) = list.painting_parts(area);
    assert_eq!((offset, visible), (0, 2));
    list.publish(
        area,
        vec![
            (ratatui::layout::Rect::new(0, 0, 12, 3), 1),
            (ratatui::layout::Rect::new(0, 4, 12, 3), 2),
        ],
        None,
    );
    assert_eq!(list.resolve_point(Position { x: 2, y: 3 }), None);
    assert_eq!(list.resolve_point(Position { x: 2, y: 6 }), Some(&2));
    list.invalidate_paint();
    assert_eq!(list.resolve_point(Position { x: 2, y: 6 }), None);
    list.publish(
        area,
        vec![(ratatui::layout::Rect::new(0, 0, 12, 3), 1)],
        None,
    );
    assert_eq!(list.selected_target(), Some(&1));
    assert_eq!(list.painting_parts(area).1, 2);
}
