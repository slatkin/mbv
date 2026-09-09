use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::{MusicWorkspaceComponent, Msg, ShellRequest};
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
    let music_id = harness
        .model()
        .music_workspace_id
        .clone()
        .expect("grouped Music child mounted");

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().mouse_subscribed.contains(&music_id));
    let track_cursor = |harness: &TickHarness| {
        harness
            .model()
            .application
            .get_component(&music_id)
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap()
            .track_cursor()
    };
    let (track_point, second_track_point) = {
        let music = harness
            .model()
            .application
            .get_component(&music_id)
            .unwrap()
            .as_any()
            .downcast_ref::<MusicWorkspaceComponent>()
            .unwrap();
        let content = music
            .test_track_content_rect()
            .expect("Wide track table retained its current content rect");
        let selected = music
            .test_track_selected_row_rect()
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
    assert_eq!(track_cursor(&harness), Some(1));
    harness.inject(click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicTrackActivate)
    )));

    // Right-click resolves the same retained row and translates directly to
    // the track context intent; no shell-side coordinate lookup is involved.
    harness.inject(right_click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicTrackContextMenuAt {
            anchor: (x, y)
        }) if *x == second_track_point.0 && *y == second_track_point.1
    )));

    // Wheel over the painted table advances exactly one local track row and
    // emits no shell-side cursor movement.
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: track_point.0,
        row: track_point.1,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert_eq!(track_cursor(&harness), Some(2));
    assert!(outcome
        .messages
        .iter()
        .all(|message| !matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))));
}

/// The Wide album rail resolves its own retained row. Group pills remain
/// Music-owned parent chrome, while the track table retains its independent
/// child behavior (covered above).
#[test]
fn music_wide_album_and_group_pill_use_their_own_retained_geometry() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Album 2", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let music_id = harness
        .model()
        .music_workspace_id
        .clone()
        .expect("grouped Music child mounted");

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    let (album_point, pill_point, group) = harness
        .model()
        .application
        .get_component(&music_id)
        .unwrap()
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .map(|music| {
            let album = music
                .album_selected_row_rect()
                .expect("Wide album control retained its selected row");
            let (pill, group) = music
                .test_pill_regions()
                .iter()
                .find(|(_, group)| *group > 0)
                .copied()
                .expect("a non-selected group pill painted");
            ((album.x, album.y), (pill.x, pill.y), group)
        })
        .expect("Music component type");
    let click = |(column, row)| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    harness.inject(click(album_point));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. })
    )));

    harness.inject(click(pill_point));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicGroupSwitch { delta }) if *delta == group as i64
    )));
}

/// Normal grouped Music uses the retained Inline result for album clicks and
/// wheel input. This live-tick proof also checks the mounted destination is
/// subscribed after the current frame, rather than relying on direct `on()`.
#[test]
fn music_narrow_album_uses_retained_geometry_for_live_mouse_gestures() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Album 2", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let music_id = harness
        .model()
        .music_workspace_id
        .clone()
        .expect("grouped Music child mounted");

    let mut terminal = Terminal::new(TestBackend::new(80, 12)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().mouse_subscribed.contains(&music_id));
    let selected = harness
        .model()
        .application
        .get_component(&music_id)
        .unwrap()
        .as_any()
        .downcast_ref::<MusicWorkspaceComponent>()
        .unwrap()
        .layout()
        .selected_item_rect
        .expect("Inline album result retained its selected geometry");
    let point = (selected.x, selected.y);

    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };
    harness.inject(click(point.0, point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicAlbumCursor { target: 0, .. })
    )));
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: point.0,
        row: point.1,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::MusicAlbumCursor { target: 1, .. })
    )));
}
