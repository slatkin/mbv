use super::test_helpers::*;
use super::*;
use crate::app::tests::make_item;
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
fn power_queue_title_and_scope_pills_stay_outside_panel() {
    let mut app = make_power_remote_queue_app();
    app.use_nerd_fonts = false;

    let (term, layout) = render_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let header = out
        .lines()
        .nth(layout.queue_scope_local_area.y as usize)
        .expect("expected queue header row");
    let device_name = mbv_core::api::device_name();
    let upper_device_name = device_name.to_uppercase();

    assert!(layout.queue_scope_local_area.y < layout.queue_area.y);
    assert!(layout.queue_scope_remote_area.y < layout.queue_area.y);
    assert!(layout.queue_scope_remote_area.x > layout.queue_scope_local_area.x);
    assert_eq!(
        layout.queue_scope_local_area.width + layout.queue_scope_remote_area.width,
        layout.queue_area.width
    );
    assert!(
        header.matches(&upper_device_name).count() >= 2,
        "expected local and remote queue controls to use session-style hostname pills:\n{out}"
    );
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
    let device_name = mbv_core::api::device_name();
    let upper_device_name = device_name.to_uppercase();

    assert!(
        header.contains(&upper_device_name),
        "expected session hostname pill in queue header:\n{out}"
    );
    assert!(
        !header.contains("Road Mix") && !header.contains("none"),
        "expected playlist pill to stay out of queue header:\n{out}"
    );
}

#[test]
fn bottom_status_bar_shows_playlist_pill_when_queue_is_a_playlist() {
    let mut app = make_power_queue_app(2);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".into()),
        name: "Road Mix".into(),
    };

    let (term, _layout) = render_view_to_terminal(&mut app, 100, 28);
    let out = buffer_to_string(&term);
    let last_line = out.lines().last().unwrap_or_default();

    assert!(
        last_line.contains("Road Mix"),
        "expected the playlist pill to appear in the main status bar:\n{last_line}"
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
fn power_queue_panel_counts_wrapped_group_headers_before_adding_padding() {
    let mut app = make_power_movie_app();
    app.panel_focus = PanelFocus::Queue;
    let mut items = Vec::new();
    for i in 0..3 {
        let mut item = make_item("Track", "Audio");
        item.id = format!("boundary-track-{i}");
        item.album_id = "boundary-album".into();
        item.album = "Long Album Title".into();
        item.artist = "Very Long Artist".into();
        items.push(item);
    }
    app.player_tab.set_items(items, 0);

    let panel_area = Rect::new(0, 0, 20, 6);
    let backend = TestBackend::new(panel_area.width, panel_area.height);
    let mut term = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    term.draw(|f| {
        let queue_area = render_power_queue_panel_frame(f, panel_area, true);
        app.render_power_queue(f, queue_area, true, &mut layout);
    })
    .unwrap();
    let out = buffer_to_string(&term);

    assert_eq!(layout.queue_area.y, 1);
    assert_eq!(layout.queue_area.height, 4);
    assert!(
        layout.queue_row_map.contains(&Some(0)),
        "expected selected track row to be mapped as visible after wrapped header: {:?}",
        layout.queue_row_map
    );
    assert!(
        out.contains("1. Tra"),
        "expected selected track to remain visible below the wrapped group header:\n{out}"
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
