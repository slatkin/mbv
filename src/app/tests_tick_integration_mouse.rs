use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{Event, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, ModalId, Msg, OverlayId, QueueComponent, ShellRequest};
use crate::app::render::make_music_group_app;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::{StepOutcome, TickHarness};
use crate::app::tests_tick_integration::search_component_mut;
use crate::app::state::types::confirm::{ConfirmAction, ConfirmModal};
use crate::app::state::types::context_menu::{
    ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry,
};
use crate::app::state::types::daemon_lost::DaemonLostModal;
use crate::app::state::types::overlay::OverlayRequest;
use crate::app::{PanelFocus, PanelMode, SidebarId, TabSelection};

// --- Task 5.3: blocking modals suppress mouse activity by eligibility (D2
// rung 1), not by message discarding. A mounted Search sidebar painted with
// results is the underlying surface: if the modal did not hold exclusivity,
// a click on a result row would move its cursor and a click outside its
// frame would emit `DismissSearch`.

/// A harness with a mounted Search sidebar painted with two results.
fn search_sidebar_with_painted_results() -> (TickHarness, Vec<(Rect, usize)>) {
    let app = make_app_stub();
    let mut harness = TickHarness::new(app);
    harness.model_mut().mount_sidebar(SidebarId::Search);
    {
        let component = search_component_mut(&mut harness);
        component.sidebar.query = "clip".into();
        component.sidebar.results = vec![
            make_item("Birthday Clip", "Movie"),
            make_item("Other Clip", "Series"),
        ];
        component.sidebar.list_height = 10;
    }
    let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().render_search_overlay(frame))
        .unwrap();
    let rows = search_component_mut(&mut harness)
        .test_results()
        .regions()
        .to_vec();
    assert_eq!(rows.len(), 2, "both search results must be painted");
    (harness, rows)
}

/// Clicking on the second painted result row (outside the modal) must
/// produce no message and leave the sidebar's cursor/scroll/filter
/// untouched; clicking outside the sidebar frame must not emit the
/// `DismissSearch` it would if the sidebar were still eligible.
fn assert_blocking_modal_suppresses_sidebar_clicks(
    harness: &mut TickHarness,
    rows: &[(Rect, usize)],
) {
    let (column, row) = {
        let (rect, _) = rows[1];
        (rect.x, rect.y)
    };
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| matches!(msg, Msg::TerminalEvent(_))),
        "a click outside the blocking modal must produce no underlying \
         message (only the UiRoot observer's NoOp redraw echo may appear)"
    );
    {
        let component = search_component_mut(harness);
        assert_eq!(component.sidebar.cursor, 0, "underlying cursor untouched");
        assert_eq!(component.sidebar.scroll, 0, "underlying scroll untouched");
        assert_eq!(component.sidebar.type_filter, 0);
    }

    // Outside the sidebar's painted frame: an eligible sidebar would emit
    // `DismissSearch` here (its Esc path).
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 39,
        row: 15,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| matches!(msg, Msg::TerminalEvent(_))),
        "the sidebar's dismiss click must not surface beneath a blocking modal"
    );
    assert_eq!(search_component_mut(harness).sidebar.cursor, 0);
}

#[test]
fn tick_blocking_confirm_modal_suppresses_underlying_mouse_activity() {
    let (mut harness, rows) = search_sidebar_with_painted_results();
    let modal_id = ComponentId::Modal(ModalId::Confirm);
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::Confirm(ConfirmModal {
        title: "Clear queue?".into(),
        message: "Remove queued items".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: ConfirmAction::ClearQueue,
    }));
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&modal_id));
    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(modal_id).collect(),
        "rung 1: only the blocking modal is mouse-eligible"
    );

    assert_blocking_modal_suppresses_sidebar_clicks(&mut harness, &rows);
}

#[test]
fn tick_blocking_daemon_lost_modal_suppresses_underlying_mouse_activity() {
    let (mut harness, rows) = search_sidebar_with_painted_results();
    let modal_id = ComponentId::Modal(ModalId::DaemonLost);
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::DaemonLost(DaemonLostModal {
        last_playing_title: Some("Birthday Clip".into()),
        daemon_log_path: "/tmp/mbvd.log".into(),
        restart_error: None,
    }));
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&modal_id));
    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(modal_id).collect(),
        "rung 1: only the blocking modal is mouse-eligible"
    );

    assert_blocking_modal_suppresses_sidebar_clicks(&mut harness, &rows);
}

// --- Task 2.1: tab-bar click-to-switch. The tab bar is the mounted
// `TabPanel` now: it resolves the click against its own painted hit regions
// and emits `ShellRequest::TabSelect`, which the shell drives through
// `set_library_tab` (the same entry point keyboard tab-cycling uses).

fn apply_outcome(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

fn drawn_tab_harness() -> TickHarness {
    let mut app = crate::app::render::make_movie_app();
    app.tab = TabSelection::Home;
    let mut harness = TickHarness::new(app);
    // The sync pass mounts the panels from paint-free chrome geometry; draw
    // once with them mounted, then sync + draw again with the placements
    // current -- the steady-state loop order.
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness
}

fn tab_panel_component(harness: &TickHarness) -> &crate::app::components::TabPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::TabPanel)
        .expect("TabPanel mounted when the library column is visible")
        .as_any()
        .downcast_ref::<crate::app::components::TabPanel>()
        .expect("TabPanel component")
}

fn library_panel_component(harness: &TickHarness) -> &LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("LibraryPanel mounted when the library column is visible")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("LibraryPanel component")
}

#[test]
fn tick_mouse_hover_delivery_updates_only_the_pointed_surface_in_narrow_and_wide_modes() {
    for (terminal_width, terminal_height) in [(80, 24), (120, 30)] {
        let mut app = crate::app::render::make_music_group_app();
        app.panel_mode = PanelMode::LibraryOnly;
        app.panel_focus = PanelFocus::Library;
        app.terminal_width = terminal_width;
        app.terminal_height = terminal_height;
        let mut harness = TickHarness::new(app);
        harness.model_mut().sync_mounted_surfaces();

        let mut terminal = Terminal::new(TestBackend::new(terminal_width, terminal_height)).unwrap();
        terminal
            .draw(|f| harness.model_mut().draw_frame(f, false, false))
            .unwrap();
        harness.model_mut().sync_mounted_surfaces();
        terminal
            .draw(|f| harness.model_mut().draw_frame(f, false, false))
            .unwrap();
        let tabs = tab_panel_component(&harness);
        let selected_tab = tabs.test_selected();
        let (tab, tab_position) = tabs
            .hit_regions()
            .iter()
            .find(|(_, position)| *position != selected_tab)
            .map(|(rect, position)| (*rect, *position))
            .expect("an unselected painted tab");
        let selector = library_panel_component(&harness)
            .test_selector_hits()
            .regions()
            .get(1)
            .map(|(rect, _)| *rect)
            .expect("an unselected main Selector-row pill");
        // A move over the tab is delivered through Application::tick() and
        // updates only the tab component's retained hover state.
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: tab.x,
            row: tab.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(
            outcome
                .messages
                .iter()
                .all(|message| !matches!(message, Msg::Shell(_))),
            "hover emits no shell action: {outcome:?}"
        );
        assert_eq!(
            tab_panel_component(&harness).test_hovered(),
            Some(tab_position),
            "the tab panel retains the hovered painted tab"
        );
        assert_eq!(
            library_panel_component(&harness).test_hovered_selector(),
            None,
            "hovering a tab does not hover a Selector pill"
        );

        // Move to the main Selector row through the same live path.
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: selector.x,
            row: selector.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(
            outcome
                .messages
                .iter()
                .all(|message| !matches!(message, Msg::Shell(_))),
            "hover emits no shell action: {outcome:?}"
        );
        assert_eq!(
            tab_panel_component(&harness).test_hovered(),
            None,
            "hovering a Selector pill clears the tab hover"
        );
        assert_eq!(
            library_panel_component(&harness).test_hovered_selector(),
            Some(1),
            "the Library panel retains the hovered Selector pill"
        );

        // The top-left gap is outside both retained role geometries. It
        // clears both local identities without invoking any shell action.
        harness.inject(Event::Mouse(MouseEvent {
            kind: MouseEventKind::Moved,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(
            outcome
                .messages
                .iter()
                .all(|message| !matches!(message, Msg::Shell(_))),
            "gap hover emits no shell action: {outcome:?}"
        );
        assert_eq!(tab_panel_component(&harness).test_hovered(), None);
        assert_eq!(library_panel_component(&harness).test_hovered_selector(), None);
    }
}

#[test]
fn tab_bar_click_switches_active_tab() {
    let mut harness = drawn_tab_harness();

    let (rect, tab_pos) = tab_panel_component(&harness)
        .hit_regions()
        .iter()
        .find(|(_, pos)| *pos == 1)
        .copied()
        .expect("Movies tab painted at position 1");
    assert_eq!(tab_pos, 1);

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: rect.x,
        row: rect.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    apply_outcome(&mut harness, outcome);

    assert_eq!(
        harness.model().app.tab,
        TabSelection::EmbyLibrary(0),
        "clicking the Movies tab switches to it, mirroring keyboard tab-cycling"
    );
}

#[test]
fn tab_bar_click_outside_tabs_area_is_noop() {
    let mut harness = drawn_tab_harness();

    assert!(
        !harness
            .model()
            .app
            .layout
            .root_frame
            .tab
            .expect("library column visible: tab bar placed")
            .contains(ratatui::layout::Position { x: 0, y: 0 }),
        "top-left corner must fall outside the tab bar for this assertion to be meaningful"
    );

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    apply_outcome(&mut harness, outcome);

    assert_eq!(
        harness.model().app.tab,
        TabSelection::Home,
        "a click outside the tab bar placement is a no-op"
    );
}

// --- Task 2.2: volume-pill scroll. The mounted `StatusBarPanel` resolves
// the scroll against its own painted volume pill and emits the volume
// intent; the shell dispatches it through the same path the `-`/`+` keys
// use.
#[test]
fn tick_scroll_on_the_volume_pill_emits_the_volume_intent() {
    let mut app = crate::app::render::make_movie_app();
    app.ui_volume = 60;
    app.mute_on = false;
    let mut harness = TickHarness::new(app);
    // The panels mount in the first sync pass from paint-free chrome
    // geometry; draw with them mounted, then sync + draw again -- the
    // steady-state loop order.
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();

    let vol = tab_panel_status_regions(&harness)
        .volume
        .expect("volume pill painted and region retained");

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: vol.x + 1,
        row: vol.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.messages.contains(&Msg::Playback(
            crate::app::components::PlaybackRequest::VolumeDelta(-5)
        )),
        "the volume intent reaches the shell: {:?}",
        outcome.messages
    );
    apply_outcome(&mut harness, outcome);
    assert_eq!(
        harness.model().app.ui_volume,
        55,
        "scroll down lowers the volume by the legacy wheel step"
    );
}

fn tab_panel_status_regions(harness: &TickHarness) -> crate::app::render::StatusBarRegions {
    harness
        .model()
        .application
        .get_component(&ComponentId::StatusBarPanel)
        .expect("StatusBarPanel mounted when the library column is visible")
        .as_any()
        .downcast_ref::<crate::app::components::StatusBarPanel>()
        .expect("StatusBarPanel component")
        .regions()
}

/// D1's mount rule through the live sync pass (tasks 2.1-2.2): the chrome
/// panels mount exactly when the root places them. Queue-only places neither
/// tab bar nor status row, so both are unmounted there; a library-visible
/// mode mounts both.
#[test]
fn tick_chrome_panels_mount_only_where_the_root_places_them() {
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::QueueOnly;
    app.terminal_width = 120; // >= MINI_VIEW_THRESHOLD, so the mode applies
    let mut harness = TickHarness::new(app);
    // One draw publishes the queue-only placements; the sync it gates must
    // keep both chrome panels unmounted.
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().application.mounted(&ComponentId::TabPanel));
    assert!(!harness
        .model()
        .application
        .mounted(&ComponentId::StatusBarPanel));

    // The library-visible counterpart mounts both.
    let harness = drawn_tab_harness();
    assert!(harness.model().application.mounted(&ComponentId::TabPanel));
    assert!(harness
        .model()
        .application
        .mounted(&ComponentId::StatusBarPanel));
}

/// Review of tasks 2.1-2.2: the sync pass decides the chrome-panel mounts
/// from paint-free chrome geometry, not from the draw-time-published
/// `root_frame` — so a placement that just appears (crossing the mini-view
/// threshold upward, which flips `effective_panel_mode` out of mini view)
/// mounts the panels before the first draw that shows it, with no
/// intervening frame missing them.
#[test]
fn tick_chrome_panels_mount_in_the_sync_pass_when_a_placement_appears() {
    // Start below the mini-view threshold: `effective_panel_mode` follows
    // `mini_view_focus` (Queue), so QueueOnly places no chrome panels.
    let mut app = make_app_stub();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 60;
    app.terminal_height = 24;
    let mut harness = TickHarness::new(app);
    let mut terminal = Terminal::new(TestBackend::new(60, 24)).unwrap();
    // A mini-view draw settles the narrow placements (which place no chrome
    // panels); the following sync keeps both panels unmounted.
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().application.mounted(&ComponentId::TabPanel));
    assert!(!harness
        .model()
        .application
        .mounted(&ComponentId::StatusBarPanel));

    // Cross the threshold upward without drawing: the very next sync must
    // see the wide placements and mount both panels, before any draw
    // publishes `root_frame`.
    harness.model_mut().app.terminal_width = 120;
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        harness.model().application.mounted(&ComponentId::TabPanel),
        "the appearing tab placement must mount the panel in the sync pass, before any draw"
    );
    assert!(
        harness
            .model()
            .application
            .mounted(&ComponentId::StatusBarPanel),
        "the appearing status-bar placement must mount the panel in the sync pass, before any draw"
    );
}

/// Task 5.4 (D2 rung 2 exclusivity): with the context menu mounted, a wheel
/// over the obscured queue must not reach it. The same wheel reaches the
/// queue and scrolls it while the queue is eligible.
#[test]
fn tick_context_menu_wheel_does_not_mutate_the_obscured_queue() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    // The queue panel retains its own geometry from its paint (task 3.1); the
    // wheel eligibility below follows the painted surface, not a seeded
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let wheel = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };
    harness.inject(wheel(5, 5));
    let outcome = harness.step();
    assert!(
        !outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::Shell(ShellRequest::QueueIntent(_)))),
        "the queue handles an eligible wheel locally without a shell relay"
    );

    let menu_id = ComponentId::Overlay(OverlayId::ContextMenu);
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::ContextMenu(ContextMenu {
        anchor: ContextMenuAnchor::SelectedItem(PanelFocus::Queue),
        entries: vec![ContextMenuEntry {
            label: "Play",
            action: Some(ContextAction::Play),
        }],
        cursor: 0,
    }));
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&menu_id));
    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(menu_id).collect(),
        "only the context menu is mouse-eligible while it is mounted"
    );

    harness.inject(wheel(5, 5));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| matches!(msg, Msg::TerminalEvent(_))),
        "the obscured queue must not receive the wheel once the menu is up"
    );
}

// --- Task 7.1: with Queue and the Library destination both visible and no
// overlay mounted, a click on each resolves through the real `tick()` sync
// order to that surface's own message (D2 exclusivity holds with two
// simultaneously eligible surfaces, not just one), and focus follows the
// click via the same `sync_mounted_surfaces` pass a real frame uses.

#[test]
fn simultaneous_queue_and_library_clicks_resolve_to_the_painting_component() {
    let mut app = crate::app::render::make_queue_app(2);
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    // Task 6.1: the library surface is the mounted `LibraryPanel` (the
    // Movies owner is embedded inside it), so both eligible surfaces are
    // Queue and the panel.
    let library_child = ComponentId::Library;
    let eligible = &harness.model().mouse_subscribed;
    assert!(
        eligible.contains(&ComponentId::Queue) && eligible.contains(&library_child),
        "Queue and the Library destination are simultaneously mouse-eligible with no overlay up: {eligible:?}"
    );
    assert_eq!(harness.model().application.focus(), Some(&library_child));

    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();

    let queue_rect = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Queue)
        .expect("queue mounted")
        .as_any_mut()
        .downcast_mut::<QueueComponent>()
        .expect("queue component type")
        .selected_row_rect()
        .expect("queue painted selected row");

    let library_point = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_list_rect())
        .expect("the Library panel must have painted a list slot");
    assert!(
        library_point.width > 0 && library_point.height > 0,
        "the Library destination must have painted a non-empty list area"
    );

    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };

    // A click inside Queue's painted rows resolves to a Queue-specific
    // message, not a Library one, even though Library currently holds focus.
    harness.inject(click(queue_rect.x, queue_rect.y));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::Shell(ShellRequest::QueueRowClick { .. }))),
        "a click on Queue's painted row must resolve through Queue"
    );
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::EmbyLibraryRowClick { .. }))),
        "the click on Queue must not also resolve through Library"
    );
    apply_outcome(&mut harness, outcome);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue),
        "focus follows the click onto Queue"
    );

    // A click inside Library's painted list resolves through the Library
    // panel's slot-event path — never through Queue — and a blank area of
    // the list claims nothing.
    harness.inject(click(
        library_point.x,
        library_point.bottom().saturating_sub(1),
    ));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::EmbyLibraryRowClick { .. }))),
        "a blank Library click must not claim without a resolved target"
    );
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::QueueRowClick { .. }))),
        "the click on Library must not also resolve through Queue"
    );
    apply_outcome(&mut harness, outcome);
    harness.model_mut().sync_mounted_surfaces();
}

// --- task 1.4: the boundary is a two-panel-layout component. RootFrame places
// it (and the sync pass mounts it) only in the Both layout; queue-only and
// library-only find it unmounted, and returning to Both remounts it.

#[path = "tests_tick_integration_mouse_panels.rs"]
mod tests_tick_integration_mouse_panels;
