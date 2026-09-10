use super::*;

#[test]
fn browser_reorder_preserves_stable_target_for_click() {
    let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
    let mut items = make_items(3);
    browser.set_content(BrowserContent::from_items(items.clone()));
    browser.set_focused(true);
    browser.set_narrow_extras(NarrowBrowseExtras {
        hero_placeholder: true,
        ..NarrowBrowseExtras::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();
    let target = "id1".to_string();
    items.reverse();
    browser.set_content(BrowserContent::from_items(items));
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();
    let position = browser
        .test_inline_target_position(target.clone())
        .unwrap_or(Position::new(1, 1));
    let message = browser.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert!(matches!(
        message,
        Some(Msg::Shell(ShellRequest::BrowserRowClick { target: Some(ref id) })) if id == &target
    ));
}

/// Narrow canonical-list path (task 6.2): with `wide_movies` false and the
/// hero-capable browse surface reserving an inline hero block
/// (`hero_placeholder`), the active control is the embedded
/// `InlineMediaBrowser`, not the generic two-column grid — `left_item_rows`
/// stays empty and row identity comes from `inline_browser.resolve_point`
/// (design.md D6). A left click below the reserved hero block must move the
/// cursor to the clicked row and emit the resolved target, exactly like the
/// wide rail's `browser_mouse_uses_the_painted_two_column_cell_for_left_and_right_clicks`.
#[test]
fn narrow_canonical_list_click_moves_cursor_and_emits_row_click() {
    let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
    browser.set_content(BrowserContent::from_items(make_items(6)));
    browser.set_focused(true);
    browser.set_narrow_extras(NarrowBrowseExtras {
        hero_placeholder: true,
        ..NarrowBrowseExtras::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();
    assert!(
        browser.test_layout().left_item_rows.is_empty(),
        "narrow canonical path must not populate the generic-grid row map"
    );

    let (area, _targets) = browser.test_inline_targets();
    let target = browser.cursor();
    let position = browser
        .test_inline_target_position(format!("id{target}"))
        .unwrap_or(Position::new(area.x, area.y));

    let message = browser.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::BrowserRowClick {
            target: Some("id1".into())
        })),
        "narrow canonical row click must resolve via inline_browser.resolve_point"
    );
    assert_eq!(browser.cursor(), 1);
}

/// A double-click on the same narrow canonical row emits activation instead
/// of a plain cursor move (mirrors the accepted Music narrow precedent,
/// task 6.1).
#[test]
fn narrow_canonical_list_double_click_emits_row_activate() {
    let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
    browser.set_content(BrowserContent::from_items(make_items(6)));
    browser.set_focused(true);
    browser.set_narrow_extras(NarrowBrowseExtras {
        hero_placeholder: true,
        ..NarrowBrowseExtras::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();

    let (area, _targets) = browser.test_inline_targets();
    let target = browser.cursor();
    let down = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: browser
            .test_inline_target_position(format!("id{target}"))
            .unwrap_or(Position::new(area.x, area.y))
            .x,
        row: browser
            .test_inline_target_position(format!("id{target}"))
            .unwrap_or(Position::new(area.x, area.y))
            .y,
        modifiers: KeyModifiers::NONE,
    });

    let first = browser.on(&down);
    assert_eq!(
        first,
        Some(Msg::Shell(ShellRequest::BrowserRowClick {
            target: Some("id1".into())
        }))
    );
    let second = browser.on(&down);
    assert_eq!(
        second,
        Some(Msg::Shell(ShellRequest::BrowserRowActivate {
            target: Some("id1".to_string())
        }))
    );
}

/// A right click on a narrow canonical row emits the context menu request
/// with the raw pointer position as the anchor (design.md D4).
#[test]
fn narrow_canonical_list_right_click_emits_row_context_menu() {
    let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
    browser.set_content(BrowserContent::from_items(make_items(6)));
    browser.set_focused(true);
    browser.set_narrow_extras(NarrowBrowseExtras {
        hero_placeholder: true,
        ..NarrowBrowseExtras::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();

    let (_area, targets) = browser.test_inline_targets();
    let target_row = targets
        .iter()
        .position(
            |target| matches!(target, Some(id) if id.as_str() != format!("id{}", browser.cursor())),
        )
        .expect("a non-selected row is painted below the inline hero");
    let target = targets[target_row].clone().unwrap();
    let position = browser.test_inline_target_position(target.clone()).unwrap();

    let message = browser.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::BrowserRowContextMenu {
            target: Some(target),
            anchor: (position.x, position.y),
        }))
    );
}

/// A pill click in narrow mode (letter pills painted above the canonical
/// inline list) resolves via the same `pill_regions` map the wide rail uses
/// — `handle_mouse` never branches on breakpoint for pill hits.
#[test]
fn narrow_canonical_list_pill_click_emits_pill_click() {
    let mut browser = BrowserComponent::new_for_kind(BrowserKind::Movies);
    browser.set_content(BrowserContent::from_items(make_items(6)));
    browser.set_focused(true);
    browser.set_narrow_extras(NarrowBrowseExtras {
        hero_placeholder: true,
        show_letter_pills: true,
        ..NarrowBrowseExtras::default()
    });
    let mut terminal = Terminal::new(TestBackend::new(60, 30)).unwrap();
    terminal
        .draw(|frame| browser.view(frame, frame.area()))
        .unwrap();

    let (rect, target) = browser
        .test_layout()
        .selector_tabs
        .iter()
        .find(|(_, target)| *target == 2)
        .copied()
        .expect("third letter pill painted");

    let message = browser.on(&Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    assert_eq!(
        message,
        Some(Msg::Shell(ShellRequest::BrowserPillClick { target }))
    );
}
