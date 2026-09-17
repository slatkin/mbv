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
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, MediaListSurfaceInput,
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
        let mut carrier = MediaListCarrier::new();
        carrier.set_content(vec![row("alpha"), row("beta"), row("gamma")]);
        let mut workspace_carrier = MediaListCarrier::new();
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
                    duration_row: None,
                    links: Vec::new(),
                    artwork: HeroArtwork {
                        shape: ArtworkShape::Landscape,
                        source: None,
                        decoration: None,
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

/// The first painted occurrence of `needle` inside `area`'s rows, for
/// locating a browser row that another painted surface (a Wide Hero pane's
/// title) also shows.
fn find_text_in(
    buf: &ratatui::buffer::Buffer,
    needle: &str,
    area: ratatui::layout::Rect,
) -> Option<(u16, u16)> {
    for row in area.top()..area.bottom() {
        let mut line = String::new();
        for x in area.left()..area.right() {
            line.push_str(buf[(x, row)].symbol());
        }
        if let Some(offset) = line.find(needle) {
            return Some((area.left() + offset as u16, row));
        }
    }
    None
}

/// A left double-click at `(x, y)`: two press+step pairs, the gesture the
/// list slot's activation seam reads.
fn double_click(harness: &mut TickHarness, x: u16, y: u16) {
    for _ in 0..2 {
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }));
        let _ = harness.step();
    }
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
    // The viewport is the painted Workspace box's content rows: re-derive
    // the expected window from the box the frame actually painted (the
    // overlay's size is an arrangement fact, not this test's input).
    let (_, box_content) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box painted");
    let visible = box_content.height as usize;
    let scroll = tv_owner_of(&harness).episode_scroll();
    assert!(
        scroll > 0,
        "the Workspace viewport must follow the cursor with overflow"
    );
    assert_eq!(
        scroll,
        14 + 1 - visible,
        "the viewport follows the cursor: the cursor's row is the window's last row"
    );
    // The last row above the scrolled-in window has left the box. (Probing
    // row 1 is a substring of row 11's label, so only probe row numbers
    // whose label cannot match a longer one.)
    if scroll >= 2 {
        assert!(
            find_text(buf, &format!("{}. Episode {scroll}", scroll)).is_none(),
            "the rows above the window left the Workspace box"
        );
    }
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );
}

/// The double-click-to-overlay gate is the non-Wide frame itself (design
/// D4): `narrow_geometry` is `Some` exactly when a non-Wide frame painted.
/// A non-Wide browser double-click opens the Library Hero overlay; the same
/// double-click in Wide geometry stays in the list slot and never does.
#[test]
fn browser_double_click_opens_the_overlay_only_in_non_wide_geometry() {
    // Non-Wide (80 < TWO_COLUMN_THRESHOLD): the gate is reported and the
    // gesture opens the overlay.
    let mut harness = migrated_tv_with_detail(2);
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_narrow_geometry())
            .is_some(),
        "a non-Wide frame reports the overlay gate"
    );
    let terminal = draw_frame_at_model_size(&mut harness);
    let (x, y) = find_text(terminal.backend().buffer(), "Focused Movie")
        .expect("the browser row paints");
    double_click(&mut harness, x, y);
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "a non-Wide browser double-click opens the Library Hero overlay"
    );

    // The same gesture in Wide geometry: the list slot takes the
    // double-click and the overlay stays closed.
    let mut harness = migrated_tv_with_detail(2);
    harness.model_mut().app.terminal_width = 100;
    drop(draw_frame_at_model_size(&mut harness));
    let wide_painted = panel_of(&harness)
        .and_then(|panel| panel.test_wide_geometry())
        .is_some();
    assert!(wide_painted, "the frame is Wide");
    let gate_reported = panel_of(&harness)
        .and_then(|panel| panel.test_narrow_geometry())
        .is_some();
    assert!(!gate_reported, "a Wide frame reports no overlay gate");
    let terminal = draw_frame_at_model_size(&mut harness);
    let list = panel_of(&harness)
        .and_then(|panel| panel.test_list_rect())
        .expect("the Wide list painted");
    let (x, y) = find_text_in(terminal.backend().buffer(), "Focused Movie", list)
        .expect("the Wide browser row paints in the list slot");
    double_click(&mut harness, x, y);
    drop(draw_frame_at_model_size(&mut harness));
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_none(),
        "a Wide double-click must not take the overlay path"
    );
}

/// The saved-geometry shift (unify-narrow U2/6.1): the non-Wide list slot's
/// hit rect is the Wide pane's row-flow inset, so a click on the painted
/// list box's outer two columns is padding and is not delegated to the list;
/// a click inside the row flow still selects the row under it.
#[test]
fn non_wide_padding_click_is_not_delegated_and_a_row_flow_click_selects() {
    let (mut harness, log) = migrated_home();
    drop(draw_frame_at_model_size(&mut harness));
    let narrow = panel_of(&harness)
        .and_then(|panel| panel.test_narrow_geometry())
        .expect("the frame is non-Wide");

    // The outer two columns of the painted list box: on the pane the panel
    // painted, but outside the row flow it retained as the hit rect.
    let (edge_x, edge_y) = (narrow.list_panel.x + 1, narrow.list_panel.y + 2);
    assert!(narrow.list_panel.contains(ratatui::layout::Position {
        x: edge_x,
        y: edge_y,
    }), "the click lands on the painted pane");
    assert!(
        !narrow.list_area.contains(ratatui::layout::Position {
            x: edge_x,
            y: edge_y,
        }),
        "the click lands in the padding outside the row flow inset"
    );
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: edge_x,
        row: edge_y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert!(
        log.borrow().events.is_empty(),
        "a padding click must not reach the list"
    );

    // A click inside the row flow still selects the row under it.
    let terminal = draw_frame_at_model_size(&mut harness);
    let (x, y) = find_text_in(terminal.backend().buffer(), "alpha", narrow.list_area)
        .expect("the first browser row paints in the row flow");
    drop(terminal);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    assert_eq!(
        log.borrow().selections.last(),
        Some(&Some("alpha".to_string())),
        "the row under a row-flow click is selected"
    );
}

/// The non-Wide saved geometry agrees with the painted frame (unify-narrow
/// 6.2, ADR 0024): the hit rect is the row-flow inset, the selected-row rect
/// is the painted cursor row, and the context-menu anchor is the painted
/// Browser pane — not the inset.
#[test]
fn non_wide_saved_geometry_agrees_with_the_painted_frame() {
    let (mut harness, _log) = migrated_home();
    let terminal = draw_frame_at_model_size(&mut harness);
    let narrow = panel_of(&harness)
        .and_then(|panel| panel.test_narrow_geometry())
        .expect("the frame is non-Wide");
    let panel = panel_of(&harness).expect("the Library panel is mounted");

    // The hit rect is the row-flow inset, not the pane.
    assert_eq!(panel.test_list_rect(), Some(narrow.list_area));
    assert_ne!(narrow.list_area, narrow.list_panel);

    // The context-menu anchor is the painted Browser pane.
    let (anchor, selected) = panel.menu_geometry().expect("the frame painted");
    assert_eq!(anchor, narrow.list_panel);
    assert_ne!(
        anchor, narrow.list_area,
        "the anchor is the pane, not the inset"
    );

    // The selected-row rect is the painted cursor row: the cursor row's text
    // paints inside it, and it sits in the row flow.
    let selected = selected.expect("the cursor row painted");
    let (x, y) = find_text_in(terminal.backend().buffer(), "alpha", selected)
        .expect("the cursor row paints in the selected-row rect");
    assert_eq!(y, selected.y);
    assert!(narrow.list_area.contains(ratatui::layout::Position { x, y }));
}

/// The Library Hero overlay paints its Workspace box from the Workspace's own
/// focus: the sheet carries its PillRow surface, and the box's body carries the
/// focused `MainContentBox` fill (`#48584e`) while the Workspace holds focus,
/// with the list's rows striped in its focused `LibraryPanel` fill. The resting
/// half of the same resolution is pinned by
/// `wide_tests::unfocused_workspace_hero_renders_resting_surfaces`.
#[test]
fn overlay_sheet_and_workspace_box_paint_the_workspace_focus() {
    use crate::app::palette::{surface_colors, Surface};
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(6);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the overlay is open in non-Wide geometry"
    );
    let buf = terminal.backend().buffer();

    // The sheet's own surface: the overlay frame's top-left corner cell.
    let (_, frame) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .expect("the overlay painted");
    assert_eq!(
        buf[(frame.x, frame.y)].bg,
        surface_colors(Surface::PillRow, false).fill,
        "the overlay sheet carries the PillRow surface"
    );

    // The Workspace box takes the focused `MainContentBox` fill while the
    // Workspace holds focus. A whole-box "no resting cell" scan is no longer
    // expressible: the selected row's bar shares the resting fill's value, so
    // the box body's focused fill is proven on its own padding row below.
    let (box_panel, _) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box painted");
    let resting_body = surface_colors(Surface::MainContentBox, false).fill;
    let focused_body = surface_colors(Surface::MainContentBox, true).fill;
    assert_ne!(resting_body, focused_body);

    // The box's body row carries the focused `MainContentBox` fill and the
    // list's rows stripe with the Workspace's own focused `LibraryPanel`
    // fill.
    assert_eq!(
        buf[(box_panel.x + 1, box_panel.bottom() - 1)].bg,
        focused_body,
        "the box's padding row carries the focused MainContentBox body"
    );
    let focused_stripe = surface_colors(Surface::LibraryPanel, true).fill;
    let striped = (box_panel.top()..box_panel.bottom()).any(|y| {
        (box_panel.left()..box_panel.right()).any(|x| buf[(x, y)].bg == focused_stripe)
    });
    assert!(
        striped,
        "the Workspace box stripes its rows with the Workspace's focused LibraryPanel fill"
    );

    // The Queue column taking focus drops the same box to the resting fill
    // while the overlay stays open: the box follows the Workspace's focus, not
    // the overlay's existence.
    harness.model_mut().app.panel_mode = PanelMode::Both;
    harness.model_mut().app.terminal_width = 100;
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.model_mut().sync_mounted_surfaces();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the overlay stays open while the Queue holds focus"
    );
    let (box_panel, _) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box stays painted");
    assert_eq!(
        terminal.backend().buffer()[(box_panel.x + 1, box_panel.bottom() - 1)].bg,
        resting_body,
        "an unfocused Workspace box rests while the Queue holds focus"
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

/// A narrow Music overlay whose album tracks are still fetching: the
/// Workspace's rows arrive on a later sync pass and take the focus the open
/// transition could not, so Down moves the track list (never the album
/// browser) without a second Enter, and the focus survives a Queue focus
/// round trip; an explicit shell focus clear still wins and stays won.
#[test]
fn late_overlay_workspace_takes_the_focus_when_its_rows_arrive() {
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
        "the Workspace's arriving rows take the focus the open transition could not"
    );
    assert_eq!(
        harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .selected_track_item()
            .map(|track| track.id),
        Some("track-1".into()),
        "the arriving rows seed the cursor on the first track"
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

    // An explicit shell focus clear wins while the overlay is open and
    // stays won: no follow-up push re-seizes the focus.
    harness.model_mut().music_track_focus_request =
        Some(crate::app::shell::MusicTrackFocusRequest::Clear);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .track_focused(),
        "the shell's focus clear takes effect over the open overlay"
    );
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness
            .model()
            .library_owner::<MusicContent>(&music_key)
            .unwrap()
            .track_focused(),
        "no push re-seizes the focus after the shell's clear"
    );
}

/// The overlay Workspace's pager delegates to the episode list's own page
/// stride (the shared owner's canonical page operation, like Music's
/// overlay pager) and reports the applied movement as the component-
/// resolved `TvEpisodeMove` delta — never recomputed from painted row
/// arithmetic that has no Narrow geometry behind it.
#[test]
fn overlay_workspace_pager_moves_by_the_episode_lists_own_stride() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(10);
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
            .is_some()
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::PageDown,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: 5 })
        )),
        "PageDown moves by the episode list's own page stride"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        5,
        "PageDown lands five rows down"
    );
    assert_eq!(
        harness.model().app.libs[0].nav_stack[0].resting().cursor(),
        0,
        "the covered browser list must not move"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::PageUp,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: -5 })
        )),
        "PageUp reports the applied movement too"
    );
    assert_eq!(tv_owner_of(&harness).episode_cursor(), 0);
}

/// The overlay Workspace's Home/End mutate the episode list locally AND
/// report the component-resolved movement, exactly like the sibling
/// movement chords — the shell never recomputes a jump the component made.
#[test]
fn overlay_workspace_home_end_report_the_resolved_move() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(10);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    drop(draw_frame_at_model_size(&mut harness));

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::End,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: 9 })
        )),
        "End reports the resolved jump to the last row"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        9,
        "End lands on the last row"
    );

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Home,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(crate::app::components::msg::ShellRequest::TvEpisodeMove { delta: -9 })
        )),
        "Home reports the resolved jump to the first row"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        0,
        "Home lands on the first row"
    );
}

/// A stale overlay-open bit must never let the narrow overlay path shadow
/// Wide handling: after a narrow->wide resize the overlay's keyboard arms
/// are inert and the Wide workspace's own Home reaches the series rail.
#[test]
fn stale_overlay_bit_never_shadows_wide_keyboard_handling() {
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

    // Resize narrow -> wide: the sync pass re-pushes the owner's actual
    // breakpoint before the next key is delivered.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().app.terminal_height = 40;
    assert!(
        harness.model().app.wide_tv_library_area(0).is_some(),
        "the resized terminal is Wide-eligible"
    );
    harness.model_mut().sync_mounted_surfaces();

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Home,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(crate::app::components::msg::ShellRequest::TvJumpCursor {
                to_end: false
            })
        )),
        "the Wide workspace's Home reaches the series rail, not the overlay arms"
    );
    assert_eq!(
        tv_owner_of(&harness).episode_cursor(),
        0,
        "the overlay's episode list did not move"
    );
}


/// A library leaving the catalog retires its owner: the catalog-retention
/// rule (moved inside from `reconcile_destination_mounts`) runs in the sync
/// pass.
#[path = "tests_tick_integration_library_panel_hero.rs"]
mod tests_tick_integration_library_panel_hero;

/// The overlay's focused Workspace paints its cursor: every row of the
/// canonical list that holds focus resolves the list's own focused emphasis
/// (the selected row's bold title), and the Workspace box's own body fill
/// follows the Workspace's focus as the Wide Hero pane's does.
#[test]
fn overlay_workspace_paints_its_cursor_row() {
    use tuirealm::event::{Key, KeyEvent};

    let mut harness = migrated_tv_with_detail(6);
    drop(draw_frame_at_model_size(&mut harness));
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Enter,
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let terminal = draw_frame_at_model_size(&mut harness);
    assert!(
        panel_of(&harness)
            .and_then(|panel| panel.test_overlay_geometry())
            .is_some(),
        "the overlay is open in non-Wide geometry"
    );
    let (_, content) = panel_of(&harness)
        .and_then(|panel| panel.test_overlay_workspace_box())
        .expect("the overlay's Workspace box painted");
    // TV's Workspace carries no header row, so the only bar row inside the
    // box's content is the cursor's own selected row.
    let buf = terminal.backend().buffer();
    let cursor_row = (content.top()..content.bottom()).find(|&y| {
        (content.left()..content.right())
            .any(|x| buf[(x, y)].bg == crate::app::palette::SELECTED_ROW_BG)
    });
    assert!(
        cursor_row.is_some(),
        "the overlay's focused Workspace must paint a visible cursor row"
    );
    let first_episode = find_text_in(buf, "Episode 1", content)
        .expect("the first episode row paints")
        .1;
    assert_eq!(
        cursor_row,
        Some(first_episode),
        "the cursor row is the Workspace's selected first episode"
    );
}
