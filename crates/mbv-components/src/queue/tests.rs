use super::*;
use mbv_render::arrangements::queue::queue_list_box;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use mbv_queue::QueueItem;
use mbv_theme as palette;
use mbv_theme::Surface;
use mbv_theme::surface_colors;

fn feed_slot(id: u64, title: &str) -> QueueSlot {
    QueueSlot {
        slot_id: QueueSlotId::from_raw(id),
        item: QueueItem::Feed(mbv_queue::FeedEntry {
            guid: format!("guid-{id}"),
            title: title.to_string(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: None,
            pub_date_secs: None,
            feed_kind: None,
            feed_id: None,
            position_ticks: 0,
            played: false,
        }),
    }
}

fn draw_mini_queue(frame_focused: bool) -> ratatui::buffer::Buffer {
    let mut component = QueueComponent::new();
    component.set_rows(
        &[
            feed_slot(1, "first"),
            feed_slot(2, "second"),
            feed_slot(3, "third"),
        ],
        PlaybackState::default(),
    );
    component.set_cursor(&QueueCursorUpdate::Set(0));
    component.attr(Attribute::Focus, AttrValue::Flag(true));
    component.set_mini_view(true);
    component.set_frame_focused(frame_focused);

    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 24)))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn count_bg(buf: &ratatui::buffer::Buffer, area: Rect, color: ratatui::style::Color) -> usize {
    (area.y..area.bottom())
        .flat_map(|y| (area.x..area.right()).map(move |x| (x, y)))
        .filter(|&(x, y)| buf[(x, y)].style().bg == Some(color))
        .count()
}

/// Mini-view body palette: with the projected
/// appearance bit focused, the rows paint exactly the wide focused palette --
/// the selected row's opaque bar and the focused zebra stripe -- so the
/// focused mini panel is the wide focused panel, not a third style. The
/// focused zebra tone can appear inside the list box only through a zebra
/// stripe (the box's own fill is the `QueuePanel` base), and the Iris bar
/// paints only from the paint policy's focused bit.
#[test]
fn mini_view_rows_paint_the_focused_palette_when_the_appearance_bit_is_focused() {
    let buf = draw_mini_queue(true);

    let focused_zebra = surface_colors(Surface::QueueColumn, true).fill;
    let row_area = queue_list_box(Rect::new(0, 0, 60, 24));
    assert!(
        count_bg(&buf, row_area, focused_zebra) > 0,
        "the focused zebra stripe painted"
    );
    assert!(
        count_bg(&buf, row_area, palette::SELECTED_ROW_BG) > 0,
        "the selected row bar painted"
    );
}

/// The other side of the contract: with the appearance bit resting (unpinned
/// mini view), the body rows keep the resting palette -- no focused zebra --
/// even though the component holds interaction focus. The selected row is the
/// exception to the palette suppression: its canonical cursor bar stays
/// visible on the resting stripe (the same contract the app-level
/// `panel_focus` proof asserts for the composed frame).
#[test]
fn mini_view_rows_rest_when_the_appearance_bit_is_resting() {
    let buf = draw_mini_queue(false);

    let focused_zebra = surface_colors(Surface::QueueColumn, true).fill;
    let row_area = queue_list_box(Rect::new(0, 0, 60, 24));
    assert_eq!(
        count_bg(&buf, row_area, focused_zebra),
        0,
        "focused zebra tone while resting"
    );
    assert!(
        count_bg(&buf, row_area, palette::SELECTED_ROW_BG) > 0,
        "the cursor bar stayed on the resting stripe"
    );
}
