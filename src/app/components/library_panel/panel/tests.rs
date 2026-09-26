use crate::app::components::library_panel::{
    LibraryContentOwner, LibraryKey, LibraryPanel, LibrarySlotEvent,
};
use crate::app::components::media_list::MediaListSurfaceInput;
use crate::app::components::{Msg, ShellRequest, TerminalObserverEvent, UserEvent};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute};

use crate::app::components::library_panel::content::{
    HeroContent, LibraryPanelContent, ListSlot, SelectorRow,
};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState,
};
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use std::cell::RefCell;
use std::rc::Rc;
use tuirealm::event::MouseButton;

fn item(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// What a fixture owner did: every slot event and every post-event list
/// selection, shared with the test through an `Rc` (the panel owns the
/// owner, so the test reads the log, not the component).
#[derive(Default)]
struct FixtureLog {
    events: Vec<LibrarySlotEvent>,
    selections: Vec<Option<String>>,
}

/// A test-owned content owner: the minimal migrated owner the panel can
/// host — a carrier-backed list slot plus a policy-shaped hero. It
/// translates slot events exactly as a destination will ("as today":
/// typed point resolution through its own carrier), proving the
/// infrastructure without converting any destination.
struct FixtureOwner {
    carrier: MediaListCarrier<String>,
    log: Rc<RefCell<FixtureLog>>,
    link: bool,
}

impl FixtureOwner {
    fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
        Self {
            carrier,
            log,
            link: false,
        }
    }

    fn with_link(mut self) -> Self {
        self.link = true;
        self
    }
}

impl LibraryContentOwner for FixtureOwner {
    fn content(&mut self) -> LibraryPanelContent<'_> {
        LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into(), "New".into()],
                markers: vec![],
                active: Some(0),
            }),
            list: ListSlot::Media(&mut self.carrier),
            hero: Some(HeroContent {
                facts: crate::app::components::library_panel::HeroFacts {
                    title: "Dune".into(),
                    meta_rows: if self.link {
                        vec!["2021".into(), "IMDb".into()]
                    } else {
                        vec!["2021".into()]
                    },
                    links: if self.link {
                        vec![crate::app::components::library_panel::HeroLink {
                            name: "IMDb".into(),
                            url: "https://imdb.test/dune".into(),
                        }]
                    } else {
                        Vec::new()
                    },
                    duration_row: None,
                    progress_row: None,
                    artwork: crate::app::components::library_panel::HeroArtwork {
                        shape: crate::app::components::library_panel::ArtworkShape::Landscape,
                        source: None,
                        decoration: None,
                        image: crate::app::components::library_panel::content::HeroImageState::None,
                    },
                },
                overview: None,
                credits: None,
                workspace: None,
            }),
        }
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        let selection = match event {
            LibrarySlotEvent::List(input) => {
                // Typed translation "as today": the owner resolves the
                // pointer target through its own carrier, then delegates.
                let target = match input {
                    MediaListSurfaceInput::Click(at)
                    | MediaListSurfaceInput::DoubleClick(at)
                    | MediaListSurfaceInput::ContextClick(at) => {
                        self.carrier.resolve_current_point(at).cloned()
                    }
                    _ => None,
                };
                if let Some(target) = &target {
                    self.carrier.select_target(target);
                }
                if let Some(operation) = input.into_operation(target) {
                    self.carrier.delegate_operation(operation);
                }
                self.carrier.selected_target().cloned()
            }
            _ => self.carrier.selected_target().cloned(),
        };
        let mut log = self.log.borrow_mut();
        log.events.push(event);
        log.selections.push(selection);
        // A consumed pointer gesture after a list mutation reports the
        // framework's claim marker (ADR 0024).
        Some(Msg::TerminalEvent(
            crate::app::components::msg::TerminalObserverEvent::MouseClaimed,
        ))
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn mouse_event(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn draw_panel(panel: &mut LibraryPanel) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    let area = Rect::new(0, 0, 120, 30);
    terminal.draw(|f| Component::view(panel, f, area)).unwrap();
    terminal.backend().buffer().clone()
}

fn line_text(buf: &ratatui::buffer::Buffer, row: u16) -> String {
    (0..buf.area.width)
        .map(|x| buf[(x, row)].symbol())
        .collect::<String>()
}

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
    assert!(log.borrow().events.is_empty());

    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Moved,
            pill.right() + 1,
            pill.y
        )),
        None
    );
    assert_eq!(panel.test_hovered_selector(), None);
    assert!(log.borrow().events.is_empty());
}

#[test]
fn double_click_hero_policy_defaults_to_hero_bearing_and_music_overrides() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut generic = FixtureOwner::new(log);
    assert!(generic.double_click_opens_hero_overlay());

    let mut music = crate::app::components::MusicContent::new();
    assert!(!music.double_click_opens_hero_overlay());
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
            LibrarySlotEvent::List(
                crate::app::components::media_list::MediaListSurfaceInput::Click(_)
            )
        )),
        "a row click routes List delegation to the owner"
    );
    assert_eq!(
        log.selections.last(),
        Some(&Some("beta".into())),
        "the owner resolved the clicked row's typed target"
    );
}

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

/// Owner retention: dropping a key from the live set removes its owner;
/// retained owners keep their state.
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
    panel.retain_owners(&[LibraryKey::Home]);
    assert!(panel.has_owner(&LibraryKey::Home));
    assert!(!panel.has_owner(&LibraryKey::Feeds));
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
    assert!(panel
        .on(&Event::Keyboard(KeyEvent::new(
            Key::Esc,
            KeyModifiers::NONE,
        )))
        .is_some());
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
