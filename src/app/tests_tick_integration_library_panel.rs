//! Live-tick coverage for the mounted `LibraryPanel` (task 5.9). Proves, in
//! one real `Application::tick()` flow through the shell sync pass (ADR 0024,
//! design D15): framework focus follows the active library's migration state,
//! mouse eligibility follows the painted panel, painted pill clicks route
//! `SelectorPicked` to the active owner, the Wide split drag is owned by
//! the panel, an inactive owner
//! keeps its cursor/scroll across a tab change, and a library leaving the
//! catalog retires its owner. No destination converts here: the migrated
//! path is exercised through a test-owned fixture content owner, the minimal
//! owner the panel can host.

use std::cell::RefCell;
use std::rc::Rc;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow, Workspace,
};
use crate::app::components::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use crate::app::components::library_panel::{
    hero_content_emby, ArtworkShape, HeroArtwork, HeroContentData, HeroFacts, LibraryKey,
};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, MediaListSurfaceInput,
};
use crate::app::components::msg::{Msg, TerminalObserverEvent};
use crate::app::components::{LibraryKind, ComponentId};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{PanelFocus, PanelMode};

// ── Fixture content owner ───────────────────────────────────────────────

#[derive(Default)]
struct FixtureLog {
    events: Vec<LibrarySlotEvent>,
    selections: Vec<Option<String>>,
}

struct FixtureOwner {
    carrier: MediaListCarrier<String>,
    workspace_carrier: MediaListCarrier<String>,
    log: Rc<RefCell<FixtureLog>>,
    workspace: bool,
    workspace_focused: bool,
}

impl FixtureOwner {
    fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![row("alpha"), row("beta"), row("gamma")]);
        let mut workspace_carrier = MediaListCarrier::new(Presentation::Wide);
        workspace_carrier.set_content(vec![row("track one"), row("track two")]);
        Self {
            carrier,
            workspace_carrier,
            log,
            workspace: false,
            workspace_focused: false,
        }
    }

    fn enable_workspace(&mut self) {
        self.workspace = true;
        self.workspace_carrier.select_index(1);
    }

    fn workspace_state(&self) -> (usize, usize, Option<String>) {
        (
            self.workspace_carrier.cursor(),
            self.workspace_carrier.scroll(),
            self.workspace_carrier.selected_target().cloned(),
        )
    }

    fn select_multiple_for_test(&mut self) {
        self.carrier.toggle_selection(&"alpha".to_string());
        self.carrier.toggle_selection(&"beta".to_string());
    }

    fn selected_targets(&self) -> Vec<String> {
        self.carrier.multi_selection().to_vec()
    }
}

fn row(target: &str) -> MediaListRow<String> {
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

impl LibraryContentOwner for FixtureOwner {
    fn clear_selection(&mut self) {
        self.carrier.clear_selection();
    }

    fn content(&mut self) -> LibraryPanelContent<'_> {
        LibraryPanelContent {
            selector: Some(SelectorRow {
                pills: vec!["All".into(), "New".into()],
                active: Some(0),
            }),
            controls: None,
            list: ListSlot::Media(&mut self.carrier),
            hero: Some(HeroContent {
                facts: HeroFacts {
                    title: "Dune".into(),
                    meta_rows: vec!["2021".into()],
                    links: Vec::new(),
                    artwork: HeroArtwork {
                        shape: ArtworkShape::Landscape,
                        source: None,
                        image: crate::app::components::library_panel::content::HeroImageState::None,
                    },
                },
                overview: None,
                credits: None,
                workspace: self.workspace.then_some(Workspace {
                    header: Some("Tracks"),
                    selector: None,
                    list: &mut self.workspace_carrier,
                    focused: self.workspace_focused,
                }),
            }),
        }
    }

    fn focus_hero_workspace(&mut self) -> bool {
        self.workspace_focused = self.workspace;
        self.workspace_focused
    }

    fn on_slot_event(&mut self, event: LibrarySlotEvent) -> Option<Msg> {
        let selection = match event {
            LibrarySlotEvent::List(input) => {
                // The owner's typed translation "as today": resolve the
                // pointer target through its own carrier, then delegate.
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
        Some(Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn home_key() -> LibraryKey {
    LibraryKey::Home
}

fn movies_key() -> LibraryKey {
    LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-movies".into(),
        kind: LibraryKind::Movies,
    }
}

/// A Home tab whose owner has migrated: the fixture owner is pushed into the
/// mounted panel and one sync pass routes focus/eligibility to
/// `ComponentId::Library`. The base app's one Emby library is grouped Music
/// — still mounted as its old destination (task 8), so the fixture's
/// un-migrated branch is a real un-migrated library after task 6.1 moved
/// Movies into the panel.
fn migrated_home() -> (TickHarness, Rc<RefCell<FixtureLog>>) {
    let mut app = crate::app::render::make_music_group_app();
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.tab = crate::app::TabSelection::Home;
    let mut harness = TickHarness::new(app);
    let log = Rc::new(RefCell::new(FixtureLog::default()));
    harness.model_mut().sync_library_panel();
    harness
        .model_mut()
        .push_library_owner(home_key(), Box::new(FixtureOwner::new(log.clone())));
    harness.model_mut().sync_mounted_surfaces();
    (harness, log)
}

fn migrated_home_with_workspace() -> (TickHarness, Rc<RefCell<FixtureLog>>) {
    let (mut harness, log) = migrated_home();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .enable_workspace();
    harness.model_mut().sync_mounted_surfaces();
    (harness, log)
}

#[test]
fn tick_clears_multi_selection_when_library_destination_changes() {
    let (mut harness, _log) = migrated_home();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .select_multiple_for_test();
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .selected_targets(),
        vec!["alpha", "beta"]
    );
    let terminal = draw_frame(&mut harness);
    let tab = harness
        .model()
        .application
        .get_component(&ComponentId::TabPanel)
        .expect("tab panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::TabPanel>()
        .expect("tab panel component")
        .hit_regions()
        .iter()
        .find(|(_, position)| *position == 1)
        .map(|(rect, _)| *rect)
        .expect("library tab painted");
    drop(terminal);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: tab.x,
        row: tab.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .selected_targets()
            .is_empty(),
        "switching destination through the tick clears selection"
    );
}

#[test]
fn tick_context_menu_overlay_does_not_clear_multi_selection() {
    let (mut harness, _log) = migrated_home();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .select_multiple_for_test();

    // Dispatch the same typed request produced by a selected-row context
    // click. Opening the overlay changes TuiRealm's active component, but it
    // must not run the destination-identity clearing hook.
    let request = Msg::Shell(crate::app::components::msg::ShellRequest::RowContextMenu(
        crate::app::types_context_menu::ContextMenuTargets::Emby(vec![
            crate::app::tests::make_item("context", "Movie"),
        ]),
        Some((10, 10)),
    ));
    let mut music_resize = false;
    let mut tv_resize = false;
    // The shell resolves Emby context targets only for an active Emby tab;
    // restore Home before the sync pass so this test isolates overlay focus
    // from the destination-identity boundary under test.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness
        .model_mut()
        .handle_terminal_message(request, &mut music_resize, &mut tv_resize);
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    let menu_id = ComponentId::Overlay(crate::app::components::OverlayId::ContextMenu);
    assert!(harness.model().application.mounted(&menu_id));
    assert_eq!(harness.model().application.focus(), Some(&menu_id));

    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .selected_targets(),
        vec!["alpha", "beta"]
    );
}

fn draw_frame(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

/// Draw at the model's own terminal width — the breakpoint-transition draw.
fn draw_frame_sized(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let width = harness.model().app.terminal_width;
    let mut terminal = Terminal::new(TestBackend::new(width, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

/// Draw at the model's own terminal width and height — for overlay cases
/// whose Workspace box needs more rows than the default 30-row backend
/// leaves the Library pane.
fn draw_frame_at_model_size(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

fn find_text(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    for row in 0..buf.area.height {
        let mut line = String::new();
        for x in 0..buf.area.width {
            line.push_str(buf[(x, row)].symbol());
        }
        if let Some(offset) = line.find(needle) {
            return Some((offset as u16, row));
        }
    }
    None
}

#[test]
fn library_panel_focus_follows_the_active_library() {
    let (mut harness, _log) = migrated_home();
    assert_eq!(
        harness.model().application.focus().cloned(),
        Some(ComponentId::Library),
        "the migrated tab's focus is the Library panel"
    );

    // Music is migrated too: its Service owner remains behind the Library panel.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library)
    );

    // Back to the migrated tab: the panel takes focus again.
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library)
    );
}

/// Mouse eligibility follows the painted panel: the migrated surface is
/// subscribed, an un-migrated tab unsubscribes it, and a click on a painted
/// selector pill delivers `SelectorPicked` to the active owner through the
/// real subscription path.
#[test]
fn library_panel_mouse_eligibility_and_pill_slot_events() {
    let (mut harness, log) = migrated_home();
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    // Music is migrated: the panel remains the painted and subscribed boundary.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    // Draw, then click the first painted selector pill through the real
    // subscription.
    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "All").expect("the migrated owner's selector pill paints");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    let log = log.borrow();
    assert!(
        log.events
            .iter()
            .any(|event| matches!(event, LibrarySlotEvent::SelectorPicked(0))),
        "the painted pill's slot event reached the active owner"
    );
    assert!(
        outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))),
        "the owner's claim marker flows through the tick"
    );
}

/// The Wide split-boundary drag is owned by the panel:
/// through the real subscription, a press inside the panel's painted gap
/// arms only the split gesture and the drag resolves the live width.
#[test]
fn library_panel_split_drag_resolves_the_live_width() {
    let (mut harness, _log) = migrated_home();
    let terminal = draw_frame(&mut harness);
    drop(terminal);

    // The panel's painted gap, read from its retained geometry.
    let gap = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_split_gap())
        .expect("the migrated surface paints a Wide split");
    assert!(gap.width > 0 && gap.height > 0);

    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: gap.x,
        row: gap.y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();

    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: gap.x + 8,
        row: gap.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|msg| matches!(
            msg,
            Msg::Shell(crate::app::components::msg::ShellRequest::ResizeListPaneLive(_))
        )),
        "the panel's split drag resolves the live width through the tick"
    );

    // Dispatch the drag's request the way the run loop does, then the shell
    // has stored the width and the panel receives it on the next sync.
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.list_pane_width.is_some());
}

/// An inactive owner keeps its cursor/scroll across a tab change: the
/// selection made on the migrated tab survives the round trip through an
/// un-migrated tab (the owner map's retention rule, design D2).
#[test]
fn inactive_owner_keeps_cursor_scroll_across_a_tab_change() {
    let (mut harness, log) = migrated_home();

    // Click the second row: the fixture's carrier selects "beta".
    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "beta").expect("the second row paints");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(log.borrow().selections.last(), Some(&Some("beta".into())));

    // Switch to the un-migrated Emby library, then back: the owner stays.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().library_panel_has_owner(&home_key()));
    harness.model_mut().app.tab = crate::app::TabSelection::Home;
    harness.model_mut().sync_mounted_surfaces();

    // A wheel move from the retained selection lands on the NEXT row — only
    // true if the selection survived the tab change as "beta".
    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "beta").expect("the retained owner's rows repaint");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        log.borrow().selections.last(),
        Some(&Some("gamma".into())),
        "the wheel step continues from the selection made before the tab change"
    );
}

/// The mounted Library panel, for the geometry-reading tests.
fn panel_of(harness: &TickHarness) -> Option<&crate::app::components::library_panel::LibraryPanel> {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
}

/// A Wide→Narrow resize drops the stale Wide geometry (ADR 0024): the old
/// gutter no longer arms the split drag, and a click inside the freshly
/// painted narrow list — outside the stale Wide list rect — still reaches the
/// active owner instead of being silently dropped.
#[test]
fn wide_to_narrow_resize_drops_the_stale_wide_geometry() {
    let (mut harness, log) = migrated_home();
    drop(draw_frame(&mut harness));

    // The Wide frame's painted gap and list rect, read from the panel.
    let gap = panel_of(&harness)
        .and_then(|panel| panel.test_split_gap())
        .expect("the Wide frame paints a split gap");
    let wide_list = panel_of(&harness)
        .and_then(|panel| panel.test_list_rect())
        .expect("the Wide frame paints a list slot");

    // Resize to Narrow (below TWO_COLUMN_THRESHOLD, above the mini view) and
    // draw: the vanished split claims nothing and the list rect is the
    // narrow one.
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    drop(draw_frame_sized(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_split_gap())
            .is_none(),
        "the Narrow frame must not retain the Wide split's gap"
    );
    let narrow_list = panel_of(&harness)
        .and_then(|panel| panel.test_list_rect())
        .expect("the Narrow frame paints a list slot");

    // A press at the old gutter position no longer arms the split drag, so
    // the drag resolves no live width.
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: gap.x,
        row: gap.y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: gap.x + 8,
        row: gap.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        !outcome.raw_messages.iter().any(|msg| matches!(
            msg,
            Msg::Shell(crate::app::components::msg::ShellRequest::ResizeListPaneLive(_))
        )),
        "the vanished gutter must not arm the split drag"
    );

    // A click inside the newly painted narrow list, not the stale Wide list
    // rect, reaches the owner.
    let click_x = narrow_list.x + 1;
    assert!(
        !wide_list.contains(ratatui::layout::Position {
            x: click_x,
            y: narrow_list.y + 1,
        }),
        "test setup: the click must sit outside the stale Wide list rect"
    );
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: click_x,
        row: narrow_list.y + 1,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(
        log.borrow()
            .events
            .iter()
            .any(|event| matches!(event, LibrarySlotEvent::List(MediaListSurfaceInput::Click(_)))),
        "the narrow list click outside the stale Wide rect reaches the owner"
    );
}

/// Owner state survives a Panel-mode round trip (design D2's retention rule):
/// queue-only hides the library column but must not destroy the owners, so
/// the selection made before the switch still drives the wheel step after it,
/// and the panel still takes focus and resolves clicks.
#[test]
fn owner_state_survives_a_queue_only_round_trip() {
    let (mut harness, log) = migrated_home();

    // Select "beta" on the migrated Home tab.
    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "beta").expect("the second row paints");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(log.borrow().selections.last(), Some(&Some("beta".into())));

    // Queue-only hides the library column; the panel stays mounted with its
    // owners (mounted ≠ painted) instead of dropping them.
    harness.model_mut().app.panel_mode = PanelMode::QueueOnly;
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().library_panel_has_owner(&home_key()),
        "the hidden library column must not destroy the owner map"
    );

    // Back to library-only: the selection survived and the panel routes
    // clicks again.
    harness.model_mut().app.panel_mode = PanelMode::LibraryOnly;
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Library),
        "the panel takes focus again after the round trip"
    );
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    let terminal = draw_frame(&mut harness);
    let buf = terminal.backend().buffer();
    let (x, y) = find_text(buf, "beta").expect("the retained owner's rows repaint");
    harness.inject(tuirealm::event::Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        log.borrow().selections.last(),
        Some(&Some("gamma".into())),
        "the wheel step continues from the selection made before the mode switch"
    );
}

/// Enter and browser double-click are delivered through the mounted panel
/// after the shell sync pass. Both open the Library-local overlay; the mouse
/// path resolves the clicked row rather than the pre-existing cursor.
#[test]
fn mounted_narrow_activation_opens_overlay_for_leaf_and_workspace() {
    let (mut harness, _log) = migrated_home();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    drop(draw_frame_sized(&mut harness));
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code: tuirealm::event::Key::Enter,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|msg| matches!(
        msg,
        Msg::TerminalEvent(TerminalObserverEvent::KeyClaimed)
    )));
    drop(draw_frame_sized(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());

    let (mut harness, log) = migrated_home_with_workspace();
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();
    let terminal = draw_frame_sized(&mut harness);
    let beta = find_text(terminal.backend().buffer(), "beta").expect("first browser row");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: beta.0,
        row: beta.1,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(log.borrow().selections.last(), Some(&Some("beta".into())));

    let terminal = draw_frame_sized(&mut harness);
    let gamma = find_text(terminal.backend().buffer(), "gamma").expect("second browser row");
    for _ in 0..2 {
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: gamma.0,
            row: gamma.1,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(outcome.raw_messages.iter().any(|message| {
            matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
        }));
    }
    assert_eq!(log.borrow().selections.last(), Some(&Some("gamma".into())));
    drop(draw_frame_sized(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .workspace_state(),
        (1, 0, Some("track two".into()))
    );
}

/// The Library overlay is local to its panel: Queue can take focus and handle
/// a normal action without dismissing it, then Library focus restores the
/// retained overlay state.
#[test]
fn mounted_queue_action_preserves_unfocused_library_overlay() {
    let (mut harness, _log) = migrated_home_with_workspace();
    harness.model_mut().app.panel_mode = PanelMode::Both;
    harness.model_mut().sync_mounted_surfaces();
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any_mut()
                .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
        })
        .expect("Library panel mounted")
        .test_open_hero_overlay();
    harness
        .model_mut()
        .library_owner_mut::<FixtureOwner>(&home_key())
        .expect("fixture owner installed")
        .focus_hero_workspace();
    let workspace_state = harness
        .model()
        .library_owner::<FixtureOwner>(&home_key())
        .unwrap()
        .workspace_state();
    drop(draw_frame(&mut harness));
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().application.focus(), Some(&ComponentId::Queue));
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code: tuirealm::event::Key::Down,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    harness.inject(Event::Keyboard(tuirealm::event::KeyEvent {
        code: tuirealm::event::Key::Esc,
        modifiers: tuirealm::event::KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some(), "Queue Esc must not dismiss Library overlay");
    drop(draw_frame(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .workspace_state(),
        workspace_state,
        "Queue input preserves the overlay Workspace state"
    );
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().application.focus(), Some(&ComponentId::Library));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert_eq!(
        harness
            .model()
            .library_owner::<FixtureOwner>(&home_key())
            .unwrap()
            .workspace_state(),
        workspace_state,
        "returning to Library restores the Workspace state"
    );
}

// ── Library Hero overlay Workspace focus (bug-fix unit) ────────────────
//
// The overlay is the only non-Wide surface that focuses a Workspace: on
// open the destination's Workspace list must receive the destination's
// local keys (and the pointer's row resolution), keep them across ordinary
// refresh and Queue focus/return, and never let the covered browser list
// move. Exercised through the real TV and Music owners (their key routing
// is where the narrow fall-through lived), not the fixture owner, whose
// `on_key` is a no-op.

/// A narrow-geometry TV tab whose selected Series' season detail is cached:
/// the mounted panel hosts the real `TvContent` owner with a paintable
/// Workspace (`episode_count` episodes), tall enough for the overlay's
/// Workspace box.
fn migrated_tv_with_detail(episode_count: usize) -> TickHarness {
    let mut app = crate::app::render::make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    for item in &mut app.libs[0].nav_stack[0].items {
        item.item_type = "Series".into();
        item.image_tags.thumb = "tag".into();
    }
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 80;
    app.terminal_height = 60;
    let mut season = crate::app::tests::make_item("Season 1", "Season");
    season.id = "season-1".into();
    let episodes: Vec<_> = (1..=episode_count)
        .map(|index| {
            let mut episode =
                crate::app::tests::make_item(&format!("Episode {index}"), "Episode");
            episode.id = format!("episode-{index}");
            episode
        })
        .collect();
    app.series_detail_cache.insert(
        "movie-focused".into(),
        crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), episodes)].into_iter().collect(),
        },
    );
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn tv_owner_of(harness: &TickHarness) -> &crate::app::components::tv_content::TvContent {
    harness
        .model()
        .library_owner::<crate::app::components::tv_content::TvContent>(
            &harness.model().test_tv_owner_key(),
        )
        .expect("tv owner installed")
}

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
            .and_then(|panel| panel.test_overlay_geometry())
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
        Msg::Shell(crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: 1 })
    )));
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
    harness
        .model_mut()
        .app
        .series_detail_cache
        .insert("movie-focused".into(), crate::app::SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), episodes)].into_iter().collect(),
        });
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        panel_of(&harness)
            .map(|panel| panel.test_hero_overlay_open())
            .unwrap_or(false),
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
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());

    let (x, y) = find_text(terminal.backend().buffer(), "Episode 2")
        .expect("the Workspace row paints");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(crate::app::components::msg::ShellRequest::TvHitClick {
            hit: crate::app::components::msg::TvHit::EpisodeRow(target),
        }) if target == "episode-2"
    )));
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

    let mut harness = migrated_tv_with_detail(30);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_at_model_size(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert_eq!(tv_owner_of(&harness).episode_scroll(), 0);

    // More Down steps than the Workspace box's painted rows: the viewport
    // must follow the cursor past its bottom edge.
    for _ in 0..14 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
    }
    let terminal = draw_frame_at_model_size(&mut harness);
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        14,
        "the cursor moved through the overflowing list"
    );
    assert!(
        tv_owner_of(&harness).episode_scroll() > 0,
        "the Workspace viewport must follow the cursor with overflow"
    );
    let buf = terminal.backend().buffer();
    assert!(
        find_text(buf, "15. Episode 15").is_some(),
        "the cursor's row scrolled into the Workspace box"
    );
    // Not "1. Episode 1": that string is a substring of "11. Episode 11".
    // Row 6 is the last row above the scrolled-in window.
    assert!(
        find_text(buf, "6. Episode 6").is_none(),
        "the first rows scrolled out of the Workspace box"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );
}

/// A wheel over the overlay's Workspace rows scrolls the episode list and
/// is claimed; the covered browser list never scrolls from it.
#[test]
fn overlay_workspace_wheel_scrolls_and_is_claimed() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(30);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());

    // Walk the cursor into the overflow first (the keyboard path, already
    // covered above): the wheel's single notch must then scroll a following
    // viewport, not a reset one. One notch = one cursor step (the canonical
    // wheel = Move translation); the 30 ms burst throttle collapses rapid
    // notches, so the test drives exactly one recognized gesture.
    for _ in 0..14 {
        harness.inject(Event::Keyboard(KeyEvent {
            code: Key::Down,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
    }
    drop(draw_frame_at_model_size(&mut harness));
    let scroll_before = tv_owner_of(&harness).episode_scroll();
    assert!(scroll_before > 0, "test setup: the viewport is in overflow");

    let (x, y) = find_text(terminal.backend().buffer(), "Episode 2")
        .or_else(|| find_text(terminal.backend().buffer(), "Episode 3"))
        .or_else(|| find_text(terminal.backend().buffer(), "Episode 4"))
        .or_else(|| find_text(terminal.backend().buffer(), "Episode 5"))
        .expect("a Workspace row paints");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed)
    )));
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        15,
        "the wheel stepped the Workspace cursor"
    );
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        tv_owner_of(&harness).episode_scroll() > scroll_before,
        "the wheel scrolled the Workspace viewport past its bottom edge"
    );
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some(),
        "the wheel leaves the overlay open");
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );
}

/// A narrow Music overlay whose album tracks are still fetching: opening
/// cannot focus an empty Workspace, so the deferred track push restores the
/// focus when the rows arrive; Down then moves the track list (never the
/// album browser) and keeps doing so after a Queue focus round trip.
#[test]
fn overlay_open_focuses_a_late_workspace_and_keeps_its_keys() {
    use crate::app::components::MusicContent;
    use tuirealm::event::{Key, KeyEvent};

    let music_key = LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: LibraryKind::Music,
    };
    let mut app = crate::app::render::make_music_group_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 80;
    app.terminal_height = 60;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().sync_mounted_surfaces();
    drop(draw_frame_sized(&mut harness));

    // Open while the selected album's tracks are not cached yet.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_sized(&mut harness));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_some());
    assert!(
        !harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .expect("music owner installed")
            .track_focused(),
        "an empty Workspace cannot take focus yet"
    );

    // The provider completion: the tracks arrive on the next sync pass.
    let tracks: Vec<_> = (1..=2)
        .map(|index| {
            let mut track = crate::app::tests::make_item(&format!("Track {index}"), "Audio");
            track.id = format!("track-{index}");
            track
        })
        .collect();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert("album-1".into(), tracks);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .expect("music owner installed")
            .track_focused(),
        "the arriving Workspace rows restore the open-time focus"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(!outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(crate::app::components::msg::ShellRequest::MusicAlbumCursor { .. })
    )));
    assert_eq!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .selected_track_item()
            .map(|track| track.id),
        Some("track-2".into()),
        "Down moves the overlay Workspace's track list"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[1].resting().cursor(),
        0,
        "the covered album browser must not move"
    );

    // Queue takes focus and gives it back: the Workspace keys keep working.
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    harness.model_mut().app.panel_focus = PanelFocus::Library;
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Up,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .selected_track_item()
            .map(|track| track.id),
        Some("track-1".into()),
        "the Workspace keeps its keys across the Queue focus round trip"
    );
}


/// A library leaving the catalog retires its owner: the catalog-retention
/// rule (moved inside from `reconcile_destination_mounts`) runs in the sync
/// pass.
#[path = "tests_tick_integration_library_panel_hero.rs"]
mod tests_tick_integration_library_panel_hero;
