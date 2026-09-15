use crate::app::components::library_panel::{
    LibraryContentOwner, LibraryKey, LibraryPanel, LibrarySlotEvent,
};
use crate::app::components::media_list::MediaListSurfaceInput;
use crate::app::components::msg::LeafKeyResult;
use crate::app::components::{Msg, ShellRequest, TerminalObserverEvent, UserEvent};
use tuirealm::component::{AppComponent, Component};
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use tuirealm::props::{AttrValue, Attribute};

use crate::app::components::library_panel::content::{
    HeroContent, LibraryPanelContent, ListSlot, SelectorRow,
};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation,
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
    workspace_carrier: MediaListCarrier<String>,
    log: Rc<RefCell<FixtureLog>>,
    link: bool,
    workspace: bool,
    inline_search: bool,
    workspace_focused: bool,
}

impl FixtureOwner {
    fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![item("alpha"), item("beta"), item("gamma")]);
        let mut workspace_carrier = MediaListCarrier::new(Presentation::Wide);
        workspace_carrier.set_content(vec![item("track one"), item("track two")]);
        Self {
            carrier,
            workspace_carrier,
            log,
            link: false,
            workspace: false,
            inline_search: false,
            workspace_focused: false,
        }
    }

    fn with_inline_search(mut self) -> Self {
        self.inline_search = true;
        self
    }

    fn with_link(mut self) -> Self {
        self.link = true;
        self
    }

    fn with_workspace(mut self) -> Self {
        self.workspace = true;
        self
    }
}

impl LibraryContentOwner for FixtureOwner {
    fn content(&mut self) -> LibraryPanelContent<'_> {
        LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into(), "New".into()],
                active: Some(0),
            }),
            controls: None,
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
                    artwork: crate::app::components::library_panel::HeroArtwork {
                        shape: crate::app::components::library_panel::ArtworkShape::Landscape,
                        source: None,
                        image: crate::app::components::library_panel::content::HeroImageState::None,
                    },
                },
                overview: None,
                credits: None,
                workspace: self.workspace.then_some(
                    crate::app::components::library_panel::content::Workspace {
                        header: Some("Tracks"),
                        selector: None,
                        list: &mut self.workspace_carrier,
                        focused: false,
                    },
                ),
            }),
        }
    }

    fn inline_search_active(&self) -> bool {
        self.inline_search
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.workspace_focused = self.workspace;
        self.workspace_focused
    }

    fn clear_hero_workspace_focus(&mut self) {
        self.workspace_focused = false;
    }

    fn on_key_result(&mut self, key: &KeyEvent) -> LeafKeyResult {
        if key.modifiers.is_empty() && key.code == Key::Enter && self.workspace_focused {
            return LeafKeyResult::Consumed(Some(Msg::TerminalEvent(
                TerminalObserverEvent::KeyClaimed,
            )));
        }
        LeafKeyResult::Unhandled
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

fn draw_panel_at(panel: &mut LibraryPanel, area: Rect) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(area.width, area.height)).unwrap();
    terminal.draw(|f| Component::view(panel, f, area)).unwrap();
    terminal.backend().buffer().clone()
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
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
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
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
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
fn list_click_delegates_to_the_active_owner() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
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
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
    let _ = draw_panel(&mut panel);
    let gap = panel.test_split_gap().expect("a wide split painted");
    assert!(gap.width > 0 && gap.height > 0);

    // Press inside the gap, drag right, release.
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            gap.x,
            gap.y
        )),
        None,
        "press-and-release without motion changes nothing"
    );
    let msg = panel.on(&mouse_event(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x + 6,
        gap.y,
    ));
    match msg {
        Some(Msg::Shell(ShellRequest::ResizeListPaneLive(width))) => {
            assert!(width > 0, "the drag resolves a live width: {width}");
        }
        other => panic!("the drag must resolve the live width: {other:?}"),
    }
    // DragEnd is a live-only no-op.
    assert_eq!(
        panel.on(&mouse_event(
            MouseEventKind::Up(MouseButton::Left),
            gap.x + 6,
            gap.y
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
        Some(Msg::Shell(ShellRequest::OpenUrl(
            "https://imdb.test/dune".into()
        )))
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
fn overlay_buffer_covers_leaf_and_workspace_hero_content() {
    for (workspace, expected) in [(false, "Dune"), (true, "track")] {
        let mut panel = LibraryPanel::new();
        panel.set_active(Some(LibraryKey::Home));
        let owner = FixtureOwner::new(Rc::new(RefCell::new(FixtureLog::default())));
        let owner = if workspace {
            owner.with_workspace()
        } else {
            owner
        };
        panel.insert_owner(LibraryKey::Home, Box::new(owner));
        let normal = draw_panel(&mut panel);
        panel.test_open_hero_overlay();
        let overlay = draw_panel(&mut panel);
        let (pane, frame) = panel.test_overlay_geometry().expect("overlay was painted");
        assert!(line_text(&overlay, frame.y).contains("Library Hero"));
        assert!(
            (0..30).any(|y| line_text(&overlay, y).contains(expected)),
            "expected {expected}"
        );
        let outside = (pane.y..pane.bottom())
            .flat_map(|y| (pane.x..pane.right()).map(move |x| (x, y)))
            .find(|&(x, y)| !frame.contains((x, y).into()))
            .expect("backdrop cell outside frame");
        assert_ne!(
            overlay[outside].bg, normal[outside].bg,
            "Library backdrop is dimmed"
        );
        let inside = (frame.x + 1, frame.y + 1);
        assert_eq!(
            overlay[inside].bg, normal[inside].bg,
            "Hero content inside the frame is not dimmed"
        );
    }
}

#[test]
fn non_wide_enter_opens_overlay_but_inline_search_enter_does_not() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    panel.attr(Attribute::Focus, AttrValue::Flag(true));
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));
    let msg = panel.on(&Event::Keyboard(KeyEvent::new(
        Key::Enter,
        KeyModifiers::NONE,
    )));
    assert!(matches!(
        msg,
        Some(Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed))
    ));
    assert!(
        panel.test_overlay_geometry().is_none(),
        "geometry is painted on the next frame"
    );
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));
    assert!(panel.test_overlay_geometry().is_some());

    let mut search_panel = LibraryPanel::new();
    search_panel.set_active(Some(LibraryKey::Home));
    search_panel.insert_owner(
        LibraryKey::Home,
        Box::new(
            FixtureOwner::new(Rc::new(RefCell::new(FixtureLog::default()))).with_inline_search(),
        ),
    );
    search_panel.attr(Attribute::Focus, AttrValue::Flag(true));
    let _ = draw_panel_at(&mut search_panel, Rect::new(0, 0, 80, 30));
    assert!(search_panel
        .on(&Event::Keyboard(KeyEvent::new(
            Key::Enter,
            KeyModifiers::NONE
        )))
        .is_none());
    assert!(search_panel.test_overlay_geometry().is_none());
}

#[test]
fn browser_double_click_opens_overlay_and_workspace_activation_stays_open() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(FixtureLog::default()))).with_workspace()),
    );
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));
    let list = panel.test_list_rect().expect("narrow list painted");
    let point = (list.x + 1, list.y + 1);
    let _ = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    let msg = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    assert!(matches!(
        msg,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));
    assert!(panel.test_overlay_geometry().is_some());
    let frame = panel.test_overlay_geometry().unwrap().1;
    let point = (frame.x + 2, frame.y + 2);
    let _ = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    let msg = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    assert!(matches!(
        msg,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert!(panel.test_overlay_geometry().is_some());
}

#[test]
fn overlay_esc_dismisses_and_destination_change_or_missing_parent_dismisses() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    panel.test_open_hero_overlay();
    panel.attr(Attribute::Focus, AttrValue::Flag(true));
    assert!(panel
        .on(&Event::Keyboard(KeyEvent::new(
            Key::Esc,
            KeyModifiers::NONE
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
fn closed_overlay_does_not_swallow_local_keys() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    panel.attr(Attribute::Focus, AttrValue::Flag(true));
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));

    for key in [Key::Down, Key::Char('x')] {
        assert!(
            panel
                .on(&Event::Keyboard(KeyEvent::new(key, KeyModifiers::NONE)))
                .is_none(),
            "closed overlay must not claim {key:?}"
        );
    }
}

#[test]
fn overlay_chrome_click_is_claimed_without_browser_selection() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));
    panel.test_open_hero_overlay();
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 80, 30));
    let frame = panel.test_overlay_geometry().unwrap().1;
    let point = (frame.x..frame.right())
        .flat_map(|x| (frame.y..frame.bottom()).map(move |y| (x, y)))
        .find(|&(x, y)| {
            x == frame.x || y == frame.y || x + 1 == frame.right() || y + 1 == frame.bottom()
        })
        .expect("overlay chrome");
    let before = log.borrow().selections.clone();
    assert!(matches!(
        panel.on(&mouse_event(
            MouseEventKind::Down(MouseButton::Left),
            point.0,
            point.1
        )),
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(log.borrow().selections, before);
}

#[test]
fn overlay_paints_and_retains_library_geometry() {
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(
        LibraryKey::Home,
        Box::new(FixtureOwner::new(Rc::new(RefCell::new(
            FixtureLog::default(),
        )))),
    );
    panel.test_open_hero_overlay();
    let _ = draw_panel(&mut panel);
    let (pane, frame) = panel.test_overlay_geometry().expect("overlay was painted");
    assert!(frame.x >= pane.x && frame.right() <= pane.right());
    assert!(frame.y >= pane.y && frame.bottom() <= pane.bottom());
}

#[test]
fn overlay_dismissal_consumes_backdrop_without_selection_mutation() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
    let _ = draw_panel(&mut panel);
    let list = panel.test_list_rect().expect("list was painted");
    panel.test_open_hero_overlay();
    let _ = draw_panel(&mut panel);
    let (pane, frame) = panel.test_overlay_geometry().unwrap();
    let point = (pane.y..pane.bottom())
        .flat_map(|y| (pane.x..pane.right()).map(move |x| (x, y)))
        .find(|&(x, y)| !frame.contains(ratatui::layout::Position::new(x, y)))
        .expect("dimmed Library remainder");
    let before = log.borrow().selections.clone();
    let msg = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        point.0,
        point.1,
    ));
    assert!(matches!(
        msg,
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    ));
    assert_eq!(log.borrow().selections, before);
    assert!(panel.test_overlay_geometry().is_none());
    assert!(list.width > 0);
}

#[test]
fn overlay_refresh_uses_latest_geometry_and_blocks_old_browser_point() {
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    let mut panel = LibraryPanel::new();
    panel.set_active(Some(LibraryKey::Home));
    panel.insert_owner(LibraryKey::Home, Box::new(FixtureOwner::new(log.clone())));
    let _ = draw_panel(&mut panel);
    let old_list = panel.test_list_rect().unwrap();
    panel.test_open_hero_overlay();
    let _ = draw_panel_at(&mut panel, Rect::new(0, 0, 100, 30));
    let (_, frame) = panel.test_overlay_geometry().unwrap();
    let point = (old_list.y..old_list.bottom())
        .flat_map(|y| {
            (old_list.x..old_list.right()).map(move |x| ratatui::layout::Position::new(x, y))
        })
        .find(|point| frame.contains(*point))
        .expect("old browser geometry overlaps refreshed frame");
    let msg = panel.on(&mouse_event(
        MouseEventKind::Down(MouseButton::Left),
        point.x,
        point.y,
    ));
    assert!(msg.is_some());
    assert!(
        log.borrow().events.is_empty(),
        "covered browser did not receive click"
    );
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
