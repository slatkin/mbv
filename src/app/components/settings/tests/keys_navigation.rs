use super::*;

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
