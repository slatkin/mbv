use super::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use mbv_queue::QueueItem;
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

/// Mini-view body palette (`pinned-mini-view-focus`): with the projected
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
    let mut zebra = 0;
    let mut selected = 0;
    for y in row_area.y..row_area.bottom() {
        for x in row_area.x..row_area.right() {
            let bg = buf[(x, y)].style().bg;
            zebra += u32::from(bg == Some(focused_zebra));
            selected += u32::from(bg == Some(palette::SELECTED_ROW_BG));
        }
    }
    assert!(zebra > 0, "the focused zebra stripe painted");
    assert!(selected > 0, "the selected row bar painted");
}

/// The other side of the contract: with the appearance bit resting (unpinned
/// mini view), the body rows keep the resting palette -- no focused zebra, no
/// selected bar -- even though the component holds interaction focus.
#[test]
fn mini_view_rows_rest_when_the_appearance_bit_is_resting() {
    let buf = draw_mini_queue(false);

    let focused_zebra = surface_colors(Surface::QueueColumn, true).fill;
    let row_area = queue_list_box(Rect::new(0, 0, 60, 24));
    for y in row_area.y..row_area.bottom() {
        for x in row_area.x..row_area.right() {
            let bg = buf[(x, y)].style().bg;
            assert_ne!(
                bg,
                Some(focused_zebra),
                "focused zebra tone at ({x}, {y}) while resting"
            );
            assert_ne!(
                bg,
                Some(palette::SELECTED_ROW_BG),
                "selected bar at ({x}, {y}) while resting"
            );
        }
    }
}
