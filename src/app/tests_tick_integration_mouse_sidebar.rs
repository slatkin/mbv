use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};

use crate::app::action::Command;
use crate::app::components::{
    BrowserComponent, ComponentId, HelpComponent, Msg, OverlayId, PlaylistsComponent, QueueComponent,
    ShellRequest, TerminalObserverEvent, UserEvent,
};
use crate::app::tests::{make_app_stub, make_item};
use crate::app::tests_tick_harness::{StepOutcome, TickHarness};
use crate::app::PanelFocus;

fn key(code: Key) -> Event<UserEvent> {
    Event::Keyboard(KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    })
}

fn apply_outcome(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

/// A sidebar opened from the F4 path must claim a wheel at an off-panel
/// pointer; a normal key afterward proves the tick remains healthy.
#[test]
fn playlists_sidebar_claims_immediate_wheel_and_keeps_normal_keys() {
    let mut app = make_app_stub();
    app.playlists = vec![make_item("P1", "Playlist"), make_item("P2", "Playlist")];
    app.playlists_cursor = 0;
    assert!(app.playlists_open.is_none());
    assert!(app.playlists_open_items.is_empty());
    app.layout.main.panel_area = Rect::new(0, 0, 40, 20);
    let mut harness = TickHarness::new(app);
    harness.inject(key(Key::Function(4)));
    let outcome = harness.step();
    assert!(matches!(
        outcome.router,
        crate::app::router::RouterOutcome::Command(Command::OpenPlaylists)
    ));
    assert!(outcome.messages.is_empty(), "the router consumes F4's leaf message");
    harness
        .model_mut()
        .dispatch_router_command(Command::OpenPlaylists);
    apply_outcome(&mut harness, outcome);
    harness.model_mut().sync_mounted_surfaces();
    let playlists_id = ComponentId::Overlay(OverlayId::Playlists);
    assert!(harness.model().application.mounted(&playlists_id));
    assert_eq!(harness.model().application.focus(), Some(&playlists_id));
    assert_eq!(
        harness.model().mouse_subscribed,
        std::iter::once(playlists_id.clone()).collect(),
        "the real F4 lifecycle leaves only Playlists mouse-eligible"
    );
    // The focused sole overlay claims its wheel without needing a prior paint
    // or a pointer inside the sidebar.
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 59,
        row: 23,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed)
    )));
    assert_eq!(
        harness
            .model_mut()
            .application
            .get_component_mut(&playlists_id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<PlaylistsComponent>()
            .unwrap()
            .cursor(),
        1,
        "one wheel notch advances the playlist-list cursor exactly one row"
    );
    harness.inject(key(Key::Down));
    let key_messages = harness
        .model_mut()
        .application
        .tick(tuirealm::application::PollStrategy::Once(
            std::time::Duration::from_millis(500),
        ))
        .expect("normal key tick");
    assert!(key_messages
        .iter()
        .any(|message| matches!(message, Msg::TerminalEvent(TerminalObserverEvent::Key(_)))));
}

#[test]
fn tick_help_sidebar_scrolls_immediately_after_open_without_click() {
    let mut app = make_app_stub();
    app.layout.main.panel_area = Rect::new(0, 0, 30, 16);
    let mut harness = TickHarness::new(app);
    harness.model_mut().mount_help();
    harness.model_mut().sync_mounted_surfaces();

    let mut terminal = Terminal::new(TestBackend::new(40, 16)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    // Help owns wheel delivery while focused; pointer position is irrelevant.
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome
        .raw_messages
        .iter()
        .any(|msg| {
            matches!(
                msg,
                Msg::TerminalEvent(TerminalObserverEvent::MouseClaimed)
            )
        }));
    let help = harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Overlay(OverlayId::Help))
        .unwrap()
        .as_any_mut()
        .downcast_mut::<HelpComponent>()
        .unwrap();
    assert_eq!(help.test_scroll(), 1);
}

#[test]
fn tick_queue_only_wheel_excludes_unpainted_library_and_keeps_keyboard() {
    let mut app = crate::app::render::make_queue_app(8);
    app.terminal_width = 70;
    app.mini_view_focus = PanelFocus::Queue;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(70, 24)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();

    let queue_id = ComponentId::Queue;
    let library_id = harness
        .model()
        .emby_browser_id
        .clone()
        .expect("Queue-only keeps the Library destination mounted");
    assert!(
        !harness.model().mouse_subscribed.contains(&library_id),
        "an unpainted Library destination must not be mouse-eligible"
    );
    let library_cursor_before = harness
        .model_mut()
        .application
        .get_component_mut(&library_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<BrowserComponent>()
        .unwrap()
        .cursor();
    let first_row = harness
        .model_mut()
        .application
        .get_component_mut(&queue_id)
        .unwrap()
        .as_any_mut()
        .downcast_mut::<QueueComponent>()
        .unwrap()
        .selected_row_rect()
        .expect("queue painted selected row");
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: first_row.x,
        row: first_row.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert_eq!(
        harness
            .model_mut()
            .application
            .get_component_mut(&queue_id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<QueueComponent>()
            .unwrap()
            .test_cursor(),
        1
    );
    assert!(outcome
        .raw_messages
        .iter()
        .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::QueueIntent(_)))));
    assert_eq!(
        harness
            .model_mut()
            .application
            .get_component_mut(&library_id)
            .unwrap()
            .as_any_mut()
            .downcast_mut::<BrowserComponent>()
            .unwrap()
            .cursor(),
        library_cursor_before,
        "the hidden Library must not mutate from Queue-only wheel"
    );
    harness.inject(key(Key::Down));
    let raw_messages = harness
        .model_mut()
        .application
        .tick(tuirealm::application::PollStrategy::Once(
            std::time::Duration::from_millis(500),
        ))
        .unwrap();
    assert!(raw_messages
        .iter()
        .any(|msg| matches!(msg, Msg::TerminalEvent(TerminalObserverEvent::Key(_)))));
}
