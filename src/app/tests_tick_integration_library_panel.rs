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
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
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
    log: Rc<RefCell<FixtureLog>>,
}

impl FixtureOwner {
    fn new(log: Rc<RefCell<FixtureLog>>) -> Self {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![row("alpha"), row("beta"), row("gamma")]);
        Self { carrier, log }
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
                workspace: None,
            }),
        }
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
                self.carrier.delegate_operation(input.into_operation(target).expect("resolved media-list pointer target"));
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

fn find_text(buf: &ratatui::buffer::Buffer, needle: &str) -> Option<(u16, u16)> {
    for row in 0..30 {
        let mut line = String::new();
        for x in 0..120 {
            line.push_str(buf[(x, row)].symbol());
        }
        if let Some(offset) = line.find(needle) {
            return Some((offset as u16, row));
        }
    }
    None
}

/// Focus follows the active library through the real sync pass: the migrated
/// tab routes to `ComponentId::Library`, an un-migrated tab routes to its old
/// destination child, and back again. Task 6.1 migrated Movies, so the
/// un-migrated fixture is the grouped Music library (task 8 still mounts its
/// workspace).
#[test]
fn library_overlay_dismissal_routes_through_tick_without_queue_click_through() {
    let (mut harness, _log) = migrated_home();
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
    let _ = draw_frame(&mut harness);
    let panel = panel_of(&harness).expect("Library panel painted");
    let (pane, frame) = panel.test_overlay_geometry().expect("overlay painted");
    let (x, y) = (pane.y..pane.bottom())
        .flat_map(|y| (pane.x..pane.right()).map(move |x| (x, y)))
        .find(|&(x, y)| !frame.contains(ratatui::layout::Position::new(x, y)))
        .expect("dimmed Library remainder");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed))
    }));
    assert!(panel_of(&harness)
        .and_then(|panel| panel.test_overlay_geometry())
        .is_none());
    assert!(harness.model().application.mounted(&ComponentId::Queue));
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

/// A library leaving the catalog retires its owner: the catalog-retention
/// rule (moved inside from `reconcile_destination_mounts`) runs in the sync
/// pass.
#[path = "tests_tick_integration_library_panel_hero.rs"]
mod tests_tick_integration_library_panel_hero;
