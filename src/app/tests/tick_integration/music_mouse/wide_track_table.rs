use super::*;

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
        let content = music
            .track_list
            .current_content_rect()
            .expect("Wide track table retained its current content rect");
        let selected = music
            .track_list
            .current_selected_row_rect()
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
    assert!(outcome
        .messages
        .iter()
        .all(|message| { !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumCursor { .. })) }));
    assert_eq!(track_state(&harness), (true, Some(1)));
    harness.inject(click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome
        .messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::MusicTrackActivate { .. }))));

    // Right-click resolves the same retained row and translates directly to
    // the track context intent; no shell-side coordinate lookup is involved.
    harness.inject(right_click(second_track_point.0, second_track_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::MusicRowContextMenu(_, Some((x, y))) if *x == second_track_point.0 && *y == second_track_point.1))));

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
        .all(|message| !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumCursor { .. }))));
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
        harness
            .model()
            .test_music_owner()
            .track_list
            .multi_selection(),
        &["track-1".to_string(), "track-2".to_string()]
    );

    harness.inject(mouse(x, y + 2, tuirealm::event::KeyModifiers::SHIFT));
    harness.step();
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .track_list
            .multi_selection(),
        &[
            "track-1".to_string(),
            "track-2".to_string(),
            "track-3".to_string()
        ]
    );

    harness.inject(mouse(x, y, tuirealm::event::KeyModifiers::NONE));
    harness.step();
    assert!(harness
        .model()
        .test_music_owner()
        .track_list
        .multi_selection()
        .is_empty());
}
