use ratatui::backend::TestBackend;
use ratatui::layout::{Position, Rect};
use ratatui::Terminal;
use rstest::rstest;
use tuirealm::component::Component;

use crate::app::components::list::{
    ThreeLineFlatList, ThreeLineItem, ThreeLineRole, ThreeLineSpan,
};
use crate::app::palette;

fn item(target: u8) -> ThreeLineItem<u8> {
    ThreeLineItem::new(
        target,
        std::array::from_fn(|line| {
            vec![ThreeLineSpan::new(
                format!("item{target}-{line}"),
                ThreeLineRole::Name,
            )]
        }),
    )
}

#[rstest]
#[case(0, false)]
#[case(1, false)]
#[case(2, false)]
#[case(3, true)]
fn test_three_line_short_height_never_publishes_partial_cards(
    #[case] height: u16,
    #[case] visible: bool,
) {
    let mut list = ThreeLineFlatList::new(1);
    list.set_content(vec![item(9)]);
    let mut terminal = Terminal::new(TestBackend::new(12, 3)).unwrap();
    terminal
        .draw(|frame| list.view(frame, Rect::new(0, 0, 12, height)))
        .unwrap();
    assert_eq!(
        list.resolve_point(Position { x: 0, y: 0 }) == Some(&9),
        visible
    );
}

#[test]
fn test_three_line_buffer_selection_stripes_hits_and_gap() {
    let mut list = ThreeLineFlatList::new(1);
    list.set_content((0..5).map(item).collect());
    list.set_focused(true);
    list.select_target(&1);
    let mut terminal = Terminal::new(TestBackend::new(12, 7)).unwrap();
    terminal
        .draw(|frame| list.view(frame, Rect::new(0, 0, 12, 7)))
        .unwrap();
    let buffer = terminal.backend().buffer();

    assert_eq!(buffer[(0, 4)].bg, palette::SELECTED_ROW_BG);
    assert_eq!(buffer[(0, 5)].bg, palette::SELECTED_ROW_BG);
    assert_eq!(buffer[(0, 6)].bg, palette::SELECTED_ROW_BG);
    assert_eq!(
        buffer[(0, 3)].bg,
        palette::surface_colors(palette::Surface::SidebarBody, false).fill
    );
    assert_eq!(list.resolve_point(Position { x: 1, y: 3 }), None);
    for y in 4..=6 {
        assert_eq!(list.resolve_point(Position { x: 1, y }), Some(&1));
    }

    list.set_gap(2);
    assert_eq!(list.resolve_point(Position { x: 1, y: 4 }), None);
    terminal
        .draw(|frame| list.view(frame, Rect::new(0, 0, 12, 7)))
        .unwrap();
    for y in 0..=2 {
        assert_eq!(list.resolve_point(Position { x: 1, y }), Some(&1));
    }
    assert_eq!(list.resolve_point(Position { x: 1, y: 3 }), None);
}

#[test]
fn test_three_line_buffer_stripes_follow_flow_position_after_scroll() {
    let mut list = ThreeLineFlatList::new(1);
    list.set_content((0..6).map(item).collect());
    list.set_focused(true);
    let mut terminal = Terminal::new(TestBackend::new(12, 7)).unwrap();
    list.select_target(&4);
    terminal
        .draw(|frame| list.view(frame, Rect::new(0, 0, 12, 7)))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let stripe = palette::SESSIONS_STRIPE_BG;
    let surface = palette::surface_colors(palette::Surface::SidebarBody, false).fill;
    assert_eq!(buffer[(0, 0)].bg, stripe);
    assert_eq!(buffer[(0, 4)].bg, palette::SELECTED_ROW_BG);
    assert_eq!(buffer[(0, 3)].bg, surface);
}
