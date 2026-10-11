use super::*;

use crate::emby_library_content::{BrowserOwnerPush, EmbyLibraryContent, EmbySelectorMode};
use mbv_emby_model::test_support::make_item;
use mbv_queue::ServiceKind;
use mbv_ui_model::library::LibraryKind;
use mbv_ui_msg::{ShellRequest, TerminalObserverEvent};
use ratatui::layout::Position;
use rstest::rstest;

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

fn movies_panel(latest: bool) -> (LibraryPanel, Position) {
    let key = LibraryKey::Service {
        service: ServiceKind::Emby,
        library_id: "movies".into(),
        kind: LibraryKind::Movies,
    };
    let mut movie = make_item("Zorba", "Movie");
    movie.id = "zorba".into();
    let mut owner = EmbyLibraryContent::new(LibraryKind::Movies);
    owner.set_content(BrowserOwnerPush {
        items: vec![movie.clone()],
        latest_items: vec![movie],
        total_count: 1,
        library_total: Some(1),
        letter_filter: None,
        loading: false,
        selector_mode: EmbySelectorMode::Letters,
        feed_groups: Vec::new(),
        feed_group_ids: Vec::new(),
        feed_group_cursor: 0,
    });
    owner.set_latest_mode(latest);
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(key.clone()));
    panel.insert_owner(key, Box::new(owner));
    panel.attr(Attribute::Focus, AttrValue::Flag(true));
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    terminal
        .draw(|f| Component::view(&mut panel, f, Rect::new(0, 0, 60, 24)))
        .unwrap();
    let buf = terminal.backend().buffer();
    let row = (0..24)
        .find(|&row| line_text(buf, row).contains("Zorba"))
        .expect("row painted");
    let x = (0..60)
        .find(|&x| buf[(x, row)].symbol() == "Z")
        .expect("row text painted");
    (panel, Position::new(x, row))
}

fn press_enter(panel: &mut LibraryPanel, _row: Position) -> Option<Msg> {
    panel.on(&Event::Keyboard(KeyEvent::new(
        Key::Enter,
        KeyModifiers::NONE,
    )))
}

fn double_click_row(panel: &mut LibraryPanel, row: Position) -> Option<Msg> {
    let down = mouse_event(MouseEventKind::Down(MouseButton::Left), row.x, row.y);
    let _ = panel.on(&down);
    panel.on(&down)
}

fn is_owner_activation(message: Option<Msg>) -> bool {
    matches!(
        message,
        Some(Msg::Shell(request))
            if matches!(
                *request,
                ShellRequest::EmbyLibraryActivate { .. }
                    | ShellRequest::EmbyLibraryRowActivate { .. }
            )
    )
}

/// destination-latest-modes "Latest rows play directly in non-Wide
/// geometry": the panel's narrow Enter and double-click gates refuse the
/// overlay on a Latest list, so the owner's activation reaches the shell.
/// Other pills keep the overlay entry.
#[rstest]
#[case::enter_on_latest(press_enter, true, true, false)]
#[case::double_click_on_latest(double_click_row, true, true, false)]
#[case::enter_off_latest(press_enter, false, false, true)]
#[case::double_click_off_latest(double_click_row, false, false, true)]
fn narrow_latest_row_plays_and_other_pills_open_the_overlay(
    #[case] activate: fn(&mut LibraryPanel, Position) -> Option<Msg>,
    #[case] latest: bool,
    #[case] owner_activates: bool,
    #[case] overlay_open: bool,
) {
    let (mut panel, row) = movies_panel(latest);

    let message = activate(&mut panel, row);

    assert_eq!(is_owner_activation(message), owner_activates);
    assert_eq!(panel.test_hero_overlay_open(), overlay_open);
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
