use super::*;

/// Enter opens the overlay with the Workspace focused; Up/Down then move the
/// episode list while the covered browser cursor stays put, an ordinary
/// refresh (provider detail completion) keeps the focus, and the keys keep
/// working on the refreshed rows.
#[test]
fn overlay_workspace_keys_move_the_episode_list_not_the_browser() {
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
            .and_then(crate::app::components::library_panel::LibraryPanel::test_overlay_geometry)
            .is_some(),
        "Enter opens the overlay in narrow geometry"
    );

    // An ordinary refresh pass between open and the first key: the
    // Workspace focus must survive it.
    harness.model_mut().sync_mounted_surfaces();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: 1 }))));
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        1,
        "Down moves the overlay Workspace's episode list"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );

    // Provider completion refreshes the Workspace rows in place; the focus
    // and cursor survive it and the keys keep working.
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let episodes: Vec<_> = (1..=3)
        .map(|index| {
            let mut episode = crate::app::tests::make_item(&format!("Episode {index}"), "Episode");
            episode.id = format!("episode-{index}");
            episode
        })
        .collect();
    harness.model_mut().app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), episodes)].into_iter().collect(),
        },
    );
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        panel_of(&harness).is_some_and(
            crate::app::components::library_panel::LibraryPanel::test_hero_overlay_open
        ),
        "the refresh keeps the overlay open"
    );
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        2,
        "the refreshed Workspace keeps focus and moves to the third row"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the browser cursor still never moved"
    );
}

/// Clicking a Workspace row inside the overlay resolves to the episode
/// list's stable target and is claimed — no part of the gesture reaches the
/// covered browser.
#[test]
fn overlay_workspace_click_selects_and_is_claimed() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(2);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_overlay_geometry)
        .is_some());

    let (x, y) =
        find_text(terminal.backend().buffer(), "Episode 2").expect("the Workspace row paints");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), crate::app::components::msg::ShellRequest::TvHitClick {
            hit: crate::app::components::msg::TvHit::EpisodeRow(target),
        } if target == "episode-2"))));
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        1,
        "the click selected the clicked Workspace row"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );
}

/// Keyboard movement inside the overlay's overflowing Workspace drags the
/// viewport with the cursor: enough Down steps push the first row out of
/// the box and pull the cursor's row in, with the browser list untouched.
#[test]
fn overlay_workspace_keyboard_scroll_follows_the_cursor_with_overflow() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(40);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_at_model_size(&mut harness));
    assert!(panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_overlay_geometry)
        .is_some());
    assert_eq!(tv_owner_of(&harness).episode_scroll(), 0);

    // More Down steps than the Workspace box's painted rows: the viewport
    // must follow the cursor past its bottom edge.
    for _ in 0..29 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
    }
    let terminal = draw_frame_at_model_size(&mut harness);
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        29,
        "the cursor moved through the overflowing list"
    );
    assert!(
        tv_owner_of(&harness).episode_scroll() > 0,
        "the Workspace viewport must follow the cursor with overflow"
    );
    let buf = terminal.backend().buffer();
    assert!(
        find_text(buf, "30. Episode 30").is_some(),
        "the cursor's row scrolled into the Workspace box"
    );
    // The viewport is the painted Workspace box's content rows: re-derive
    // the expected window from the box the frame actually painted (the
    // overlay's size is an arrangement fact, not this test's input).
    let (_, box_content) = panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_overlay_workspace_box)
        .expect("the overlay's Workspace box painted");
    let visible = box_content.height as usize;
    let scroll = tv_owner_of(&harness).episode_scroll();
    assert!(
        scroll > 0,
        "the Workspace viewport must follow the cursor with overflow"
    );
    assert_eq!(
        scroll,
        29 + 1 - visible,
        "the viewport follows the cursor: the cursor's row is the window's last row"
    );
    // The last row above the scrolled-in window has left the box. (Probing
    // row 1 is a substring of row 11's label, so only probe row numbers
    // whose label cannot match a longer one.)
    if scroll >= 2 {
        assert!(
            find_text(buf, &format!("{scroll}. Episode {scroll}")).is_none(),
            "the rows above the window left the Workspace box"
        );
    }
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );
}

/// The double-click-to-overlay gate is the non-Wide frame itself (design
/// D4): `narrow_geometry` is `Some` exactly when a non-Wide frame painted.
/// A non-Wide browser double-click opens the Library Hero overlay; the same
/// double-click in Wide geometry stays in the list slot and never does.
#[test]
fn browser_double_click_opens_the_overlay_only_in_non_wide_geometry() {
    // Non-Wide (80 < TWO_COLUMN_THRESHOLD): the gate is reported and the
    // gesture opens the overlay.
    let mut harness = migrated_tv_with_detail(2);
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(crate::app::components::library_panel::LibraryPanel::test_narrow_geometry)
            .is_some(),
        "a non-Wide frame reports the overlay gate"
    );
    let terminal = draw_frame_at_model_size(&mut harness);
    let (x, y) =
        find_text(terminal.backend().buffer(), "Focused Movie").expect("the browser row paints");
    double_click(&mut harness, x, y);
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(crate::app::components::library_panel::LibraryPanel::test_overlay_geometry)
            .is_some(),
        "a non-Wide browser double-click opens the Library Hero overlay"
    );

    // The same gesture in Wide geometry: the list slot takes the
    // double-click and the overlay stays closed.
    let mut harness = migrated_tv_with_detail(2);
    harness.model_mut().app.terminal_width = 100;
    drop(draw_frame_at_model_size(&mut harness));
    let wide_painted = panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_wide_geometry)
        .is_some();
    assert!(wide_painted, "the frame is Wide");
    let gate_reported = panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_narrow_geometry)
        .is_some();
    assert!(!gate_reported, "a Wide frame reports no overlay gate");
    let terminal = draw_frame_at_model_size(&mut harness);
    let list = panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_list_rect)
        .expect("the Wide list painted");
    let (x, y) = find_text_in(terminal.backend().buffer(), "Focused Movie", list)
        .expect("the Wide browser row paints in the list slot");
    double_click(&mut harness, x, y);
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(crate::app::components::library_panel::LibraryPanel::test_overlay_geometry)
            .is_none(),
        "a Wide double-click must not take the overlay path"
    );
}

/// The saved-geometry shift (unify-narrow U2/6.1): the non-Wide list slot's
/// hit rect is the Wide pane's row-flow inset, so a click on the painted
/// list box's outer two columns is padding and is not delegated to the list;
/// a click inside the row flow still selects the row under it.
#[test]
fn non_wide_padding_click_is_not_delegated_and_a_row_flow_click_selects() {
    let (mut harness, log) = migrated_home();
    drop(draw_frame_at_model_size(&mut harness));
    let narrow = panel_of(&harness)
        .and_then(crate::app::components::library_panel::LibraryPanel::test_narrow_geometry)
        .expect("the frame is non-Wide");

    // The outer two columns of the painted list box: on the pane the panel
    // painted, but outside the row flow it retained as the hit rect.
    let (edge_x, edge_y) = (narrow.list_panel.x + 1, narrow.list_panel.y + 2);
    assert!(
        narrow.list_panel.contains(ratatui::layout::Position {
            x: edge_x,
            y: edge_y,
        }),
        "the click lands on the painted pane"
    );
    assert!(
        !narrow.list_area.contains(ratatui::layout::Position {
            x: edge_x,
            y: edge_y,
        }),
        "the click lands in the padding outside the row flow inset"
    );
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: edge_x,
        row: edge_y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(
        log.borrow().events.is_empty(),
        "a padding click must not reach the list"
    );

    // A click inside the row flow still selects the row under it.
    let terminal = draw_frame_at_model_size(&mut harness);
    let (x, y) = find_text_in(terminal.backend().buffer(), "alpha", narrow.list_area)
        .expect("the first browser row paints in the row flow");
    drop(terminal);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        log.borrow().selections.last(),
        Some(&Some("alpha".to_string())),
        "the row under a row-flow click is selected"
    );
}
