use std::time::{Duration, Instant};

use tuirealm::component::AppComponent;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers};

use crate::app::components::msg::{ConfirmIntent, ServiceRequest};
use crate::app::components::{
    ComponentId, ModalId, Msg, OverlayId, QueueRequest, SearchSidebarComponent, ShellRequest,
    TerminalObserverEvent, UserEvent,
};
use crate::app::router::RouterOutcome;
use crate::app::shell::apply_router_outcome;
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::TickHarness;
use crate::app::types_confirm::{ConfirmAction, ConfirmModal};
use crate::app::types_overlay::OverlayRequest;
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

fn search_component_mut(
    harness: &mut TickHarness,
) -> &mut SearchSidebarComponent {
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
        Some(Msg::Queue(QueueRequest::Scope(crate::app::QueueScope::Local)))
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
        .tick(tuirealm::application::PollStrategy::Once(Duration::from_millis(500)))
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
    assert!(harness.model_mut().tick_search_clock(Instant::now()).is_none());

    std::thread::sleep(Duration::from_millis(310));
    let outcome = harness.step();

    assert!(outcome.raw_messages.is_empty());
    let component = search_component_mut(&mut harness);
    assert!(component.debounce_pending.is_none());
    assert!(component.debounce_deadline.is_none());
    let _ = ServiceRequest::SearchQuery;
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
        Some(Msg::Shell(ShellRequest::ConfirmIntent(ConfirmIntent::Accept)))
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
        .tick(tuirealm::application::PollStrategy::Once(Duration::from_millis(500)))
        .expect("tick lower focused queue");
    let router = harness.model_mut().router_outcome(&raw_messages);
    let messages = apply_router_outcome(raw_messages, pre_fold_focus.as_ref(), &router);
    assert_eq!(pre_fold_focus, Some(ComponentId::Queue));
    assert!(matches!(router, RouterOutcome::Swallow));
    assert!(messages.is_empty());
}
