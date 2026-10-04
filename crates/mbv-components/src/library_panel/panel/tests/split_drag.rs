use super::*;

use mbv_ui_msg::ShellRequest;

use tuirealm::event::MouseButton;

/// The Wide split-boundary drag moved in from
/// former split boundary: a press inside the painted gap arms only
/// the split gesture, and the drag resolves the live width from the
/// panel's own painted geometry.
#[test]
fn split_drag_resolves_the_live_width_from_the_painted_gap() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log))),
    );
    let _ = draw_panel(&mut panel);
    let gap = panel.test_split_gap().expect("a wide split painted");
    assert!(gap.width > 0 && gap.height > 0);

    // A press and release without motion leaves the split unchanged and
    // emits no persistence request.
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            gap.x,
            gap.y
        )),
        None
    );
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            gap.x,
            gap.y
        )),
        None,
        "press-and-release without motion changes nothing"
    );

    // Press inside the gap, drag right, release.
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            gap.x,
            gap.y
        )),
        None
    );
    let msg = panel.on(&mouse_event(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x + 6,
        gap.y,
    ));
    match msg {
        Some(Msg::Shell(shell_boxed)) => {
            let ShellRequest::ResizeListPaneLive(width) = *shell_boxed else {
                panic!("the drag must resolve the live width: {shell_boxed:?}")
            };
            assert!(width > 0, "the drag resolves a live width: {width}");
        }
        other => panic!("the drag must resolve the live width: {other:?}"),
    }
    // A changed drag emits exactly one persistence request at release.
    let end = panel.on(&mouse_event(
        MouseEventKind::Up(MouseButton::Left),
        gap.x + 6,
        gap.y,
    ));
    assert!(matches!(
        end,
        Some(Msg::Shell(ref shell_boxed))  if matches!(shell_boxed.as_ref(), ShellRequest::ResizeListPaneEnd(width) if *width > 0)));
}

#[test]
fn split_drag_started_outside_the_gap_emits_no_resize_request() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log)));
    let _ = draw_panel(&mut panel);
    let list = panel.test_list_rect().expect("a wide list painted");
    let x = list.x;
    let y = list.y;

    let _ = panel.on(&mouse_event(MouseEventKind::Down(MouseButton::Left), x, y));
    let drag = panel.on(&mouse_event(
        MouseEventKind::Drag(MouseButton::Left),
        x.saturating_add(6),
        y,
    ));
    let end = panel.on(&mouse_event(
        MouseEventKind::Up(MouseButton::Left),
        x.saturating_add(6),
        y,
    ));
    assert!(!matches!(
        drag,
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::ResizeListPaneLive(_))));
    assert!(!matches!(
        end,
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::ResizeListPaneEnd(_))));
}

#[test]
fn interrupted_split_drag_does_not_claim_a_later_row_release_or_unchanged_drag() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log))),
    );
    let _ = draw_panel(&mut panel);
    let gap = panel.test_split_gap().expect("a wide split painted");
    let list = panel.test_list_rect().expect("a wide list painted");

    let _ = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        gap.x,
        gap.y,
    ));
    assert!(matches!(
        panel.on(&mouse_event(
            MouseEventKind::Drag(MouseButton::Left),
            gap.x + 6,
            gap.y,
        )),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::ResizeListPaneLive(_))));
    // The release is dropped outside the painted area, abandoning the gap
    // recognizer before it can emit its end request.
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            120,
            gap.y,
        )),
        None
    );

    let _ = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        list.x,
        list.y,
    ));
    assert_eq!(
        log.borrow().events.last(),
        Some(&LibrarySlotEvent::List(MediaListSurfaceInput::Click(
            ratatui::layout::Position::new(list.x, list.y),
        )))
    );
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            list.x,
            list.y,
        )),
        None,
        "the non-gap release remains with the surface gesture"
    );

    // A fresh drag that resolves the already-stored width emits neither a
    // live update nor an end request.
    let _ = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        gap.x,
        gap.y,
    ));
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Drag(MouseButton::Left),
            gap.x + 6,
            gap.y,
        )),
        None
    );
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            gap.x + 6,
            gap.y,
        )),
        None
    );
}
