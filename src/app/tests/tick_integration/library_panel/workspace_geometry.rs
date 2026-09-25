use super::*;

#[test]
fn non_wide_saved_geometry_agrees_with_the_painted_frame() {
    let (mut harness, _log) = migrated_home();
    let terminal = draw_frame_at_model_size(&mut harness);
    let narrow = panel_of(&harness)
        .and_then(|panel| panel.test_narrow_geometry())
        .expect("the frame is non-Wide");
    let panel = panel_of(&harness).expect("the Library panel is mounted");

    // The hit rect is the row-flow inset, not the pane.
    assert_eq!(panel.test_list_rect(), Some(narrow.list_area));
    assert_ne!(narrow.list_area, narrow.list_panel);

    // The context-menu anchor is the painted Browser pane.
    let (anchor, selected) = panel.menu_geometry().expect("the frame painted");
    assert_eq!(anchor, narrow.list_panel);
    assert_ne!(
        anchor, narrow.list_area,
        "the anchor is the pane, not the inset"
    );

    // The selected-row rect is the painted cursor row: the cursor row's text
    // paints inside it, and it sits in the row flow.
    let selected = selected.expect("the cursor row painted");
    let (x, y) = find_text_in(terminal.backend().buffer(), "alpha", selected)
        .expect("the cursor row paints in the selected-row rect");
    assert_eq!(y, selected.y);
    assert!(narrow
        .list_area
        .contains(ratatui::layout::Position { x, y }));
}

/// The Library Hero overlay paints its Workspace box from the Workspace's own
/// focus: the sheet carries its PillRow surface, and the box's body carries the
/// focused `MainContentBox` fill (`#48584e`) while the Workspace holds focus,
/// with the list's rows striped in its focused `LibraryPanel` fill. The resting
/// half of the same resolution is pinned by
/// `wide_tests::unfocused_workspace_hero_renders_resting_surfaces`.
#[test]
fn overlay_sheet_and_workspace_box_rest_at_the_hero_box_slate() {
    use crate::app::palette::{surface_colors, Surface, SURFACE_RESTING};
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(6);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the overlay is open in non-Wide geometry"
    );
    let buf = terminal.backend().buffer();

    // The sheet's own surface: the overlay frame's top-left corner cell.
    let (_, frame) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .expect("the overlay painted");
    assert_eq!(
        buf[(frame.x, frame.y)].bg,
        surface_colors(Surface::PillRow, false).fill,
        "the overlay sheet carries the PillRow surface"
    );

    // The Workspace box rests at the hero boxes' own Slate while the
    // Workspace holds focus, and the list's rows stripe with the fixed
    // resting content Storm. A whole-box "no resting cell" scan is no
    // longer expressible: the selected row's bar shares the resting fill's
    // value, so the box body's fill is proven on its own padding row below.
    let (box_panel, _) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box painted");
    let resting_body = surface_colors(Surface::MainContentBox, false).fill;
    let focused_body = surface_colors(Surface::MainContentBox, true).fill;
    assert_ne!(resting_body, focused_body);

    // The box's body row rests at the hero boxes' Slate even while the
    // Workspace holds focus.
    assert_eq!(
        buf[(box_panel.x + 1, box_panel.bottom() - 1)].bg,
        resting_body,
        "the box's padding row rests at the hero boxes' Slate while the Workspace holds focus"
    );
    let striped = (box_panel.top()..box_panel.bottom())
        .any(|y| (box_panel.left()..box_panel.right()).any(|x| buf[(x, y)].bg == SURFACE_RESTING));
    assert!(
        striped,
        "the Workspace box stripes its rows with the fixed resting content Storm"
    );

    // The Queue column taking focus leaves the same box at the resting
    // Slate: the overlay's Workspace box never takes a focused fill.
    harness.model_mut().app.panel_mode = PanelMode::Both;
    harness.model_mut().app.terminal_width = 100;
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the overlay stays open while the Queue holds focus"
    );
    let (box_panel, _) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box stays painted");
    assert_eq!(
        terminal.backend().buffer()[(box_panel.x + 1, box_panel.bottom() - 1)].bg,
        resting_body,
        "the Workspace box rests at the same Slate while the Queue holds focus"
    );
}

/// A wheel over the overlay's Workspace rows scrolls the episode list and
/// is claimed; the covered browser list never scrolls from it.
#[test]
fn overlay_workspace_wheel_scrolls_and_is_claimed() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(40);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());

    // Walk the cursor into the overflow first (the keyboard path, already
    // covered above): the wheel's single notch must then scroll a following
    // viewport, not a reset one. One notch = one cursor step (the canonical
    // wheel = Move translation); the 30 ms burst throttle collapses rapid
    // notches, so the test drives exactly one recognized gesture.
    for _ in 0..29 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
    }
    drop(draw_frame_at_model_size(&mut harness));
    let scroll_before = tv_owner_of(&harness).episode_scroll();
    assert!(scroll_before > 0, "test setup: the viewport is in overflow");

    let (x, y) = find_text(terminal.backend().buffer(), "Episode 2")
        .or_else(|| find_text(terminal.backend().buffer(), "Episode 3"))
        .or_else(|| find_text(terminal.backend().buffer(), "Episode 4"))
        .or_else(|| find_text(terminal.backend().buffer(), "Episode 5"))
        .expect("a Workspace row paints");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed)
    )));
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        30,
        "the wheel stepped the Workspace cursor"
    );
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        tv_owner_of(&harness).episode_scroll() > scroll_before,
        "the wheel scrolled the Workspace viewport past its bottom edge"
    );
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the wheel leaves the overlay open"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );
}

/// A narrow Music overlay whose album tracks are still fetching: the
/// Workspace's rows arrive on a later sync pass and take the focus the open
/// transition could not, so Down moves the track list (never the album
/// browser) without a second Enter, and the focus survives a Queue focus
/// round trip; an explicit shell focus clear still wins and stays won.
#[test]
fn late_overlay_workspace_takes_the_focus_when_its_rows_arrive() {
    use crate::app::components::MusicContent;
    use tuirealm::event::{Key, KeyEvent};

    let music_key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: LibraryKind::Music,
    };
    let mut app = crate::app::render::make_music_group_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 80;
    app.terminal_height = 60;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().sync_mounted_surfaces();
    drop(draw_frame_sized(&mut harness));

    // Open while the selected album's tracks are not cached yet.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_sized(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert!(
        !harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .expect("music owner installed")
            .track_focused(),
        "an empty Workspace cannot take focus yet"
    );

    // The provider completion: the tracks arrive on the next sync pass.
    let tracks: Vec<_> = (1..=2)
        .map(|index| {
            let mut track = crate::app::tests::make_item(&format!("Track {index}"), "Audio");
            track.id = format!("track-{index}");
            track
        })
        .collect();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert("album-1".into(), tracks);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .expect("music owner installed")
            .track_focused(),
        "the Workspace's arriving rows take the focus the open transition could not"
    );
    assert_eq!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .selected_track_item()
            .map(|track| track.id),
        Some("track-1".into()),
        "the arriving rows seed the cursor on the first track"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(!outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::MusicAlbumCursor { .. }))));
    assert_eq!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .selected_track_item()
            .map(|track| track.id),
        Some("track-2".into()),
        "Down moves the overlay Workspace's track list"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[1].resting().cursor(),
        0,
        "the covered album browser must not move"
    );

    // Queue takes focus and gives it back: the Workspace keys keep working.
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Up,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .selected_track_item()
            .map(|track| track.id),
        Some("track-1".into()),
        "the Workspace keeps its keys across the Queue focus round trip"
    );

    // An explicit shell focus clear wins while the overlay is open and
    // stays won: no follow-up push re-seizes the focus.
    harness.model_mut().music_track_focus_request =
        Some(crate::app::shell::MusicTrackFocusRequest::Clear);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .track_focused(),
        "the shell's focus clear takes effect over the open overlay"
    );
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .track_focused(),
        "no push re-seizes the focus after the shell's clear"
    );
}

/// The overlay Workspace's pager delegates to the episode list's own page
/// stride (the shared owner's canonical page operation, like Music's
/// overlay pager) and reports the applied movement as the component-
/// resolved `TvEpisodeMove` delta — never recomputed from painted row
/// arithmetic that has no Narrow geometry behind it.
#[test]
fn overlay_workspace_pager_moves_by_the_episode_lists_own_stride() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(10);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_at_model_size(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: 5 }))),
        "PageDown moves by the episode list's own page stride"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        5,
        "PageDown lands five rows down"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::PageUp,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: -5 }))),
        "PageUp reports the applied movement too"
    );
    assert_eq!(tv_owner_of(&harness).episode_cursor(), 0);
}

/// The overlay Workspace's Home/End mutate the episode list locally AND
/// report the component-resolved movement, exactly like the sibling
/// movement chords — the shell never recomputes a jump the component made.
#[test]
fn overlay_workspace_home_end_report_the_resolved_move() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(10);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_at_model_size(&mut harness));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::End,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: 9 }))),
        "End reports the resolved jump to the last row"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        9,
        "End lands on the last row"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Home,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: -9 }))),
        "Home reports the resolved jump to the first row"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        0,
        "Home lands on the first row"
    );
}

/// A stale overlay-open bit must never let the narrow overlay path shadow
/// Wide handling: after a narrow->wide resize the overlay's keyboard arms
/// are inert and the Wide workspace's own Home reaches the series rail.
#[test]
fn stale_overlay_bit_never_shadows_wide_keyboard_handling() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(2);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "Enter opens the overlay in narrow geometry"
    );

    // Resize narrow -> wide: the sync pass re-pushes the owner's actual
    // breakpoint before the next key is delivered.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().app.terminal_height = 40;
    assert!(
        harness.model().app.wide_tv_library_area(0).is_some(),
        "the resized terminal is Wide-eligible"
    );
    harness.model_mut().sync_mounted_surfaces();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Home,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ref shell_boxed)
         if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvJumpCursor { to_end: false }))),
        "the Wide workspace's Home reaches the series rail, not the overlay arms"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        0,
        "the overlay's episode list did not move"
    );
}

/// The overlay's focused Workspace paints its cursor: every row of the
/// canonical list that holds focus resolves the list's own focused emphasis
/// (the selected row's bold title), and the selected row paints the focused
/// Iris bar while the Workspace box itself rests at Slate like the Wide Hero
/// pane's does.
#[test]
fn overlay_workspace_paints_its_cursor_row() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(6);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the overlay is open in non-Wide geometry"
    );
    let (_, content) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box painted");
    // TV's Workspace carries no header row, so the only bar row inside the
    // box's content is the cursor's own selected row, painted with the
    // focused Iris bar (not the shared Slate bar, nor the unfocused Ink).
    let buf = terminal.backend().buffer();
    let cursor_row = (content.top()..content.bottom()).find(|&y| {
        (content.left()..content.right())
            .any(|x| buf[(x, y)].bg == crate::app::palette::ACCENT_ACTIVE)
    });
    assert!(
        cursor_row.is_some(),
        "the overlay's focused Workspace must paint a visible cursor row"
    );
    let first_episode = find_text_in(buf, "Episode 1", content)
        .expect("the first episode row paints")
        .1;
    assert_eq!(
        cursor_row,
        Some(first_episode),
        "the cursor row is the Workspace's selected first episode"
    );
}
