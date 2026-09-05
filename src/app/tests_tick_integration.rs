use std::time::{Duration, Instant};

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::component::AppComponent;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::msg::{ConfirmIntent, PlaybackRequest, ServiceRequest};
use crate::app::components::{
    BrowserComponent, ComponentId, ModalId, Msg, OverlayId, QueueComponent, QueueRequest,
    SearchSidebarComponent, ShellRequest, TerminalObserverEvent, UserEvent,
};
use crate::app::router::RouterOutcome;
use crate::app::shell::apply_router_outcome;
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::{StepOutcome, TickHarness};
use crate::app::types_confirm::{ConfirmAction, ConfirmModal};
use crate::app::types_context_menu::{
    ContextAction, ContextMenu, ContextMenuAnchor, ContextMenuEntry,
};
use crate::app::types_daemon_lost::DaemonLostModal;
use crate::app::types_overlay::OverlayRequest;
use crate::app::types_playback::RemoteReanchorPopup;
use crate::app::{PanelFocus, PanelMode, SidebarId, TabSelection};

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn queue_focused_harness() -> TickHarness {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    TickHarness::new(app)
}

fn search_component_mut(harness: &mut TickHarness) -> &mut SearchSidebarComponent {
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Overlay(OverlayId::Search))
        .expect("search sidebar mounted")
        .as_any_mut()
        .downcast_mut::<SearchSidebarComponent>()
        .expect("search sidebar type")
}

fn arm_search_query(harness: &mut TickHarness, query: &str) {
    for c in query.chars() {
        let message = search_component_mut(harness).on(&key(Key::Char(c)));
        assert!(message.is_none(), "typing search chars stays local");
    }
}

/// Phase 1 delivery proof (task 2.7): with Queue focused, a click on the
/// seek-bar row still reaches the unfocused `PlaybackComponent` through its
/// `mouse_sub()` subscription, and the component resolves the column against
/// its own painted `seekbar_area` into a 0.0..=1.0 fraction. No other eligible
/// surface claims the event (D2 exclusivity).
#[test]
fn tick_delivers_seekbar_click_to_unfocused_playback_as_a_fraction() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.connected_session_id = Some("session-1".into());
    app.layout.playback.player_area = Rect::new(10, 5, 40, 4);
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(60, 12)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().render_playback_component(frame))
        .unwrap();

    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 30,
        row: 5,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();

    assert_eq!(outcome.pre_fold_focus, Some(ComponentId::Queue));
    let seeks: Vec<f64> = outcome
        .raw_messages
        .iter()
        .filter_map(|msg| match msg {
            Msg::Playback(PlaybackRequest::SeekTo(f)) => Some(*f),
            _ => None,
        })
        .collect();
    assert_eq!(seeks.len(), 1, "exactly one surface claims the click");
    assert!((seeks[0] - 0.5).abs() < 1e-6, "column 30 of x10..w40 is 0.5");
}

#[test]
fn tick_delivers_key_to_focused_queue_before_root_observer_once() {
    let mut harness = queue_focused_harness();
    harness.inject(key(Key::Char('[')));

    let outcome = harness.step();

    assert_eq!(outcome.pre_fold_focus, Some(ComponentId::Queue));
    assert!(matches!(outcome.router, RouterOutcome::FallThrough));
    assert_eq!(outcome.raw_messages.len(), 2, "one leaf and one observer");
    assert!(matches!(
        outcome.raw_messages.first(),
        Some(Msg::Queue(QueueRequest::Scope(
            crate::app::QueueScope::Local
        )))
    ));
    assert!(matches!(
        outcome.raw_messages.get(1),
        Some(Msg::TerminalEvent(TerminalObserverEvent::Key(_)))
    ));
    assert_eq!(
        outcome
            .raw_messages
            .iter()
            .filter(|msg| matches!(msg, Msg::Queue(QueueRequest::Scope(_))))
            .count(),
        1
    );
    assert_eq!(
        outcome
            .raw_messages
            .iter()
            .filter(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::Key(_))))
            .count(),
        1
    );
    assert_eq!(outcome.messages.len(), 1, "observer key is fold-only");

    harness.inject(key(Key::Char('[')));
    let next = harness.step();
    assert_eq!(next.raw_messages.len(), 2);
    assert_eq!(next.messages.len(), 1);
}

#[test]
fn full_sync_sequence_leaves_focus_on_queue_or_library_destination() {
    let mut queue_harness = queue_focused_harness();
    queue_harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        queue_harness.model().application.focus(),
        Some(&ComponentId::Queue)
    );

    let mut library_app = crate::app::render::make_movie_app();
    library_app.tab = TabSelection::EmbyLibrary(0);
    library_app.panel_focus = PanelFocus::Library;
    library_app.panel_mode = PanelMode::Both;
    let mut library_harness = TickHarness::new(library_app);
    library_harness.model_mut().sync_mounted_surfaces();
    let child = library_harness
        .model()
        .emby_browser_id
        .clone()
        .expect("movie browser child mounted");
    assert_eq!(library_harness.model().application.focus(), Some(&child));

    let mut stub_app = make_app_stub();
    stub_app.tab = TabSelection::EmbyLibrary(0);
    stub_app.panel_focus = PanelFocus::Library;
    let mut stub_harness = TickHarness::new(stub_app);
    stub_harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        stub_harness.model().application.focus(),
        Some(&ComponentId::UiRoot)
    );
}

#[test]
fn search_clock_user_event_reaches_mounted_search_component() {
    let mut harness = TickHarness::new(make_app_stub());
    harness.model_mut().mount_sidebar(SidebarId::Search);
    arm_search_query(&mut harness, "ab");
    std::thread::sleep(Duration::from_millis(310));

    harness.inject(Event::User(UserEvent::Clock(Instant::now())));
    let raw_messages = harness
        .model_mut()
        .application
        .tick(tuirealm::application::PollStrategy::Once(
            Duration::from_millis(500),
        ))
        .expect("tick user clock");

    assert!(raw_messages.iter().any(|msg| {
        matches!(
            msg,
            Msg::Service(ServiceRequest::SearchQuery(query)) if query == "ab"
        )
    }));
    let component = search_component_mut(&mut harness);
    assert!(component.debounce_pending.is_none());
    assert!(component.debounce_deadline.is_none());
}

#[test]
fn search_clock_sweep_dispatches_debounce_on_step() {
    let mut harness = TickHarness::new(make_app_stub());
    harness.model_mut().mount_sidebar(SidebarId::Search);
    arm_search_query(&mut harness, "ab");
    assert!(harness
        .model_mut()
        .tick_search_clock(Instant::now())
        .is_none());

    std::thread::sleep(Duration::from_millis(310));
    let outcome = harness.step();

    assert!(outcome.raw_messages.is_empty());
    let component = search_component_mut(&mut harness);
    assert!(component.debounce_pending.is_none());
    assert!(component.debounce_deadline.is_none());
    let _ = ServiceRequest::SearchQuery;
}

/// Mini view keeps `effective_panel_focus` on Queue, so `sync_queue` used to
/// re-activate Queue on the tick after a sidebar mounted, stealing the Esc
/// that would close it. The sync passes must yield focus while an overlay is
/// up.
#[test]
fn esc_closes_a_sidebar_in_mini_view() {
    let mut app = make_app_stub();
    app.terminal_width = 70;
    let mut harness = TickHarness::new(app);
    harness.model_mut().mount_sidebar(SidebarId::Sessions);
    let id = ComponentId::Overlay(OverlayId::Sessions);

    // The sync pass that previously stole focus back to Queue.
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().application.focus(), Some(&id));

    harness.inject(key(Key::Esc));
    let outcome = harness.step();
    let focused = outcome.pre_fold_focus.clone();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness.model_mut().handle_terminal_message(
            message,
            focused.as_ref(),
            &mut music_resize,
            &mut tv_resize,
        );
    }
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().application.mounted(&id));
}

#[test]
fn blocking_confirm_overlay_keeps_focus_and_receives_input() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.pending_overlay = Some(OverlayRequest::Confirm(ConfirmModal {
        title: "Clear queue?".into(),
        message: "Remove queued items".into(),
        hint: "[y] Confirm    [Esc] Cancel".into(),
        on_confirm: ConfirmAction::ClearQueue,
    }));
    let mut harness = TickHarness::new(app);

    harness.model_mut().sync_mounted_surfaces();
    let confirm_id = ComponentId::Modal(ModalId::Confirm);
    assert_eq!(harness.model().application.focus(), Some(&confirm_id));

    harness.inject(key(Key::Char('y')));
    let outcome = harness.step();
    assert_eq!(outcome.pre_fold_focus, Some(confirm_id.clone()));
    assert!(matches!(outcome.router, RouterOutcome::FallThrough));
    assert!(matches!(
        outcome.raw_messages.first(),
        Some(Msg::Shell(ShellRequest::ConfirmIntent(
            ConfirmIntent::Accept
        )))
    ));

    harness
        .model_mut()
        .application
        .active(&ComponentId::Queue)
        .expect("activate lower queue for swallow guard");
    harness.inject(key(Key::Char('c')));
    let pre_fold_focus = harness.model().application.focus().cloned();
    let raw_messages = harness
        .model_mut()
        .application
        .tick(tuirealm::application::PollStrategy::Once(
            Duration::from_millis(500),
        ))
        .expect("tick lower focused queue");
    let router = harness.model_mut().router_outcome(&raw_messages);
    let messages = apply_router_outcome(raw_messages, pre_fold_focus.as_ref(), &router);
    assert_eq!(pre_fold_focus, Some(ComponentId::Queue));
    assert!(matches!(router, RouterOutcome::Swallow));
    assert!(messages.is_empty());
}

// --- Task 5.3: blocking modals suppress mouse activity by eligibility (D2
// rung 1), not by message discarding. A mounted Search sidebar painted with
// results is the underlying surface: if the modal did not hold exclusivity,
// a click on a result row would move its cursor and a click outside its
// frame would emit `DismissSearch`.

/// A harness with a mounted Search sidebar painted with two results.
fn search_sidebar_with_painted_results() -> (TickHarness, Vec<(Rect, usize)>) {
    let mut app = make_app_stub();
    app.layout.main.panel_area = Rect::new(0, 0, 30, 16);
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
    harness.model_mut().app.pending_overlay =
        Some(OverlayRequest::DaemonLost(DaemonLostModal {
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

#[test]
fn tick_blocking_remote_reanchor_modal_suppresses_underlying_mouse_activity() {
    let (mut harness, rows) = search_sidebar_with_painted_results();
    let modal_id = ComponentId::Modal(ModalId::RemoteReanchor);
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::RemoteReanchor(
        RemoteReanchorPopup {
            targets: vec![(0, "Local".into())],
            cursor: 0,
        },
    ));
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().application.mounted(&modal_id));
    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(modal_id).collect(),
        "rung 1: only the blocking modal is mouse-eligible"
    );

    assert_blocking_modal_suppresses_sidebar_clicks(&mut harness, &rows);
}

// --- Task 6.5: tab-bar click-to-switch. The tab bar is shell-painted chrome
// with no mounted component, so it has no `mouse_sub()` claim; the click is
// resolved by the shell against `layout.tabs_hitmap` via the `MouseClick`
// observer signal, then driven through `set_library_tab` (the same entry
// point keyboard tab-cycling uses).

fn apply_outcome(harness: &mut TickHarness, outcome: StepOutcome) {
    let focused = outcome.pre_fold_focus.clone();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness.model_mut().handle_terminal_message(
            message,
            focused.as_ref(),
            &mut music_resize,
            &mut tv_resize,
        );
    }
}

#[test]
fn tab_bar_click_switches_active_tab() {
    let mut app = crate::app::render::make_movie_app();
    app.tab = TabSelection::Home;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();

    let (rect, tab_pos) = harness
        .model()
        .app
        .layout
        .main
        .tabs_hitmap
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
    let mut app = crate::app::render::make_movie_app();
    app.tab = TabSelection::Home;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();

    assert!(
        !harness.model().app.layout.tabs_area.contains(
            ratatui::layout::Position { x: 0, y: 0 }
        ),
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
        "a click outside tabs_area is a no-op"
    );
}

/// Task 5.4 (D2 rung 2 exclusivity): with the context menu mounted, a wheel
/// over the obscured queue must not reach it. The same wheel reaches the
/// queue and scrolls it while the queue is eligible.
#[test]
fn tick_context_menu_wheel_does_not_mutate_the_obscured_queue() {
    let mut app = make_app_stub();
    app.panel_focus = PanelFocus::Queue;
    app.layout.main.queue_area = Rect::new(0, 0, 40, 10);
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
        outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::Shell(ShellRequest::QueueScroll { .. }))),
        "the queue responds to the wheel while it is mouse-eligible"
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

    let library_child = harness
        .model()
        .emby_browser_id
        .clone()
        .expect("movie browser child mounted");
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

    let (queue_rect, _) = *harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Queue)
        .expect("queue mounted")
        .as_any_mut()
        .downcast_mut::<QueueComponent>()
        .expect("queue component type")
        .test_rows()
        .first()
        .expect("queue painted at least one row");

    let library_point = harness
        .model_mut()
        .application
        .get_component_mut(&library_child)
        .expect("library child mounted")
        .as_any_mut()
        .downcast_mut::<BrowserComponent>()
        .expect("browser component type")
        .test_layout()
        .left_area;
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
            .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::BrowserRowClick { .. }))),
        "the click on Queue must not also resolve through Library"
    );
    apply_outcome(&mut harness, outcome);
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(
        harness.model().application.focus(),
        Some(&ComponentId::Queue),
        "focus follows the click onto Queue"
    );

    // A click inside Library's painted list resolves to a Library-specific
    // message and focus follows back onto the Library destination.
    harness.inject(click(library_point.x, library_point.y));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::Shell(ShellRequest::BrowserRowClick { .. }))),
        "a click on Library's painted list must resolve through the Library destination"
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
    assert_eq!(
        harness.model().application.focus(),
        Some(&library_child),
        "focus follows the click back onto the Library destination"
    );
}
