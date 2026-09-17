//! Queue "Go to Library" regression coverage for change
//! `per-destination-item-navigation` (row 5.2 manual-check failure): an
//! Episode navigated from the queue must either land on its owning Series
//! (workspace hand-off included) or take the 4.2 error-flash path — never
//! die silently. Mocked Emby boundary only (`MockHttp`), per the AGENTS.md
//! mocks-only policy.

use ratatui::backend::TestBackend;
use ratatui::Terminal;
use std::time::Duration;

use crate::app::components::tv_content::TvContent;
use crate::app::components::ContextMenuComponent;
use crate::app::components::{ComponentId, OverlayId};
use crate::app::notify_actions::ToastSeverity;
use crate::app::tests::{install_test_emby, make_app_stub, make_item};
use crate::app::tests_tick_harness::TickHarness;
use crate::app::{LibraryTab, LibEvent, PanelFocus, PanelMode, TabSelection};
use mbv_core::mock_http::MockHttp;

/// App stub with a scripted in-memory Emby transport installed (same shape
/// as `tests_library_navigate_reveal`'s helper).
fn app_with_mock_emby(http: &MockHttp) -> crate::app::App {
    let mut app = make_app_stub();
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
    app
}

/// A saved root position for the TV library: the root series list, cursor
/// resting on the first show.
fn tv_saved_position() -> crate::config::LibraryPosition {
    crate::config::LibraryPosition {
        levels: vec![crate::config::LibraryPositionLevel {
            parent_id: "lib-tv".into(),
            title: "TV".into(),
            focused_item_id: Some("ser0".into()),
            cursor_index: 0,
            item_types: Some("Series".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            library_total: Some(2),
        }],
        ..Default::default()
    }
}

fn tv(harness: &TickHarness) -> &TvContent {
    harness.model().test_tv_owner()
}

fn menu_entry_index(harness: &TickHarness, label: &str) -> usize {
    let menu_id = ComponentId::Overlay(OverlayId::ContextMenu);
    let menu = harness
        .model()
        .application
        .get_component(&menu_id)
        .and_then(|component| component.as_any().downcast_ref::<ContextMenuComponent>())
        .expect("context menu mounted");
    menu.entries()
        .iter()
        .position(|entry| entry.label == label)
        .unwrap_or_else(|| panic!("{label} entry present"))
}

/// The resolve worker must never die silently when Emby is unavailable: the
/// queue "Go to Library" action takes the 4.2 path (error flash, tab
/// unchanged) instead of a bare return.
#[test]
fn go_to_library_without_an_emby_client_flashes_and_keeps_the_tab() {
    let _guard = crate::config::TestStateDirGuard::new();
    // `make_app_stub` leaves `emby_runtime` default: no client.
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;

    app.spawn_navigate_to_item(
        "ep1".into(),
        "Episode".into(),
        vec![(0, "lib-tv".into(), "tvshows".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("error event: the unavailable client must not be a silent drop");
    assert!(matches!(ev, LibEvent::Error(_)));
    app.handle_lib_event(ev);
    assert!(
        app.status.contains("Library error"),
        "the 4.2 flash fires: {}",
        app.status
    );
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
}

/// Row 5.2 manual-check regression: queue right-click "Go to Library" on an
/// Episode of a TV library that was never loaded this session but HAS a
/// saved position. The landing arms the pending Series landing and rides the
/// saved-position restore drain; that restore must serve the pending landing
/// even though the TV tab is not yet active (the queue tab is) — the tab
/// switches only after the landing completes. Landed state: the show
/// selected in the series list and the Wide workspace open, through one real
/// `Application::tick()` sync pass.
#[test]
fn queue_go_to_library_on_an_episode_lands_through_a_restored_saved_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);

    // The target TV library: empty nav_stack (never loaded this session),
    // saved root position, mock transport.
    let mut library = make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    app.replace_saved_library_position(0, tv_saved_position());
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Queue;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 50;

    // A queued episode: the queue context menu's "Go to Library" target.
    let mut episode = make_item("Pilot", "Episode");
    episode.id = "ep1".into();
    episode.series_id = "ser1".into();
    app.replace_playback_queue(vec![episode], 0);

    // Scripted responses in call order: the episode fetch, the owning-series
    // fetch, the saved-position restore of the root series list, and the
    // landed show's detail fetch.
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep1","Name":"Pilot","Type":"Episode","SeriesId":"ser1"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser1","Name":"The Show","Type":"Series"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser0","Name":"Other Show","Type":"Series"},{"Id":"ser1","Name":"The Show","Type":"Series"}],"TotalRecordCount":2}"#,
    );
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);

    let mut harness = TickHarness::new(app);

    // The real queue path: build the context menu over the queued episode and
    // select its "Go to Library" entry through the shell's select handler.
    harness.model_mut().app.open_context_menu(false, None);
    harness.model_mut().sync_mounted_surfaces();
    let go_to_library = menu_entry_index(&harness, "Go to Library");
    harness.model_mut().handle_context_menu_select(go_to_library);

    // The resolve worker names the owning Series; the landing arms the
    // pending landing (the library was never loaded) instead of flashing.
    let ev = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    assert!(
        matches!(ev, LibEvent::NavigateTo { .. }),
        "expected a NavigateTo landing"
    );
    harness.model_mut().handle_inline_search_lib_event(ev);
    assert!(
        harness.model().app.pending_series_landing.is_some(),
        "an unloaded library with a saved position arms the pending landing"
    );
    assert_eq!(
        harness.model().app.tab,
        TabSelection::Home,
        "no tab yank before the landing completes"
    );

    // The saved-position restore drains and must serve the pending landing
    // even though the TV tab is not the active one yet.
    let ev = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("restore event");
    assert!(
        matches!(ev, LibEvent::RestoreLibraryPosition { .. }),
        "expected a RestoreLibraryPosition drain"
    );
    harness.model_mut().handle_inline_search_lib_event(ev);

    assert!(
        harness.model().app.pending_series_landing.is_none(),
        "the restore drain retries the pending landing to a landing"
    );
    assert_eq!(
        harness.model().app.tab,
        TabSelection::EmbyLibrary(0),
        "landed and switched to the TV library"
    );

    // One real sync pass runs the Series detail hand-off: workspace opens,
    // episode selection focused.
    let mut terminal = Terminal::new(TestBackend::new(160, 50)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.step();

    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("ser1".into()),
        "the TV owner re-anchored onto the navigated show"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the Wide workspace is open with episode selection focused"
    );
    assert_ne!(
        harness.model().app.status_severity,
        ToastSeverity::Error,
        "a successful landing does not flash an error: {}",
        harness.model().app.status
    );
}

