use super::*;

use tuirealm::event::MouseButton;

/// The panel without a migrated owner claims nothing: no paint, no
/// gesture, no slot event (the transitional branch).
#[test]
fn unpainted_panel_claims_nothing() {
    let mut panel = LibraryPanel::new();
    let buf = draw_panel(&mut panel);
    // Untouched: every cell still carries the empty terminal's style,
    // compared relatively so no raw colour primitive is named here.
    let untouched = buf[(0, 0)].bg;
    for y in 0..30 {
        for x in 0..120 {
            assert_eq!(buf[(x, y)].bg, untouched);
        }
    }
    assert_eq!(
        panel.on(&mouse_event(MouseEventKind::Down(MouseButton::Left), 4, 4)),
        None,
        "an unpainted panel claims no pointer input"
    );
    assert!(panel.test_split_gap().is_none());
}

/// The panel paints the active owner's content and retains its slot hit
/// geometry: pills and list rows are on screen, and a pill click routes
/// `SelectorPicked` to the active owner.
#[test]
fn panel_paints_the_active_owner_and_resolves_slot_events() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log))),
    );
    let buf = draw_panel(&mut panel);

    // Content painted: the selector pill and a list row.
    let pill = (0..30)
        .map(|row| (row, line_text(&buf, row)))
        .find(|(_, text)| text.contains("All"))
        .expect("the migrated owner's selector pill paints");
    assert!(
        (0..30).any(|row| line_text(&buf, row).contains("alpha")),
        "the migrated owner's list rows paint"
    );

    // A click on the painted pill routes SelectorPicked to the owner.
    let pill_x = (0..120)
        .find(|&x| buf[(x, pill.0)].symbol() == "A")
        .expect("pill text painted");
    let msg = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        pill_x,
        pill.0,
    ));
    assert!(msg.is_some(), "a pill click claims the event");
    assert_eq!(
        log.borrow().events.last(),
        Some(&LibrarySlotEvent::SelectorPicked(0)),
        "the painted pill's index is the slot event"
    );
}

/// A list click resolves the row under the pointer through the owner's
/// typed carrier and delegates it as today.
#[test]
fn selector_move_updates_only_private_hover_identity_and_returns_no_message() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log))),
    );
    let _ = draw_panel(&mut panel);
    let (pill, _) = panel.test_selector_hits().regions()[1];

    assert_eq!(
        panel.on(&mouse_event(MouseEventKind::Moved, pill.x + 1, pill.y)),
        None
    );
    assert_eq!(panel.test_hovered_selector(), Some(1));
    assert_eq!(log.borrow().events, [] as [LibrarySlotEvent; 0]);

    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Moved,
            pill.right() + 1,
            pill.y
        )),
        None
    );
    assert_eq!(panel.test_hovered_selector(), None);
    assert_eq!(log.borrow().events, [] as [LibrarySlotEvent; 0]);
}

#[test]
fn list_click_delegates_to_the_active_owner() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log))),
    );
    let buf = draw_panel(&mut panel);

    // Find the second row's text and click it.
    let row = (0..30)
        .find(|&row| line_text(&buf, row).contains("beta"))
        .expect("list rows paint");
    let x = (0..120)
        .find(|&x| buf[(x, row)].symbol() == "b")
        .expect("row text painted");
    let _ = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        x,
        row,
    ));
    let log = log.borrow();
    assert!(
        log.events.last().is_some_and(|event| matches!(
            event,
            LibrarySlotEvent::List(crate::media_list::MediaListSurfaceInput::Click(_))
        )),
        "a row click routes List delegation to the owner"
    );
    assert_eq!(
        log.selections.last(),
        Some(&Some("beta".into())),
        "the owner resolved the clicked row's typed target"
    );
}

/// Owner retention: dropping a key from the live set removes its owner;
/// retained owners keep their state. #745: a panel reset reaches every
/// retained owner, active and inactive alike.
#[test]
fn owner_retention_follows_the_catalog() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    panel.insert_owner(
        LibraryKey::Feeds,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );

    // The reset reaches the active owner and the retained inactive owner,
    // returning both lists to their first row without replacing their rows.
    fixture_owner_mut(&mut panel, &LibraryKey::Home)
        .carrier
        .select_last();
    fixture_owner_mut(&mut panel, &LibraryKey::Feeds)
        .carrier
        .select_last();
    assert_eq!(
        fixture_selection(&panel, &LibraryKey::Home).as_deref(),
        Some("gamma")
    );
    panel.reset_presentation();
    assert_eq!(
        fixture_selection(&panel, &LibraryKey::Home).as_deref(),
        Some("alpha")
    );
    assert_eq!(
        fixture_selection(&panel, &LibraryKey::Feeds).as_deref(),
        Some("alpha"),
        "the inactive retained owner receives the reset too"
    );

    panel.retain_owners(&[LibraryKey::Home]);
    assert!(panel.has_owner(&LibraryKey::Home));
    assert!(!panel.has_owner(&LibraryKey::Feeds));
}

#[test]
fn unpainted_frame_after_a_painted_one_arms_nothing() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    let _ = draw_panel(&mut panel);
    assert!(panel.test_split_gap().is_some());
    // The next view without an owner clears the geometry.
    panel.set_active(None);
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|f| Component::view(&mut panel, f, Rect::new(0, 0, 120, 30)))
        .unwrap();
    assert_eq!(
        panel.on(&mouse_event(MouseEventKind::Down(MouseButton::Left), 4, 4)),
        None,
        "an unpainted frame claims no pointer input"
    );
    assert!(panel.test_split_gap().is_none());
}

/// The Wide hero pane's overview box is a prose viewport with no selection
/// (design D7): a recognized wheel inside the painted box moves the owner's
/// own offset one full gesture step (`delta` ±3); a wheel inside the hero
/// pane but outside the box is the owner's delegated input, never a box
/// scroll. Contract: mouse-input "Wheel behavior is verified for each
/// scrollable surface" (task 5.5).
#[test]
fn overview_wheel_steps_inside_the_painted_box_and_not_outside() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log)).with_overview("word ".repeat(400))),
    );
    let _ = draw_panel(&mut panel);
    let geometry = panel
        .test_wide_geometry()
        .expect("a Wide frame paints its geometry");
    let overview_box = geometry
        .overview_box
        .expect("a long overview paints its box");
    assert!(
        geometry.overview_content_length > geometry.overview_viewport,
        "the fixture overview wraps beyond the box viewport"
    );
    let owner_hero_scroll =
        |panel: &mut LibraryPanel| fixture_owner_mut(panel, &LibraryKey::Home).hero_scroll;

    let inside = panel.on(&mouse_event(
        MouseEventKind::ScrollDown,
        overview_box.x,
        overview_box.y,
    ));
    assert!(matches!(
        inside,
        Some(Msg::TerminalEvent(
            mbv_ui_msg::TerminalObserverEvent::MouseClaimed
        ))
    ));
    assert_eq!(
        owner_hero_scroll(&mut panel),
        3,
        "one wheel step = three lines"
    );

    // The hero pane's header area (above the overview box) is not the box.
    let hero = geometry.hero;
    let outside = panel.on(&mouse_event(
        MouseEventKind::ScrollDown,
        overview_box.x,
        hero.y,
    ));
    assert_eq!(
        outside,
        Some(Msg::TerminalEvent(
            mbv_ui_msg::TerminalObserverEvent::MouseClaimed
        )),
        "the hero pane still receives its delegated input"
    );
    assert_eq!(
        owner_hero_scroll(&mut panel),
        3,
        "a wheel outside the overview box does not scroll it"
    );
}
