use ratatui::backend::TestBackend;
use ratatui::layout::Position;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::emby_library_content::EmbyLibraryContent as BrowserOwner;
use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest};
use crate::app::render::make_movie_app;

use crate::app::tests::{install_test_emby, make_session};
use crate::app::tests_tick_harness::TickHarness;

/// The migrated Movies/HomeVideos/Generic owner inside the mounted
/// `LibraryPanel` (task 6.1): the panel is the library area's one event
/// boundary, and the browse state is read through the owner the panel hosts
/// — never a destination component (`emby_browser_id` stays `None` for
/// these kinds).
fn browser_owner(harness: &TickHarness) -> &BrowserOwner {
    let (_, key, _) = harness
        .model().active_emby_library_owner()
        .expect("the active library's owner has migrated");
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .owner(&key)
        .and_then(|owner| owner.as_any().downcast_ref::<BrowserOwner>())
        .expect("browser owner installed")
}

fn draw(harness: &mut TickHarness, width: u16, height: u16) -> Terminal<TestBackend> {
    harness.model_mut().app.terminal_width = width;
    harness.model_mut().app.terminal_height = height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    terminal
}

#[test]
fn browser_wide_tick_moves_control_without_recomputing_app_cursor() {
    let mut app = make_movie_app();
    app.tab = crate::app::TabSelection::EmbyLibrary(0);
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let _terminal = draw(&mut harness, 100, 30);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);

    let _ = draw(&mut harness, 100, 30);
}

#[test]
fn browser_narrow_tick_click_uses_retained_geometry() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    // At 60 columns the stored panel mode is ignored and the mini view
    // derives the visible panel from `mini_view_focus`; a narrow *library*
    // view must select Library here.
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _terminal = draw(&mut harness, 60, 30);

    // The panel's own retained Narrow geometry is the painted truth: the
    // admitted inline hero block is the Inline presentation's detail rect,
    // so a click on it resolves to the retained selected target through the
    // owner's carrier — never a stale-geometry row re-resolution.
    let geometry = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<LibraryPanel>()
        .expect("Library panel type")
        .test_narrow_geometry()
        .expect("the panel painted a Narrow skeleton");
    let area = geometry
        .inline_hero
        .expect("selected detail retained geometry");
    assert!(!area.is_empty(), "selected detail retained geometry");
    let position = Position::new(area.x, area.y);
    harness.inject(Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: position.x,
        row: position.y,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryRowClick { target: Some(target) })
                if target == "movie-focused"
        )),
        "the painted inline hero block resolves to the selected row: {:?}", outcome.raw_messages
    );
    assert_eq!(browser_owner(&harness).cursor(), 0);
    assert_eq!(harness.model().app.libs[0].nav_stack[0].resting().cursor(), 0);
    let _ = draw(&mut harness, 60, 30);
}

/// The generic catalog's narrow surface routes through the shared owner. (The
/// test previously pinned the Grid presentation for this surface; Grid was
/// deleted as unreachable by design D13 — no library in use lacks a hero.)
#[test]
fn browser_generic_narrow_tick_isolated_from_canonical_controls() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "other".into();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_owner(&harness).cursor(), 0);

    // Tick navigation moves the shared owner and echoes the resolved index.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(message, Msg::Shell(ShellRequest::EmbyLibraryCursorIndex { index: 1 }))
    }));
    assert_eq!(browser_owner(&harness).cursor(), 1);

    // The next frame retains the selection in the shared owner.
    let _ = draw(&mut harness, 100, 30);
    assert_eq!(browser_owner(&harness).cursor(), 1);
}

#[test]
fn tick_play_prompt_mounts_and_accepts_local_fall_through() {
    let mut app = make_movie_app();
    app.panel_focus = crate::app::PanelFocus::Library;
    app.panel_mode = crate::app::PanelMode::LibraryOnly;
    let http = mbv_core::mock_http::MockHttp::new();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    http.respond(404, "");
    let (remote, player_rx, command_rx) =
        mbv_core::remote_player::RemotePlayer::stub_audio_only_with_command_rx(Vec::new(), 0);
    let session = make_session("audio-owner", "mbv");
    let endpoint = mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap());
    app.switch_to_direct_remote(&session, remote, player_rx, &endpoint);
    while command_rx.try_recv().is_ok() {}
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw(&mut harness, 100, 30);

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('p'),
        modifiers: KeyModifiers::CONTROL,
    }));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| {
        matches!(
            message,
            Msg::Shell(ShellRequest::EmbyLibraryPlay { item })
                if item.id == "movie-focused"
        )
    }));
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages.iter().cloned() {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();

    let confirm_id = ComponentId::Modal(crate::app::components::ModalId::Confirm);
    assert!(harness.model().application.get_component(&confirm_id).is_some());
    assert_eq!(harness.model().application.focus(), Some(&confirm_id));
    assert!(command_rx.try_recv().is_err(), "play stayed unsubmitted");

    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('y'),
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

    assert!(harness.model().application.get_component(&confirm_id).is_none());
    assert!(!harness.model().app.player.is_remote());
    assert!(harness.model().app.player.status.lock().unwrap().active);
    assert!(!harness.model().app.direct_remote_connected);
    assert!(harness.model().app.remote_player_tab.is_none());
    assert_eq!(harness.model().app.queue_scope, crate::app::QueueScope::Local);
    let local_items = harness.model().app.player_tab.emby_items();
    assert_eq!(
        local_items.first().expect("local playback queue is non-empty").id,
        "movie-focused"
    );
    assert!(command_rx.try_iter().any(|command| {
        matches!(
            command,
            mbv_core::ctrl::CtrlCmd::PlaybackIntent(intent)
                if intent.action == mbv_core::ctrl::PlaybackIntentAction::Stop
        )
    }));
}
