use super::*;
use mbv_render::arrangements::queue::queue_list_box;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tuirealm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use mbv_queue::QueueItem;
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
        count_bg(
            &buf,
            row_area,
            surface_colors(Surface::SelectedRow, false).fill
        ) > 0,
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
        count_bg(
            &buf,
            row_area,
            surface_colors(Surface::SelectedRow, false).fill
        ) > 0,
        "the cursor bar stayed on the resting stripe"
    );
}

fn left_down(column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn painted_footer_component() -> QueueComponent {
    let mut component = QueueComponent::new();
    component.set_rows(&[], PlaybackState::default());
    component.set_scope(QueueScope::Local);
    component.set_status_pills(vec![Span::raw("X"), Span::raw("name")], None, None);
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|frame| component.view(frame, Rect::new(0, 0, 60, 24)))
        .unwrap();
    component
}

/// Clicking the playlist pill opens the F4 Playlists sidebar.
#[test]
fn click_on_the_playlist_pill_opens_playlists() {
    let mut component = painted_footer_component();
    let hit = component.playlist_hit_for_test();
    assert!(hit.width > 0, "the playlist pill keeps a hit area");
    assert_eq!(
        component.handle_mouse(left_down(hit.x, hit.y)),
        Some(Msg::Shell(Box::new(
            mbv_ui_msg::ShellRequest::OpenPlaylists
        ))),
        "clicking the playlist pill opens Playlists"
    );
}

/// A second press on the playlist pill inside the double-click window is
/// ignored: a double-click opens once instead of toggling closed.
#[test]
fn double_click_on_the_playlist_pill_ignores_the_second_press() {
    let mut component = painted_footer_component();
    let hit = component.playlist_hit_for_test();
    let _ = component.handle_mouse(left_down(hit.x, hit.y));
    assert_eq!(
        component.handle_mouse(left_down(hit.x, hit.y)),
        None,
        "the double-click press does not toggle Playlists closed"
    );
}

/// Regression (wheel-scrolls-viewport 5.1, design.md D5): a `Preserve`
/// projection push after a wheel scroll leaves the freely scrolled Queue
/// offset where the wheel left it. The scroll-to-cursor clamp runs only in
/// the authoritative `Set` arm.
#[test]
fn a_preserve_push_after_a_wheel_scroll_keeps_the_scrolled_offset() {
    let mut component = QueueComponent::new();
    component.set_rows(
        &[
            feed_slot(1, "first"),
            feed_slot(2, "second"),
            feed_slot(3, "third"),
            feed_slot(4, "fourth"),
            feed_slot(5, "fifth"),
        ],
        PlaybackState::default(),
    );
    component.set_cursor(&QueueCursorUpdate::Set(0));

    component.delegate_row_local_input(
        MediaListSurfaceInput::Wheel {
            at: ratatui::layout::Position { x: 0, y: 0 },
            delta: 3,
        },
        None,
    );

    assert_eq!(
        component.test_cursor(),
        0,
        "the wheel never moves selection"
    );
    assert_eq!(
        component.test_scroll(),
        3,
        "the wheel moved the free offset"
    );

    component.set_cursor(&QueueCursorUpdate::Preserve);

    assert_eq!(
        component.test_scroll(),
        3,
        "Preserve leaves the wheel-scrolled offset untouched"
    );
    assert_eq!(component.test_cursor(), 0);
}
