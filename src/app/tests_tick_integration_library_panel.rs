//! Live-tick coverage for the mounted `LibraryPanel` (task 5.9). Proves, in
//! one real `Application::tick()` flow through the shell sync pass (ADR 0024,
//! design D15): framework focus follows the active library's migration state,
//! mouse eligibility follows the painted panel, painted pill clicks route
//! `SelectorPicked` to the active owner, the Wide split drag moved in from
//! `WideHeroBoundaryComponent` resolves through the panel, an inactive owner
//! keeps its cursor/scroll across a tab change, and a library leaving the
//! catalog retires its owner. No destination converts here: the migrated
//! path is exercised through a test-owned fixture content owner, the minimal
//! owner the panel can host.

use std::cell::RefCell;
use std::rc::Rc;

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tuirealm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::content::{
    HeroContent, HeroImageState, LibraryPanelContent, ListSlot, SelectorRow,
};
use crate::app::components::library_panel::owner::{LibraryContentOwner, LibrarySlotEvent};
use crate::app::components::library_panel::{
    hero_content_emby, ArtworkShape, HeroArtwork, HeroContentData, HeroFacts, LibraryKey,
};
use crate::app::components::media_list::{
    MediaKind, MediaListCarrier, MediaListRow, MediaSemanticState, Presentation, RowLocalInput,
};
use crate::app::components::msg::{Msg, TerminalObserverEvent};
use crate::app::components::{BrowserKind, ComponentId};
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
}

fn row(target: &str) -> MediaListRow<String> {
    MediaListRow::Item {
        target: target.into(),
        primary: target.into(),
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
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
                facts: HeroFacts {
                    title: "Dune".into(),
                    meta_rows: vec!["2021".into()],
                    artwork: HeroArtwork {
                        shape: ArtworkShape::Landscape,
                        source: None,
                        image: crate::app::components::library_panel::content::HeroImageState::None,
                    },
                },
                overview: None,
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
                    RowLocalInput::Click(at)
                    | RowLocalInput::DoubleClick(at)
                    | RowLocalInput::ContextClick(at) => {
                        self.carrier.resolve_current_point(at).cloned()
                    }
                    _ => None,
                };
                if let Some(target) = &target {
                    self.carrier.select_target(target);
                }
                self.carrier.delegate(input, target);
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
    LibraryKey::Service(crate::app::components::BrowserKey {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-movies".into(),
        kind: BrowserKind::Movies,
    })
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
fn library_panel_focus_follows_the_active_library() {
    let (mut harness, _log) = migrated_home();
    assert_eq!(
        harness.model().application.focus().cloned(),
        Some(ComponentId::Library),
        "the migrated tab's focus is the Library panel"
    );

    // An un-migrated Emby library keeps its old destination child focused.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    let focus = harness.model().application.focus().cloned();
    assert_ne!(focus, Some(ComponentId::Library));
    assert!(
        focus
            .as_ref()
            .is_some_and(|id| {
                matches!(
                    id,
                    ComponentId::Browser(key) if key.kind == crate::app::components::BrowserKind::Music
                )
            }),
        "an un-migrated library focuses its old destination child, got {focus:?}"
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

    // Un-migrated tab: the panel is unsubscribed and the old child is
    // subscribed instead.
    harness.model_mut().app.tab = crate::app::TabSelection::EmbyLibrary(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness
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

/// The Wide split-boundary drag moved in from `WideHeroBoundaryComponent`:
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
    assert!(
        narrow_list.right() > wide_list.right(),
        "the narrow list spans past the stale Wide list's right edge"
    );

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

    // A click inside the newly painted narrow list but outside the stale
    // Wide list rect (over the old hero pane) reaches the owner.
    let click_x = narrow_list.right() - 1;
    assert!(
        click_x > wide_list.right(),
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
            .any(|event| matches!(event, LibrarySlotEvent::List(RowLocalInput::Click(_)))),
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
#[test]
fn owner_retention_follows_the_catalog_through_the_sync_pass() {
    let mut harness = {
        let mut app = crate::app::render::make_movie_app();
        app.panel_mode = crate::app::PanelMode::LibraryOnly;
        app.panel_focus = crate::app::PanelFocus::Library;
        app.tab = crate::app::TabSelection::EmbyLibrary(0);
        let mut harness = TickHarness::new(app);
        harness.model_mut().sync_library_panel();
        harness.model_mut().push_library_owner(
            movies_key(),
            Box::new(FixtureOwner::new(Rc::new(RefCell::new(
                FixtureLog::default(),
            )))),
        );
        harness.model_mut().sync_mounted_surfaces();
        harness
    };
    assert!(
        harness.model().library_panel_has_owner(&movies_key()),
        "the in-catalog library's owner is retained"
    );

    // The library leaves the catalog: the sync pass retires its owner.
    harness.model_mut().app.libs.remove(0);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness.model().library_panel_has_owner(&movies_key()),
        "the retired library's owner is dropped by the retention rule"
    );
}

// ── Task 5.10: hero image projection (design D9) ────────────────────────
//
// The Home Hero header's fetch moved into `sync_library_hero_images`
// (`App::project_hero_image`); painting only reads the projected
// `HeroImageState`. These fixtures use a landscape-declared item so the
// projection reaches the Wide cover-fit box path.

/// A hero-bearing owner: the minimal owner the projection needs — one item's
/// policy-produced `HeroContentData`, mutable so a test can swap it for a
/// fresh item (a new cache key), and `set_hero_image` records every state the
/// projection delivers so a test can prove the placeholder frame count.
struct HeroFixtureOwner {
    carrier: MediaListCarrier<String>,
    hero: HeroContentData,
    image_states: Vec<HeroImageState>,
}

impl HeroFixtureOwner {
    fn new(hero: HeroContentData) -> Self {
        let mut carrier = MediaListCarrier::new(Presentation::Wide);
        carrier.set_content(vec![row("alpha")]);
        Self {
            carrier,
            hero,
            image_states: Vec::new(),
        }
    }
}

impl LibraryContentOwner for HeroFixtureOwner {
    fn content(&mut self) -> LibraryPanelContent<'_> {
        LibraryPanelContent {
            selector: None,
            controls: None,
            list: ListSlot::Media(&mut self.carrier),
            hero: Some(HeroContent {
                facts: self.hero.facts.clone(),
                overview: self.hero.overview.clone(),
                workspace: None,
            }),
        }
    }

    fn on_slot_event(&mut self, _event: LibrarySlotEvent) -> Option<Msg> {
        None
    }

    fn hero_data(&mut self) -> Option<HeroContentData> {
        Some(self.hero.clone())
    }

    fn set_hero_image(&mut self, state: HeroImageState) {
        self.image_states.push(state);
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// A movie item with a declared landscape (`Thumb`) image: the artwork
/// policy's Landscape arm, whose cover-fit box tracks the hero pane's width
/// directly (design D5), so a resize changes the box.
fn landscape_hero_item(id: &str) -> mbv_core::api::EmbyItem {
    let mut item = crate::app::tests::make_item("Hero", "Movie");
    item.id = id.into();
    item.image_tags.thumb = "tag".into();
    item
}

/// A Home tab with a `HeroFixtureOwner` installed instead of the plain
/// `FixtureOwner` (this file's other fixture has no declared artwork, so
/// `hero_data` returns `None` and the projection never fetches).
fn migrated_home_with_hero(item: mbv_core::api::EmbyItem, terminal_width: u16) -> TickHarness {
    let mut app = crate::app::render::make_movie_app();
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    app.panel_focus = crate::app::PanelFocus::Library;
    app.tab = crate::app::TabSelection::Home;
    app.image_protocol_enabled = true;
    app.image_picker = Some(ratatui_image::picker::Picker::halfblocks());
    app.terminal_width = terminal_width;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_library_panel();
    harness.model_mut().push_library_owner(
        home_key(),
        Box::new(HeroFixtureOwner::new(hero_content_emby(&item))),
    );
    harness
}

fn owner_of(harness: &TickHarness) -> &HeroFixtureOwner {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.owner(&home_key()))
        .and_then(|owner| owner.as_any().downcast_ref::<HeroFixtureOwner>())
        .expect("the hero fixture owner is installed")
}

/// One fetch per new hero cache key, and none on a repaint tick with the
/// same key (task 5.10's Verify clause, mirroring the queue visual slot's
/// `queue_projection_fetches_now_playing_image_once_and_none_on_repaint`,
/// task 3.4). The fixture has no Emby client, so `spawn_image_fetch`
/// balances `image_fetches_active` synchronously; the reservation set pins
/// the request count.
#[test]
fn hero_projection_fetches_image_once_and_none_on_repaint() {
    let mut harness = migrated_home_with_hero(landscape_hero_item("hero-a"), 160);
    // Establishes `root_frame.library` before the first projection reads it.
    drop(draw_frame_sized(&mut harness));
    harness.model_mut().sync_mounted_surfaces();

    let key = "hero-a:Backdrop,Primary,Logo";
    assert!(
        harness.model().app.card_image_loading.contains(key),
        "the new hero key must be reserved by the projection"
    );
    let loading = harness.model().app.card_image_loading.clone();
    let active = harness.model().app.image_fetches_active;
    let pending = harness.model().app.pending_image_fetches.len();
    let calls = harness.model().app.card_image_fetch_calls;
    assert_eq!(calls, 1, "the new hero key issues exactly one fetch call");

    // Repaint tick: nothing changed, so the projection starts no new fetch.
    // `card_image_fetch_calls` only increments past the dedup guard, so it
    // catches a broken guard even though the fixture has no Emby client
    // (`spawn_image_fetch` balances `image_fetches_active` back to its prior
    // value synchronously in that case, making the other counters blind to a
    // redundant call).
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().app.card_image_loading, loading);
    assert_eq!(harness.model().app.image_fetches_active, active);
    assert_eq!(harness.model().app.pending_image_fetches.len(), pending);
    assert_eq!(
        harness.model().app.card_image_fetch_calls,
        calls,
        "a repaint with the same hero key must not call queue_card_image_fetch again"
    );

    // A new hero item reserves exactly one new key.
    if let Some(panel) = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any_mut()
                .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
        })
    {
        if let Some(owner) = panel
            .owner_mut(&home_key())
            .and_then(|owner| owner.as_any_mut().downcast_mut::<HeroFixtureOwner>())
        {
            owner.hero = hero_content_emby(&landscape_hero_item("hero-b"));
        }
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .app
        .card_image_loading
        .contains("hero-b:Backdrop,Primary,Logo"));
}

/// A real terminal resize invalidates every card image (`sync_terminal_resize`,
/// pre-existing behaviour: font-pixel metrics can change on a real resize, so
/// the whole cache clears and every key re-fetches). For the hero key this
/// means exactly one fresh reservation at the resize's sync pass, resolved to
/// the new box size once "fetched" (simulated the same way as the initial
/// fetch) — one placeholder frame, then Ready at the new box.
#[test]
fn hero_projection_refetches_and_reencodes_on_resize_with_one_placeholder_frame() {
    let mut harness = migrated_home_with_hero(landscape_hero_item("hero-a"), 160);
    let key = "hero-a:Backdrop,Primary,Logo".to_string();

    // Establish the Ready state at the initial width.
    drop(draw_frame_sized(&mut harness));
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.card_image_loading.contains(&key));
    let img = image::DynamicImage::new_rgb8(64, 64);
    let entry = harness.model().app.build_cached_image(&key, Some(img));
    harness.model_mut().app.card_image_loading.remove(&key);
    harness
        .model_mut()
        .app
        .card_image_states
        .insert(key.clone(), entry);
    harness.model_mut().sync_mounted_surfaces();
    let box1 = harness
        .model()
        .app
        .card_image_states
        .get(&key)
        .and_then(|entry| entry.cover_box)
        .expect("the Wide header's cover-fit box is keyed on the entry");
    assert!(matches!(
        owner_of(&harness).image_states.last(),
        Some(HeroImageState::Ready { .. })
    ));

    // Resize: draw computes the fresh geometry, then the sync pass clears the
    // stale cache and reserves exactly one fresh fetch for the same key.
    harness.model_mut().app.terminal_width = 220;
    drop(draw_frame_sized(&mut harness));
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().app.card_image_states.is_empty());
    assert_eq!(
        harness.model().app.card_image_loading.len(),
        1,
        "the resize starts exactly one fresh fetch for the hero key"
    );
    assert!(harness.model().app.card_image_loading.contains(&key));
    assert_eq!(harness.model().app.image_fetches_active, 0);
    assert!(harness.model().app.pending_image_fetches.is_empty());
    assert!(
        matches!(
            owner_of(&harness).image_states.last(),
            Some(HeroImageState::Loading)
        ),
        "the resize's sync pass shows the placeholder for its one frame"
    );

    // "Fetch" resolves: the new box size's re-encode.
    let img = image::DynamicImage::new_rgb8(64, 64);
    let entry = harness.model().app.build_cached_image(&key, Some(img));
    harness.model_mut().app.card_image_loading.remove(&key);
    harness
        .model_mut()
        .app
        .card_image_states
        .insert(key.clone(), entry);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        matches!(
            owner_of(&harness).image_states.last(),
            Some(HeroImageState::Ready { .. })
        ),
        "the placeholder shows for at most the one frame the resize's sync pass painted"
    );
    let box2 = harness
        .model()
        .app
        .card_image_states
        .get(&key)
        .and_then(|entry| entry.cover_box)
        .expect("the resize re-encode keeps the box keyed on the entry");
    assert_ne!(box1, box2, "the wider panel re-encodes at a new box size");
}
