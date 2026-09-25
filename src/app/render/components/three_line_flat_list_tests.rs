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
fn test_three_line_buffer_selection_stripes_and_hits() {
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
}

#[test]
fn test_three_line_buffer_selected_bar_spans_claim_rect() {
    // Like the tree browser and wide media list, the selected row's bar
    // reaches the owning panel's edges while zebra rows stay inside the
    // content inset; hit geometry mirrors the painted rows.
    let mut list = ThreeLineFlatList::new(1);
    list.set_content((0..3).map(item).collect());
    list.set_focused(true);
    list.select_target(&1);
    let claim = Rect::new(0, 0, 14, 7);
    let content = Rect::new(2, 0, 10, 7);
    let mut terminal = Terminal::new(TestBackend::new(14, 7)).unwrap();
    terminal
        .draw(|frame| list.view_in(frame, claim, content))
        .unwrap();
    let buffer = terminal.backend().buffer();
    let surface = palette::surface_colors(palette::Surface::SidebarBody, false).fill;

    // The selected bar spans the full claim width on all three lines.
    for x in [0, 1, 2, 13] {
        for y in 4..=6 {
            assert_eq!(buffer[(x, y)].bg, palette::SELECTED_ROW_BG, "x={x} y={y}");
        }
    }
    // The selected row's text still starts at the content inset, not the band.
    assert_eq!(buffer[(2, 4)].symbol(), "i");
    // Non-selected rows stay inset: the band next to item 0 is untouched.
    assert_eq!(buffer[(0, 0)].bg, ratatui::style::Color::Reset);
    assert_eq!(buffer[(2, 0)].bg, surface);
    // Hit geometry mirrors the paint: the selected row's band resolves, the
    // non-selected row's band does not.
    assert_eq!(list.resolve_point(Position { x: 0, y: 5 }), Some(&1));
    assert_eq!(list.resolve_point(Position { x: 0, y: 1 }), None);
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
