use super::*;

/// The Keys destination paints through the main list's row painter:
/// group headers, one row per action with its chord column, the KEYS
/// shell, and an override rendering its configured chord (task 7.1,
/// design D7).
#[test]
fn keys_destination_paints_groups_actions_and_the_configured_chord() {
    let keys = vec![
        SettingsRow {
            label: "Playback".into(),
            value: String::new(),
            section: true,
            cursor: None,
        },
        SettingsRow {
            label: "toggle_play_pause".into(),
            value: "k".into(),
            section: false,
            cursor: Some(0),
        },
        SettingsRow {
            label: "stop".into(),
            value: "Esc".into(),
            section: false,
            cursor: Some(1),
        },
    ];
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Keys,
        rows: Vec::new(),
        services: Vec::new(),
        keys,
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(output.contains("KEYS"));
    assert!(output.contains("Playback"), "group header painted");
    assert!(output.contains("toggle_play_pause"), "action row painted");
    assert!(
        output.contains("k"),
        "the override's configured chord paints"
    );
    assert!(output.contains("Esc"), "the default chord paints");
}

/// Keys is a read-only destination (design D7 / ADR 0023): the arrows
/// move the local cursor, Enter/Space select nothing, and Back returns
/// to the main list and zeroes the cursor for the next entry.
#[test]
fn keys_destination_is_read_only_and_back_resets_the_cursor() {
    let keys = vec![
        SettingsRow {
            label: "a_first".into(),
            value: "F1".into(),
            section: false,
            cursor: Some(0),
        },
        SettingsRow {
            label: "b_second".into(),
            value: "F2".into(),
            section: false,
            cursor: Some(1),
        },
    ];
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Keys,
        rows: Vec::new(),
        services: Vec::new(),
        keys,
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    // Down moves the local cursor; no shell intent is emitted.
    assert!(matches!(
        component.on(&key(Key::Down)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    assert_eq!(component.keys_cursor, 1);
    // Enter/Space select nothing (read-only) — no Activate intent.
    assert!(!matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::Shell(ref shell_boxed))
     if matches!(shell_boxed.as_ref(), ShellRequest::SettingsIntent(_))));
    assert!(matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    // Back returns to the main list and zeroes the cursor.
    assert!(matches!(
       component.on(&key(Key::Esc)),
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(), ShellRequest::SettingsIntent(
           SettingsIntent::Back
       ))));
    assert_eq!(component.keys_cursor, 0);
}

/// Keys cursor and scroll fixture: one group header plus `actions`
/// action rows, painted once so the render geometry (cursor lines,
/// content area) is live.
fn painted_keys_content(actions: usize) -> SettingsComponent {
    let mut keys = vec![SettingsRow {
        label: "Playback".into(),
        value: String::new(),
        section: true,
        cursor: None,
    }];
    keys.extend((0..actions).map(|i| SettingsRow {
        label: format!("action_{i}"),
        value: "k".into(),
        section: false,
        cursor: Some(i),
    }));
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Keys,
        rows: Vec::new(),
        services: Vec::new(),
        keys,
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    component
}

/// The Keys cursor is an ordinal over action rows (headers are not
/// addressable): it clamps to the action count on both ends, and a
/// stale cursor value pushed in with content clamps to the last action
/// instead of highlighting a header or nothing.
#[test]
fn keys_cursor_numbers_actions_and_clamps_to_the_action_count() {
    let mut component = painted_keys_content(2);
    for _ in 0..5 {
        assert!(matches!(
            component.on(&key(Key::Down)),
            Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
        ));
    }
    assert_eq!(component.keys_cursor, 1, "Down clamps to the last action");
    for _ in 0..5 {
        component.on(&key(Key::Up));
    }
    assert_eq!(component.keys_cursor, 0, "Up clamps at the first action");

    // A stale cursor larger than the action count clamps on the next
    // content push (headers carry no cursor, so they are never
    // highlighted).
    component.keys_cursor = 7;
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Keys,
        rows: Vec::new(),
        services: Vec::new(),
        keys: component.keys.clone(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    assert_eq!(component.keys_cursor, 1);
}

/// Arrow moves scroll the Main window so the cursor's row stays
/// painted: section headers are not addressable, so the Down clamp
/// stops at the last action and the scroll follows the highlight.
#[test]
fn main_arrow_moves_scroll_the_cursor_into_view() {
    let mut rows = vec![SettingsRow {
        label: "Group A".into(),
        value: String::new(),
        section: true,
        cursor: None,
    }];
    rows.extend((0..20).map(|i| SettingsRow {
        label: format!("action_{i}"),
        value: "on".into(),
        section: false,
        cursor: Some(i),
    }));
    rows.push(SettingsRow {
        label: "Group B".into(),
        value: String::new(),
        section: true,
        cursor: None,
    });
    rows.extend((20..40).map(|i| SettingsRow {
        label: format!("action_{i}"),
        value: "on".into(),
        section: false,
        cursor: Some(i),
    }));
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Main,
        rows,
        services: Vec::new(),
        keys: Vec::new(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let height = component.geometry.content_area.height as usize;
    assert!(
        component.geometry.cursor_lines.len() > height,
        "fixture overflows the viewport"
    );
    // Down past the end clamps at the last action, skipping headers.
    for _ in 0..50 {
        component.on(&key(Key::Down));
    }
    assert_eq!(component.cursor, 39);
    let line = component.geometry.cursor_lines[39];
    assert!(
        line >= component.scroll && line < component.scroll + height,
        "cursor row must be inside the scrolled window after Down"
    );
    assert!(
        component.scroll > 0,
        "the window scrolled to reach the last row"
    );
    for _ in 0..50 {
        component.on(&key(Key::Up));
    }
    assert_eq!(component.cursor, 0);
    let top = component.geometry.cursor_lines[0];
    assert!(
        top >= component.scroll && top < component.scroll + height,
        "the first action row is inside the scrolled window again"
    );
}
#[test]
fn keys_arrow_moves_scroll_the_cursor_into_view() {
    let mut component = painted_keys_content(40);
    let height = component.geometry.content_area.height as usize;
    assert!(
        component.geometry.cursor_lines.len() > height,
        "fixture overflows the viewport"
    );
    for _ in 0..40 {
        component.on(&key(Key::Down));
    }
    assert_eq!(component.keys_cursor, 39);
    let line = component.geometry.cursor_lines[39];
    assert!(
        line >= component.scroll && line < component.scroll + height,
        "cursor row must be inside the scrolled window after Down"
    );
    assert!(
        component.scroll > 0,
        "the window scrolled to reach the last row"
    );
    for _ in 0..40 {
        component.on(&key(Key::Up));
    }
    assert_eq!(component.keys_cursor, 0);
    let top = component.geometry.cursor_lines[0];
    assert!(
        top >= component.scroll && top < component.scroll + height,
        "the first action row is inside the scrolled window again"
    );
}

/// PageUp/PageDown/Home/End scroll the Keys window (claimed, not
/// fallen through to the router), clamped to the document, and the
/// cursor follows into the newly visible window so the highlighted
/// action row stays painted.
#[test]
fn keys_page_and_edge_keys_scroll_and_are_claimed() {
    let mut component = painted_keys_content(40);
    let height = component.geometry.content_area.height as usize;
    let max_scroll = component
        .geometry
        .cursor_lines
        .iter()
        .copied()
        .max()
        .unwrap_or(0)
        .saturating_sub(height - 1);
    let cursor_visible = |component: &SettingsComponent| {
        let line = component.geometry.cursor_lines[component.keys_cursor];
        line >= component.scroll && line < component.scroll + height
    };
    assert!(matches!(
        component.on(&key(Key::PageDown)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    assert_eq!(component.scroll, 10.min(max_scroll));
    assert!(
        cursor_visible(&component),
        "PageDown keeps the highlighted action row in the window"
    );
    assert!(matches!(
        component.on(&key(Key::End)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    assert_eq!(component.scroll, max_scroll);
    assert!(
        cursor_visible(&component),
        "End keeps the highlighted action row in the window"
    );
    assert!(matches!(
        component.on(&key(Key::Home)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    assert_eq!(component.scroll, 0);
    assert!(
        cursor_visible(&component),
        "Home keeps the highlighted action row in the window"
    );
    assert!(matches!(
        component.on(&key(Key::PageUp)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    assert_eq!(component.scroll, 0, "PageUp clamps at the top");
    assert!(
        cursor_visible(&component),
        "PageUp keeps the highlighted action row in the window"
    );
}

/// Scrolling the Keys window moves the painted rows (buffer evidence:
/// the rows above the window scroll off, later rows paint in their
/// place).
#[test]
fn keys_scroll_moves_the_painted_window() {
    let mut component = painted_keys_content(40);
    component.on(&key(Key::PageDown));
    let mut terminal = Terminal::new(TestBackend::new(40, 12)).unwrap();
    terminal
        .draw(|frame| component.view(frame, frame.area()))
        .unwrap();
    let output: String = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|cell| cell.symbol().to_owned())
        .collect();
    assert!(
        !output.contains("action_0"),
        "rows above the window scrolled off"
    );
    assert!(
        output.contains("action_15"),
        "rows below the window painted in"
    );
}

#[test]
fn back_from_services_zeroes_the_local_services_cursor() {
    let mut component = SettingsComponent::new();
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Services,
        rows: Vec::new(),
        services: vec![
            ServiceRow {
                name: "Emby".into(),
                detail: "Not configured".into(),
                muted: false,
            },
            ServiceRow {
                name: "Audiobookshelf".into(),
                detail: "Not configured".into(),
                muted: false,
            },
        ],
        keys: Vec::new(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    // Move the Services cursor off the top row (Down is a local cursor
    // move and emits the framework-local claim marker).
    assert!(matches!(
        component.on(&key(Key::Down)),
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    // Leaving Services via Back zeroes the component's own cursor, so the
    // next Services entry starts back at the top instead of remembering
    // the old position.
    assert!(matches!(
       component.on(&key(Key::Esc)),
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(), ShellRequest::SettingsIntent(
           SettingsIntent::Back
       ))));
    component.set_content(SettingsSnapshot {
        destination: SettingsDestination::Services,
        rows: Vec::new(),
        services: vec![
            ServiceRow {
                name: "Emby".into(),
                detail: "Not configured".into(),
                muted: false,
            },
            ServiceRow {
                name: "Audiobookshelf".into(),
                detail: "Not configured".into(),
                muted: false,
            },
        ],
        keys: Vec::new(),
        setup: None,
        area: Rect::new(0, 0, 40, 12),
    });
    // Effective local cursor after re-entry: 0 (the fix), not 1 (the
    // stale position the component would otherwise carry into the next
    // entry). The re-entry push itself carries no cursor value.
    assert!(matches!(
        component.on(&key(Key::Enter)),
        Some(Msg::Service(ServiceRequest::ActivateService(0)))
    ));
}
