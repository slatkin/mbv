use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_music_group_app;
use crate::app::tests::make_item;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};

/// The framed Wide track table is a second canonical control in Grouped Music:
/// its retained claim resolves click, double-click, right-click, and one-step
/// wheel input through a live Application tick without a parent row map.
#[test]
fn music_wide_track_table_uses_retained_geometry_for_live_mouse_gestures() {
    let mut app = make_music_group_app();
    let tracks = (0..3)
        .map(|index| {
            let mut track = make_item(&format!("Track {}", index + 1), "Audio");
            track.id = format!("track-{}", index + 1);
            track.index_number = index + 1;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let music_id = ComponentId::Library;

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().mouse_subscribed.contains(&music_id));
    let track_state = |harness: &TickHarness| {
        let music = harness.model().test_music_owner();
        (music.track_focused(), music.track_selected_row())
    };
    let (_track_point, second_track_point) = {
        let music = harness.model().test_music_owner();
        let content = music.track_list
            .current_content_rect()
            .expect("Wide track table retained its current content rect");
        let selected = music
            .track_list.current_selected_row_rect()
            .expect("Wide track table retained its selected row");
        (
            (selected.x, selected.y),
            (content.x, content.y.saturating_add(1)),
        )
    };
    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };
    let right_click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    // The first click focuses the second track from the child result. The
    // second click at the unchanged painted point becomes a double-click and
    // activates that same stable provider target.
    harness.inject(click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    }));
    assert_eq!(track_state(&harness), (true, Some(1)));
    harness.inject(click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicTrackActivate { .. })
    )));

    // Right-click resolves the same retained row and translates directly to
    // the track context intent; no shell-side coordinate lookup is involved.
    harness.inject(right_click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicRowContextMenu(_, Some((x, y)))) if *x == second_track_point.0 && *y == second_track_point.1
    )));

    // Wheel over the painted table advances exactly one local track row and
    // emits no shell-side cursor movement.
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: second_track_point.0,
        row: second_track_point.1,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert_eq!(track_state(&harness), (true, Some(2)));
    assert!(outcome
        .messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))));
}

/// Modifier clicks on the Wide Music track list are delivered through the
/// LibraryPanel surface gesture and retain the shared owner's selection.
#[test]
fn music_wide_track_modifier_clicks_toggle_range_and_plain_clear() {
    let mut app = make_music_group_app();
    let tracks = (0..3)
        .map(|index| {
            let mut track = make_item(&format!("Track {}", index + 1), "Audio");
            track.id = format!("track-{}", index + 1);
            track.index_number = index + 1;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let (x, y) = {
        let music = harness.model().test_music_owner();
        let content = music
            .track_list
            .current_content_rect()
            .expect("track table retained its content rect");
        (content.x, content.y)
    };
    let mouse = |column, row, modifiers| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers,
        })
    };

    harness.inject(mouse(x, y + 1, tuirealm::event::KeyModifiers::CONTROL));
    harness.step();
    assert_eq!(
        harness.model().test_music_owner().track_list.multi_selection(),
        &["track-1".to_string(), "track-2".to_string()]
    );

    harness.inject(mouse(x, y + 2, tuirealm::event::KeyModifiers::SHIFT));
    harness.step();
    assert_eq!(
        harness.model().test_music_owner().track_list.multi_selection(),
        &[
            "track-1".to_string(),
            "track-2".to_string(),
            "track-3".to_string()
        ]
    );

    harness.inject(mouse(
        x,
        y,
        tuirealm::event::KeyModifiers::NONE,
    ));
    harness.step();
    assert!(harness
        .model()
        .test_music_owner()
        .track_list
        .multi_selection()
        .is_empty());
}

/// Grouped Music resolves its artist-root and album-leaf clicks through the
/// tree's completed hit map, then rejects that map as soon as a new frame is
/// configured. The mounted path is the real LibraryPanel/Application tick;
/// there is no shell-side album row geometry to fall back to.
#[test]
fn music_tree_click_moves_focus_to_library_and_other_panel_click_does_not() {
    let mut app = make_music_group_app();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Queue;
    app.player_tab.set_items(vec![make_item("Queue Item", "Audio")], 0);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let tree_point = {
        let music = harness.model().test_music_owner();
        let node = music.browser.projected_nodes().first().expect("tree root");
        let row = music
            .browser
            .row_rect_for(node.id())
            .expect("painted tree row");
        (row.x, row.y)
    };
    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_list_rect())
        .expect("painted Music list area");
    assert!(list_area.contains(ratatui::layout::Position::new(tree_point.0, tree_point.1)));
    assert!(harness.model().mouse_subscribed.contains(&ComponentId::Library));
    harness.inject(click(tree_point.0, tree_point.1));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::LibraryPanelFocus))
            || matches!(message, Msg::Shell(ShellRequest::MusicArtistTracks { .. }))
    }), "tree click messages: {:?}", outcome.raw_messages);
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert_eq!(harness.model().app.effective_panel_focus(), PanelFocus::Library);

    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    let queue_point = harness
        .model()
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::QueueComponent>()
        })
        .and_then(|queue| queue.selected_row_rect())
        .expect("painted queue row");
    harness.inject(click(queue_point.x, queue_point.y));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert_eq!(harness.model().app.effective_panel_focus(), PanelFocus::Queue);
}

#[test]
fn music_tree_mouse_resolves_current_rows_and_rejects_an_invalidated_frame() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();

    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_list_rect())
        .expect("Music tree list geometry");
    let (root_point, album_point) = {
        let music = harness.model().test_music_owner();
        let point_for = |target: Option<&str>| {
            let node = music
                .browser
                .projected_nodes()
                .iter()
                .find(|node| music.browser.target_of(node.id()) == target)
                .expect("painted tree node");
            let row = music
                .browser
                .row_rect_for(node.id())
                .expect("painted tree row");
            (row.x, row.y)
        };
        (point_for(None), point_for(Some("album-1")))
    };
    assert!(list_area.contains(ratatui::layout::Position::new(
        root_point.0,
        root_point.1
    )));
    assert!(list_area.contains(ratatui::layout::Position::new(
        album_point.0,
        album_point.1
    )));

    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    // The artist root is a painted, focusable tree row but not an album
    // target, so its click changes only the component-local selection.
    harness.inject(click(root_point.0, root_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    }));
    assert!(harness.model().test_music_owner().selected_is_artist());

    // The same completed frame resolves the leaf's stable album identity.
    harness.inject(click(album_point.0, album_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. })
    )));
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_album_target(),
        Some("album-1")
    );

    // A content/geometry transition invalidates the completed tree result
    // before the replacement frame paints. The mounted parent still receives
    // the event through Application::tick, but MusicContent claims no stale
    // row and emits no cursor request.
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .invalidate();
    harness.inject(click(album_point.0, album_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().all(|message| {
        !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))
    }));
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_album_target(),
        Some("album-1"),
        "the stale click did not mutate the tree selection"
    );
}
