use super::test_helpers::*;
use super::*;
use crate::app::tests::make_item;
use crate::app::QueueScope;
use ratatui::backend::TestBackend;
use ratatui::Terminal;

#[test]
fn uses_configured_left_column_width() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 55;

    let layout = render_view(&mut app, 100, 28);

    assert_eq!(layout.queue_area.width, 51);
}

#[test]
fn collapsed_power_left_column_gives_library_full_width() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 55;
    app.queue_column_collapsed = true;

    let layout = render_view(&mut app, 100, 28);

    assert_eq!(layout.queue_area, Rect::default());
    assert_eq!(layout.left_area.x, 0);
    assert_eq!(layout.left_area.width, 99);
}

#[test]
fn short_window_keeps_queue_in_left_column() {
    let mut app = make_power_movie_app();
    app.queue_column_width = 40;

    let layout = render_view(&mut app, 100, 12);

    assert!(
        layout.queue_area.x < app.queue_column_width,
        "expected short-height queue to stay in the left column, got {:?}",
        layout.queue_area
    );
    assert!(
        layout.left_area.x >= app.queue_column_width,
        "expected library area to remain in the right column, got {:?}",
        layout.left_area
    );
}

#[test]
fn power_queue_panel_fills_remaining_left_column_with_short_queue() {
    let mut app = make_power_queue_app(1);

    let (_term, layout) = render_view_to_terminal(&mut app, 100, 28);
    let bottom_y = layout.queue_area.y + layout.queue_area.height;

    assert_eq!(bottom_y, 26);
    assert!(
        layout.queue_area.height > 1,
        "expected queue viewport inside full-height panel, got {:?}",
        layout.queue_area
    );
}

#[test]
fn power_queue_panel_empty_state_is_inside_panel() {
    let mut app = make_power_queue_app(0);

    let (term, layout) = render_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let empty_y = out
        .lines()
        .position(|line| line.contains("Add items with p"))
        .expect("expected queue empty-state message");

    assert_eq!(empty_y as u16, layout.queue_area.y);
}

#[test]
fn power_queue_title_does_not_render_playlist_pill() {
    let mut app = make_power_remote_queue_app();
    app.queue_source = crate::config::QueueSource::Playlist {
        id: None,
        name: "Road Mix".into(),
    };

    let (term, layout) = render_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let header = out
        .lines()
        .nth(layout.queue_scope_local_area.y as usize)
        .expect("expected queue header row");
    assert!(
        header.contains("CONNECTED:"),
        "expected connected status in local queue pill:\n{out}"
    );
    assert!(
        !header.contains("Road Mix") && !header.contains("none"),
        "expected playlist pill to stay out of queue header:\n{out}"
    );
}

#[test]
fn remote_queue_header_styles_scope_pills_by_connection_state() {
    let mut app = make_power_remote_queue_app();
    app.use_nerd_fonts = false;
    app.queue_scope = QueueScope::Local;

    let (term, layout) = render_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let header = out
        .lines()
        .nth(layout.queue_scope_local_area.y as usize)
        .expect("expected queue header row");

    assert!(
        !header.contains("CONNECTED:  "),
        "expected one space after Connected:\n{out}"
    );
    assert_eq!(
        layout.queue_scope_remote_area.width,
        layout.queue_area.width - layout.queue_scope_local_area.width,
        "green remote pill should fill the remaining header width"
    );
}

#[test]
fn short_power_queue_panel_drops_padding_before_rows() {
    let mut app = make_power_queue_app(20);

    let (_term, layout) = render_view_to_terminal(&mut app, 100, 12);

    assert!(
        layout.queue_area.height >= 1,
        "expected at least one usable queue row on a short terminal, got {:?}",
        layout.queue_area
    );
}

#[test]
fn power_queue_panel_preserves_group_aware_scrolling() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;

    let mut items = Vec::new();
    for i in 0..4 {
        let mut item = make_item(&format!("A{i}"), "Audio");
        item.id = format!("a-{i}");
        item.album_id = "album-a".into();
        item.album = "Album A".into();
        item.artist = "Artist".into();
        items.push(item);
    }
    for i in 0..4 {
        let mut item = make_item(&format!("B{i}"), "Audio");
        item.id = format!("b-{i}");
        item.album_id = "album-b".into();
        item.album = "Album B".into();
        item.artist = "Artist".into();
        items.push(item);
    }
    app.player_tab.set_items(items, 4);
    app.queue_scroll = 9;

    let (_term, _layout) = render_view_to_terminal(&mut app, 100, 20);

    assert_eq!(app.queue_scroll, 9);
}
