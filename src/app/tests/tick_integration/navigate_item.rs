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
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::tests::tick_integration::harness::TickHarness;
use crate::app::tests::{install_test_emby, make_app_stub, make_item};
use crate::app::{BrowseLevel, LibEvent, LibraryTab, PanelFocus, PanelMode, TabSelection};
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
            fetched_rows: None,
            parent_id: "lib-tv".into(),
            title: "TV".into(),
            focused_item_id: Some("ser0".into()),
            cursor_index: 0,
            item_types: Some("Series".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            letter_filter_index: None,
            tv_content_mode: None,
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
    harness
        .model_mut()
        .handle_context_menu_select(go_to_library);

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

/// A Wide TV harness whose library holds two shows and whose `ser1` detail
/// is pre-seeded with two seasons. `episodes_for_season_2` names the cached
/// season-2 episode ids; passing an empty season and scripting the MockHttp
/// exercises the uncached-fetch path. `cached_season_episodes: false` drops
/// every cached episode map so only a fetch can satisfy the selection.
fn deep_selection_tv_harness(http: &MockHttp, episodes_for_season_2: &[&str]) -> TickHarness {
    let mut app = app_with_mock_emby(http);
    let mut library = make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    let mut ser0 = make_item("Other Show", "Series");
    ser0.id = "ser0".into();
    let mut ser1 = make_item("The Show", "Series");
    ser1.id = "ser1".into();
    // The resting cursor starts on ser1 (index 1): its detail is pre-seeded,
    // so the fixture makes no HTTP requests and each test scripts exactly the
    // fetches its own scenario needs.
    app.libs[0].nav_stack.push(BrowseLevel {
        fetched_rows: 0,
        parent_id: "lib-tv".into(),
        title: "TV".into(),
        items: vec![ser0, ser1.clone()],
        total_count: 2,
        resting: crate::app::state::types::browse::BrowseResting::new(1, 0),
        item_types: Some("Series".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    let mut season1 = make_item("Season 1", "Season");
    season1.id = "season-1".into();
    let mut season2 = make_item("Season 2", "Season");
    season2.id = "season-2".into();
    let mut ep1 = make_item("Pilot", "Episode");
    ep1.id = "ep-1".into();
    let mut season_2_episodes: Vec<mbv_core::api::EmbyItem> = Vec::new();
    for (index, id) in episodes_for_season_2.iter().enumerate() {
        let mut ep = make_item(&format!("Episode {index}"), "Episode");
        ep.id = (*id).into();
        season_2_episodes.push(ep);
    }
    app.series_detail_cache.insert(
        "ser1".into(),
        crate::app::SeriesDetail {
            seasons: vec![season1, season2],
            episodes: [
                ("season-1".into(), vec![ep1]),
                ("season-2".into(), season_2_episodes),
            ]
            .into_iter()
            .collect(),
        },
    );
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::Both;
    app.terminal_width = 160;
    app.terminal_height = 50;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

fn series_reveal() -> Box<mbv_core::api::EmbyItem> {
    let mut ser1 = make_item("The Show", "Series");
    ser1.id = "ser1".into();
    Box::new(ser1)
}

fn episode_navigate(episode_id: Option<String>) -> LibEvent {
    LibEvent::NavigateTo {
        lib_idx: 0,
        landing: crate::app::state::types::events::NavigateLanding::Series {
            reveal: series_reveal(),
            episode_id,
        },
        switch_tab: true,
    }
}

/// Drains lib events until one matches `what`'s season, panicking on timeout.
fn recv_season_episodes(harness: &mut TickHarness, season_id: &str) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "timed out waiting for season {season_id} episodes"
        );
        match harness
            .model()
            .app
            .lib_rx
            .recv_timeout(Duration::from_millis(200))
        {
            Ok(ev) => {
                let hit = matches!(
                    &ev,
                    LibEvent::SeriesSeasonEpisodesFetched { season_id: sid, .. } if sid == season_id
                );
                harness.model_mut().handle_inline_search_lib_event(ev);
                if hit {
                    return;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(e @ std::sync::mpsc::RecvTimeoutError::Disconnected) => panic!("{e}"),
        }
    }
}

/// Task 6.1 (Wide): "Go to Library" on a queued Episode lands on the show
/// with its Wide workspace open AND the episode selected with episode focus —
/// including the season move when the episode sits in a later season.
#[test]
fn navigated_workspace_selects_the_episode_in_its_season() {
    let mut harness = deep_selection_tv_harness(&MockHttp::new(), &["ep-2a", "ep-2b"]);
    // The queued episode lives in Season 2; the workspace default rests on
    // Season 1.
    harness
        .model_mut()
        .handle_inline_search_lib_event(episode_navigate(Some("ep-2b".into())));
    harness.step();

    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("ser1".into()),
        "the show is selected in the series list"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the Wide workspace is open with episode selection focused"
    );
    assert_eq!(
        tv(&harness).selected_season().map(|(_, season)| season),
        Some("season-2".into()),
        "the workspace resolved the episode's season"
    );
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("ep-2b".into()),
        "the chosen episode is selected"
    );
    assert!(
        harness.model().pending_episode_selection.is_none(),
        "the deep selection is fully consumed"
    );
    assert_ne!(
        harness.model().app.status_severity,
        ToastSeverity::Error,
        "a successful deep selection does not flash: {}",
        harness.model().app.status
    );
}

/// Task 6.1: the episode's season episodes are uncached — the deep selection
/// fetches them through the lazy season-fetch seam (season by season, in
/// order) and applies the selection on the fetch's drain.
#[test]
fn navigated_workspace_fetches_uncached_season_episodes_and_selects() {
    let http = MockHttp::new();
    let mut harness = deep_selection_tv_harness(&http, &["ep-2a", "ep-2b"]);
    // Drop every cached episode map: only a fetch can satisfy it.
    harness
        .model_mut()
        .app
        .series_detail_cache
        .get_mut("ser1")
        .expect("pre-seeded detail")
        .episodes
        .clear();
    // Scripted responses for the two season fetches, in arm order: Season 1
    // without the target, Season 2 with it. (The detail itself is cached, so
    // these are the only requests the navigation makes.)
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-1","Name":"Pilot","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-2b","Name":"Episode 1","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    harness
        .model_mut()
        .handle_inline_search_lib_event(episode_navigate(Some("ep-2b".into())));
    harness.step();

    // The immediate attempt armed the first uncached season's fetch and
    // stayed armed.
    assert!(
        harness
            .model()
            .app
            .series_season_loading
            .contains(&("ser1".into(), "season-1".into())),
        "the uncached season fetch was armed"
    );
    assert!(
        harness.model().pending_episode_selection.is_some(),
        "the deep selection stays armed while the fetch is in flight"
    );

    // Season 1 lands (without the target episode): the retry moves on to
    // Season 2's fetch.
    recv_season_episodes(&mut harness, "season-1");
    harness.step();
    assert!(
        harness.model().pending_episode_selection.is_some(),
        "the deep selection survives a season that does not hold the episode"
    );

    // Season 2 lands (with the target episode): the selection applies.
    recv_season_episodes(&mut harness, "season-2");
    harness.step();

    assert_eq!(
        tv(&harness).selected_season().map(|(_, season)| season),
        Some("season-2".into()),
        "the fetch resolved the episode's season"
    );
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("ep-2b".into()),
        "the chosen episode is selected once its season's episodes land"
    );
    assert!(harness.model().pending_episode_selection.is_none());
}

/// Task 6.1: the episode is absent from the fetched detail — the landing
/// stands with the default selection and no error (the navigation target was
/// reached).
#[test]
fn absent_episode_keeps_the_landing_with_default_selection() {
    let mut harness = deep_selection_tv_harness(&MockHttp::new(), &["ep-2a", "ep-2b"]);
    harness
        .model_mut()
        .handle_inline_search_lib_event(episode_navigate(Some("ep-gone".into())));
    harness.step();

    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("ser1".into()),
        "the show landing stands"
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the workspace is open (the hand-off is unaffected)"
    );
    assert_eq!(
        tv(&harness).selected_season().map(|(_, season)| season),
        Some("season-1".into()),
        "default selection: first season"
    );
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("ep-1".into()),
        "default selection: first episode"
    );
    assert!(
        harness.model().pending_episode_selection.is_none(),
        "the absent-episode pending is cleared"
    );
    assert_ne!(
        harness.model().app.status_severity,
        ToastSeverity::Error,
        "absence is not failure: {}",
        harness.model().app.status
    );
}

/// Task 6.1 (Narrow): the landing opens the Library Hero overlay for the
/// show; the deep selection applies to the overlay's presented episode list.
#[test]
fn navigated_narrow_overlay_selects_the_episode() {
    let mut harness = deep_selection_tv_harness(&MockHttp::new(), &["ep-2a", "ep-2b"]);
    harness.model_mut().app.terminal_width = 80;
    harness.model_mut().sync_mounted_surfaces();

    harness
        .model_mut()
        .handle_inline_search_lib_event(episode_navigate(Some("ep-2b".into())));
    harness.step();

    assert!(
        panel_overlay_open(&harness),
        "narrow navigation opens the Library Hero overlay for the show"
    );
    assert_eq!(
        tv(&harness).selected_item().map(|item| item.id),
        Some("ser1".into())
    );
    assert!(
        tv(&harness).episode_pane_focused(),
        "the overlay's presented episode list holds the selection"
    );
    assert_eq!(
        tv(&harness).selected_episode_item().map(|item| item.id),
        Some("ep-2b".into())
    );
}

/// The Library panel's hero-overlay bit, via the panel's test accessor.
fn panel_overlay_open(harness: &TickHarness) -> bool {
    use crate::app::components::library_panel::LibraryPanel;
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .map(|panel| panel.test_hero_overlay_open())
        .unwrap_or(false)
}
