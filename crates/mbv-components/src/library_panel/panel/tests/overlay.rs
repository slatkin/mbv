use super::*;

use mbv_ui_msg::{ShellRequest, TerminalObserverEvent};

use tuirealm::event::{Key, KeyEvent, MouseButton};

use tuirealm::props::{AttrValue, Attribute};

#[test]
fn double_click_hero_policy_defaults_to_hero_bearing_and_music_overrides() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut generic = FixtureOwner::new(log);
    assert!(generic.double_click_opens_hero_overlay());

    let mut music = crate::MusicContent::new();
    assert!(!music.double_click_opens_hero_overlay());
}

/// An unpainted frame (hidden library column, no owner) arms nothing:
/// the panel resolves only geometry it painted.
#[test]
fn wide_hero_link_click_emits_open_url_request() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(FixtureLog::default()))).with_link()),
    );
    let _ = draw_panel(&mut panel);
    let point = (0..120)
        .flat_map(|y| (0..30).map(move |x| (x, y)))
        .find(|&(x, y)| {
            panel
                .test_link_hits()
                .resolve(ratatui::layout::Position::new(x, y))
                .is_some()
        })
        .expect("painted IMDb link label");
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            point.0,
            point.1
        )),
        Some(Msg::Shell(Box::new(ShellRequest::OpenUrl(
            "https://imdb.test/dune".into()
        ))))
    );
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            point.0,
            point.1
        )),
        None
    );
}

#[test]
fn overlay_link_click_emits_open_url_request() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(FixtureLog::default()))).with_link()),
    );
    panel.test_open_hero_overlay();
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|f| Component::view(&mut panel, f, Rect::new(0, 0, 60, 24)))
        .unwrap();
    let (pane, _) = panel.test_overlay_geometry().expect("the overlay painted");
    let point = (pane.y..pane.bottom())
        .flat_map(|y| (pane.x..pane.right()).map(move |x| (x, y)))
        .find(|&(x, y)| {
            panel
                .test_link_hits()
                .resolve(ratatui::layout::Position::new(x, y))
                .is_some()
        })
        .expect("painted IMDb link label");

    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            point.0,
            point.1,
        )),
        Some(Msg::Shell(Box::new(ShellRequest::OpenUrl(
            "https://imdb.test/dune".into()
        ))))
    );
}

#[test]
fn overlay_area_is_the_browser_inset_list_box() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log)));
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    let area = Rect::new(0, 0, 60, 24);
    terminal
        .draw(|f| Component::view(&mut panel, f, area))
        .unwrap();
    let narrow = panel
        .test_narrow_geometry()
        .expect("a sub-breakpoint area paints the narrow skeleton");
    panel.test_open_hero_overlay();
    terminal
        .draw(|f| Component::view(&mut panel, f, area))
        .unwrap();
    let (pane, _) = panel.test_overlay_geometry().expect("the overlay painted");
    assert_eq!(pane, narrow.list_panel);
    assert!(
        pane.y > narrow.selector_bar.bottom(),
        "the reserved pill bar and its spacer band stay outside the overlay area"
    );
}

#[test]
fn overlay_dismissal_handles_escape_backdrop_destination_and_missing_parent() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::clone(&log))),
    );
    let _ = draw_panel(&mut panel);

    // Backdrop dismissal: with the overlay exactly the pane's size there is
    // no dimmed remainder, so the click-outside arm is only reachable in
    // Narrow geometry, on the reserved chrome rows above the inset list box.
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|f| Component::view(&mut panel, f, Rect::new(0, 0, 60, 24)))
        .unwrap();
    let narrow = panel
        .test_narrow_geometry()
        .expect("a sub-breakpoint area paints the narrow skeleton");
    panel.test_open_hero_overlay();
    terminal
        .draw(|f| Component::view(&mut panel, f, Rect::new(0, 0, 60, 24)))
        .unwrap();
    let point = (narrow.selector_bar.bottom()..narrow.list_panel.y)
        .flat_map(|y| (0..60).map(move |x| (x, y)))
        .next()
        .expect("a reserved chrome row outside the overlay pane");
    let before = log.borrow().selections.clone();
    assert!(matches!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            point.0,
            point.1,
        )),
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(log.borrow().selections, before);
    assert!(!panel.test_hero_overlay_open());

    panel.test_open_hero_overlay();
    panel.attr(Attribute::Focus, AttrValue::Flag(true));
    assert!(
        panel
            .on(&Event::Keyboard(KeyEvent::new(
                Key::Esc,
                KeyModifiers::NONE,
            )))
            .is_some()
    );
    assert!(!panel.test_hero_overlay_open());

    panel.test_open_hero_overlay();
    panel.set_active(Some(LibraryKey::Feeds));
    assert!(!panel.test_hero_overlay_open());
    panel.set_active(Some(LibraryKey::Home));
    panel.test_open_hero_overlay();
    panel.retain_owners(&[LibraryKey::Feeds]);
    panel.sync_overlay_state();
    assert!(!panel.test_hero_overlay_open());
}
