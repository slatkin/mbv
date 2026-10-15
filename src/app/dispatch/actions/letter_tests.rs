use mbv_emby_model::test_support::make_item;
use mbv_ui_model::browse::BrowseResting;
use mbv_ui_model::browse::ServerRows;

use crate::app::dispatch::library::browse::full_library_fetch_limit;
use crate::app::state::app_struct::TvUpcomingFetchState;
use crate::app::tests::{make_app_stub, make_items};
use crate::app::{App, BrowseEvent, BrowseLevel, LibEvent, LibraryTab, ModelContentEvent};
use mbv_emby_model::{EmbyItem, UpcomingPremiere};
use rstest::rstest;

fn lib_tab(collection_type: &str) -> LibraryTab {
    let mut library = make_item("Lib", "CollectionFolder");
    library.id = "lib-1".into();
    library.collection_type = collection_type.into();
    LibraryTab::new(library)
}

/// Installs an Emby runtime whose client answers from the returned `MockHttp`.
fn install_mock_emby(app: &mut App) -> mbv_net::mock_http::MockHttp {
    let config = crate::config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..crate::config::Config::default()
    };
    let http = mbv_net::mock_http::MockHttp::new();
    let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    http
}

#[test]
fn active_lib_is_tvshows_true_only_on_a_tvshows_library_tab() {
    // `shuffle_folder` (issue: TV libraries should shuffle from a
    // video-only fetch, everything else from the broader playable-items
    // fetch) branches on this. The matched Emby library index arrives as a
    // parameter (from `shuffle_play` / `execute_context_action`); the index
    // is the library under test, not the selected tab.
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    app.libs.push(lib_tab("music"));

    assert!(
        app.active_lib_is_tvshows(0),
        "library 0 is a tvshows library"
    );

    assert!(
        !app.active_lib_is_tvshows(1),
        "library 1 is a music library"
    );
}

/// Pushes a top-level, non-loading, non-searching `BrowseLevel` onto
/// `lib`'s `nav_stack` -- the minimum state `should_show_letter_pills`
/// needs to consider the library "at its top browse level".
fn push_top_level(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
        rows: ServerRows::new(item_count),
        parent_id: lib.library.id.clone(),
        title: lib.library.name.clone(),
        items: make_items(item_count),

        resting: BrowseResting::new(0, 0),
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
}

#[test]
fn should_show_letter_pills_true_for_any_captured_total() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);

    assert!(!app.should_show_letter_pills(0));

    app.libs[0].library_total = Some(5);
    assert!(
        app.should_show_letter_pills(0),
        "any captured total qualifies"
    );
}

#[test]
fn should_show_letter_pills_excludes_music_and_drilldowns() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("music"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    assert!(
        !app.should_show_letter_pills(0),
        "music libraries use group pills instead"
    );

    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[1], 10);
    app.libs[1].library_total = Some(1000);
    assert!(app.should_show_letter_pills(1));

    // A second nav level (drilled into a folder) is no longer the "top"
    // browse level.
    push_top_level(&mut app.libs[1], 5);
    assert!(
        !app.should_show_letter_pills(1),
        "hidden below the top browse level"
    );

    app.libs.push(lib_tab("homevideos"));
    push_top_level(&mut app.libs[2], 10);
    app.libs[2].library_total = Some(1000);
    assert!(
        !app.should_show_letter_pills(2),
        "home video libraries use folder pills instead"
    );
}

// Regression coverage for the bug found in review of the letter-pills
// PR: `spawn_all_items_prefetch`/`spawn_search_items_load` used to cap
// their unfiltered fetch's `limit` at `lvl.total_count`, which is the
// FILTERED range's count whenever a letter pill is active (e.g. ~40 for
// an `M–O` pill out of a 3,000-movie library) -- so `all_items` (the set
// `/`-search runs over) silently shrank to just the active range, and
// whole-library search missed everything outside it.
#[test]
fn full_library_fetch_limit_uses_true_total_not_the_filtered_range_count() {
    let mut lib = lib_tab("movies");
    push_top_level(&mut lib, 40); // the "M–O" slice: 40 items
    lib.library_total = Some(3000); // the library's true size
    {
        let lvl = lib.nav_stack.last_mut().unwrap();
        lvl.rows = ServerRows::complete(40); // what get_items_sorted_ranged reported for M–O
        lvl.letter_filter = mbv_ui_model::sort_filter::LetterFilter::for_index_for_kind(
            4,
            mbv_ui_model::sort_filter::LetterFilterKind::Movie,
        );
    };
    let lvl = lib.nav_stack.last().unwrap();

    assert_eq!(
        full_library_fetch_limit(&lib, lvl),
        3000,
        "must fetch the whole library, not just the active M–O range"
    );
}

fn push_top_level_tv(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
        rows: ServerRows::new(item_count),
        parent_id: lib.library.id.clone(),
        title: lib.library.name.clone(),
        items: make_items(item_count),

        resting: BrowseResting::new(0, 0),
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
}

#[rstest]
#[case::should_show_letter_pills_true_for_any_tvshows_total(5)]
fn should_show_letter_pills_true_for_tvshows_total(#[case] total: usize) {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(total);

    assert!(app.should_show_letter_pills(0));
}

#[test]
fn tv_first_capture_resolves_latest_and_replaces_large_library_rows() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 301);
    app.libs[0].nav_stack[0].rows = ServerRows::complete(301);

    app.maybe_capture_library_total_and_apply_default_pill(0);

    assert_eq!(
        app.libs[0].tv_content_mode,
        Some(mbv_queue::TvContentMode::Latest)
    );
    assert_eq!(
        app.libs[0].nav_stack[0].items,
        [] as [mbv_emby_model::EmbyItem; 0]
    );
    assert_eq!(
        app.libs[0].nav_stack[0].item_types.as_deref(),
        Some("Episode")
    );
    assert!(app.libs[0].nav_stack[0].loading);
}

fn series(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Series");
    item.id = id.into();
    item.is_folder = true;
    item
}

#[test]
fn activate_searched_series_marks_the_series_pill_and_cursor() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 1);
    app.libs[0].library_total = Some(1000);
    let corpus = vec![series("series-a", "Antelope"), series("series-z", "Zebra")];
    {
        let level = app.libs[0].nav_stack.last_mut().unwrap();
        level.all_items = Some(corpus.clone());
        level.items = corpus.clone();
        level.rows = ServerRows::complete(2);
    };
    let zebra = series("series-z", "Zebra");

    assert!(app.activate_searched_series(0, &zebra));

    let level = app.libs[0].nav_stack.last().unwrap();
    let filter = level.letter_filter.as_ref().expect("pill group marked");
    assert_eq!(filter.label, "S-Z");
    assert_eq!(
        level
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["series-z"],
        "the level list narrows to the marked pill's range"
    );
    assert_eq!(level.rows.total(), 1);
    assert_eq!(level.resting().cursor(), 0, "cursor rests on the series");
    assert_eq!(
        level.all_items.as_ref().unwrap().len(),
        2,
        "the search corpus is retained for later searches"
    );
    assert!(!level.loading);
}

#[test]
fn select_letter_pill_scopes_tv_to_series() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    app.select_letter_pill(0, 1); // "J-R"

    let lvl = app.libs[0].nav_stack.last().unwrap();
    let filter = lvl.letter_filter.as_ref().expect("pill should be set");
    assert_eq!(filter.index, 1);
    assert_eq!(filter.label, "J-R");
    assert_eq!(filter.name_ge, Some("J"));
    assert_eq!(filter.name_lt, Some("S"));
    assert_eq!(lvl.item_types, Some("Series".to_string()));
    assert_eq!(lvl.resting().cursor(), 0);
    assert_eq!(lvl.resting().scroll(), 0);
    assert!(lvl.loading, "TV scoped refresh should be in flight");
}

#[test]
fn cycle_letter_pill_wraps_on_tvshows_library() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    app.cycle_letter_pill(0, -1);
    let filter = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .as_ref()
        .unwrap();
    assert_eq!(
        filter.label, "S-Z",
        "wrapping back from default should land on S-Z"
    );

    app.cycle_letter_pill(0, 1);
    let filter = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .as_ref()
        .unwrap();
    assert_eq!(
        filter.label, "A-I",
        "wrapping forward from S-Z should land on A-I"
    );
}

// Regression (user report 2026-10-04): a TV library's `All` pill stopped at
// the first 100 series -- see `is_tv_series_root_view` for why All/Range
// roots paginate to completion without any cursor movement.
#[test]
fn tv_all_root_paginates_to_completion_without_cursor_movement() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 100);
    let lvl = app.libs[0].nav_stack.last_mut().unwrap();
    lvl.tv_content_mode = Some(mbv_queue::TvContentMode::All);
    lvl.rows = ServerRows::loaded(100, 150);

    // Cursor 0 sits far from the loaded edge, so only unconditional
    // completion can arm the next page: the component owns TV cursor
    // movement and never reports it to the App.
    app.maybe_fetch_next_page(0, 0);

    assert!(
        app.libs[0].nav_stack.last().unwrap().loading,
        "an All-mode TV series root must fetch its next page without any cursor movement"
    );
}

#[test]
fn tv_unresolved_mode_capture_load_does_not_paginate_to_completion() {
    // Unresolved mode (`None`) is the large-library capture load the size
    // default is about to replace, so completion stays cursor-proximity.
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 100);
    app.libs[0].nav_stack[0].rows = ServerRows::loaded(100, 2000);

    app.maybe_fetch_next_page(0, 0);

    assert!(!app.libs[0].nav_stack.last().unwrap().loading);
}

#[test]
fn tv_series_root_refresh_re_arms_completion_pagination() {
    // A pill switch or stop-refresh replaces a TV series root's rows in
    // place via `Refreshed` -- no `Loaded` event, no cursor movement -- so
    // the completion chain must re-arm from the refresh itself.
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 3);
    let lvl = app.libs[0].nav_stack.last_mut().unwrap();
    lvl.tv_content_mode = Some(mbv_queue::TvContentMode::All);
    lvl.rows = ServerRows::loaded(3, 5);

    app.handle_lib_event(LibEvent::Browse(BrowseEvent::Refreshed {
        lib_idx: 0,
        parent_id: "lib-1".into(),
        item_types: Some("Series".into()),
        unplayed_only: false,
        items: make_items(3),
        total_count: 5,
    }));

    assert!(
        app.libs[0].nav_stack.last().unwrap().loading,
        "an in-place TV series refresh must re-arm the next page fetch (3 of 5 rows loaded)"
    );
}

// ── TV Upcoming: once-per-session eager load (user rule, 2026-10-11) ─────────

fn upcoming_premiere(series: &str) -> UpcomingPremiere {
    UpcomingPremiere {
        series_id: mbv_ids::ItemId::new(series),
        series_name: series.into(),
        season: Some(1),
        episode: Some(1),
        title: "Premiere".into(),
        premiere_date: "2026-10-12T00:00:00.0000000Z".into(),
    }
}

/// The eager request marks the library only when a fetch can actually go
/// out: the pill select's wait-on-in-flight path treats a mark as a fetch in
/// flight, so a client-less tab load must leave the library unmarked and let
/// a later tab load retry.
#[test]
fn eager_upcoming_request_stays_unmarked_without_a_service_client() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 3);

    app.maybe_spawn_tv_upcoming_snapshot(0);

    assert!(
        app.tv_upcoming_fetches.is_empty(),
        "a client-less tab load must leave the library unmarked"
    );
}

/// A tab re-entry after a failed eager fetch re-requests it: `Failed` is a
/// retryable state, unlike the stored success snapshot, which dedups the
/// whole session. The saved position restores straight into Upcoming (the
/// restore writes the mode onto the root level), and the restore's own fetch
/// is suppressed by the current==saved shortcut — so this eager request is
/// the tab-load retry the spec names.
#[test]
fn failed_upcoming_fetch_retries_on_the_next_tab_load() {
    let mut app = make_app_stub();
    let http = install_mock_emby(&mut app);
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 2);
    app.libs[0].nav_stack[0].tv_content_mode = Some(mbv_queue::TvContentMode::Upcoming);
    app.tv_upcoming_fetches
        .insert("lib-1".into(), TvUpcomingFetchState::Failed);
    http.respond(200, r#"{"Items":[{"Id":"ep-1","Type":"Episode","SeriesId":"series-1","SeriesName":"Series 1"}]}"#);

    app.maybe_spawn_tv_upcoming_snapshot(0);

    assert_eq!(
        app.tv_upcoming_fetches.get("lib-1"),
        Some(&TvUpcomingFetchState::InFlight),
        "the tab-load retry must arm a new fetch"
    );
    let LibEvent::ModelContent(event) =
        app.channels.lib_rx.recv().expect("the retry must complete")
    else {
        panic!("the retry must complete as a model-content event");
    };
    match event {
        ModelContentEvent::TvUpcomingSnapshotFetched { library_id, items } => {
            assert_eq!(library_id, "lib-1");
            assert_eq!(items.len(), 1);
        }
        other => panic!("the retried tab load must complete with a snapshot, got {other:?}"),
    }
}

/// A genuine restore (the current position differs from the saved one) arms
/// its own Upcoming fetch synchronously: the restored level waits `loading`
/// for the restore's fetch. A recorded `Failed` state must not stack a
/// second eager request behind it — the restore's fetch is this tab load's
/// one Upcoming query.
#[test]
fn a_genuine_restore_with_a_failed_fetch_does_not_stack_a_second_eager_fetch() {
    let mut app = make_app_stub();
    let http = install_mock_emby(&mut app);
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 2);
    // The saved position selected Upcoming; the current level no longer
    // carries it, so activation takes the genuine-restore path.
    let mut saved = app.libs[0].library_position_snapshot();
    saved.levels[0].tv_content_mode = Some(mbv_queue::TvContentMode::Upcoming);
    app.library_position_state
        .libraries
        .insert("lib-1".into(), saved);
    app.tv_upcoming_fetches
        .insert("lib-1".into(), TvUpcomingFetchState::Failed);
    // Two responses: on the deduped path only the restore's fetch runs and
    // one stays scripted; without the dedupe both fetches complete and the
    // state assertion below fails instead of hanging the drain.
    http.respond(200, r#"{"Items":[{"Id":"ep-1","Type":"Episode","SeriesId":"series-1","SeriesName":"Series 1"}]}"#);
    http.respond(200, r#"{"Items":[{"Id":"ep-1","Type":"Episode","SeriesId":"series-1","SeriesName":"Series 1"}]}"#);

    app.activate_library_position(0);
    app.maybe_spawn_tv_upcoming_snapshot(0);

    // The restore thread completes before the assertions: its position
    // event is the last one it sends (the snapshot event precedes it).
    loop {
        let event = app
            .channels
            .lib_rx
            .recv()
            .expect("the restore must complete");
        if matches!(
            event,
            LibEvent::Browse(BrowseEvent::RestoreLibraryPosition { .. })
        ) {
            break;
        }
    }

    let level = app.libs[0].nav_stack.last().unwrap();
    assert!(
        level.loading,
        "the restore's placeholder waits for its fetch"
    );
    assert_eq!(
        level.tv_content_mode.as_ref(),
        Some(&mbv_queue::TvContentMode::Upcoming)
    );
    assert_eq!(
        app.tv_upcoming_fetches.get("lib-1"),
        Some(&TvUpcomingFetchState::Failed),
        "the eager request must not stack behind the restore's fetch"
    );
    let upcoming_requests = http
        .requests()
        .into_iter()
        .filter(|request| request.contains("/Shows/Upcoming"))
        .count();
    assert_eq!(upcoming_requests, 1, "one Upcoming query per tab load");
}

/// Selecting the Upcoming pill materializes the root level from the
/// per-session snapshot instead of refetching the slow server query
/// (`destination-latest-modes`-adjacent user rule, 2026-10-11).
#[test]
fn upcoming_pill_select_materializes_the_session_snapshot() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 2);
    app.libs[0].library_total = Some(1000);
    app.libs[0].tv_content_mode = Some(mbv_queue::TvContentMode::Latest);
    let snapshot = vec![upcoming_premiere("series-1"), upcoming_premiere("series-2")];
    app.tv_upcoming_snapshots.insert("lib-1".into(), snapshot);

    app.select_letter_pill(0, 1);

    let level = app.libs[0].nav_stack.last().unwrap();
    assert!(level.items.is_empty(), "premieres are not level items");
    assert_eq!(level.rows, ServerRows::complete(2));
    assert!(!level.loading, "the snapshot needs no fetch");
    assert_eq!(level.item_types.as_deref(), Some("Episode"));
    assert_eq!(level.sort_by, "PremiereDate");
    assert_eq!(level.sort_order, "Ascending");
    assert_eq!(
        app.libs[0].tv_content_mode,
        Some(mbv_queue::TvContentMode::Upcoming)
    );
}

/// With the eager fetch still in flight, selecting Upcoming waits (loading)
/// and the snapshot completion fills the level (`TvUpcomingSnapshotFetched`).
#[test]
fn waiting_upcoming_level_fills_when_the_eager_snapshot_lands() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 2);
    app.libs[0].library_total = Some(1000);
    app.libs[0].tv_content_mode = Some(mbv_queue::TvContentMode::Latest);
    app.tv_upcoming_fetches
        .insert("lib-1".into(), TvUpcomingFetchState::InFlight);

    app.select_letter_pill(0, 1);
    assert!(
        app.libs[0].nav_stack.last().unwrap().loading,
        "the selection waits for the in-flight eager fetch"
    );

    app.accept_tv_upcoming_snapshot("lib-1", vec![upcoming_premiere("series-1")]);

    let level = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.rows, ServerRows::complete(1));
    assert!(!level.loading, "the arrived snapshot unblocks the level");
    assert!(level.items.is_empty(), "premieres are not level items");
    assert_eq!(app.tv_upcoming_snapshots["lib-1"].len(), 1);
    assert!(
        app.tv_upcoming_fetches.is_empty(),
        "the snapshot's arrival clears the in-flight mark"
    );
}

/// A failed eager fetch (all retries exhausted) leaves the library marked
/// `Failed` — the panel names the failure — and unsets a waiting Upcoming
/// level's loading flag; reselecting the pill then respawns the fetch as the
/// visible retry.
#[test]
fn failed_eager_upcoming_fetch_unblocks_the_waiting_level() {
    let mut app = make_app_stub();
    let http = install_mock_emby(&mut app);
    app.libs.push(lib_tab("tvshows"));
    push_top_level_tv(&mut app.libs[0], 2);
    app.libs[0].library_total = Some(1000);
    app.libs[0].tv_content_mode = Some(mbv_queue::TvContentMode::Latest);
    app.tv_upcoming_fetches
        .insert("lib-1".into(), TvUpcomingFetchState::InFlight);

    app.select_letter_pill(0, 1);
    app.tv_upcoming_snapshot_failed("lib-1");

    let level = app.libs[0].nav_stack.last().unwrap();
    assert!(!level.loading, "the failed fetch unblocks the level");
    assert_eq!(
        app.tv_upcoming_fetches["lib-1"],
        TvUpcomingFetchState::Failed,
        "the failure stays visible instead of looking like an empty source"
    );

    // One scripted success: the retry's fetch completes without sleeps, and
    // its completion event waits in the channel until a drain accepts it.
    http.respond(200, r#"{"Items":[{"Id":"ep-1","Type":"Episode","SeriesId":"series-1","SeriesName":"Series 1"}]}"#);
    app.select_letter_pill(0, 1);

    assert!(
        app.libs[0].nav_stack.last().unwrap().loading,
        "reselecting the pill retries the failed fetch"
    );
    assert_eq!(
        app.tv_upcoming_fetches["lib-1"],
        TvUpcomingFetchState::InFlight,
        "the retry marks a new fetch in flight"
    );
}
