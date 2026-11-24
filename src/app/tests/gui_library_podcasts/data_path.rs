//! Tests of the GUI Library's Podcasts data path (add-gui-library-panel
//! design D12, row 10.1): an open of an unloaded show catalog starts the
//! TUI's own show-list chain and the fetched page lands on the App through
//! the GUI tick's drain, All lands every listed show's episodes through the
//! TUI's bounded fan-out, a checked show lands that show's episodes only, an
//! episode select stores the pick, the presented destination set names the
//! podcast libraries and restores a saved podcast location, and the All
//! shelf's shared one-shot guard requests once. The Audiobookshelf boundary
//! is the lib event channel, so no request leaves the process beyond a mocked
//! loopback port's refusal and no worker outlives the assertion.
//!
//! The tests sit in their own child module of `gui_library_podcasts`, beside
//! the projection's and the commands' tests, so that file stays within its
//! line ceiling.

use super::*;
use mbv_audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfShow, AudiobookshelfShowPage,
};
use mbv_config::{GUI_LAUNCH_STATE_VERSION, GuiLaunchState, TabIdentity};
use mbv_queue::ServiceKind;
use mbv_ui_msg::PodcastEpisodeTarget;

/// The generation the event arms accept while the test's runtime holds none.
fn generation() -> mbv_core::service_runtime::SetupGeneration {
    mbv_core::service_runtime::SetupGeneration::default()
}

/// One show of a mocked catalog page or browse state.
fn show(id: &str, title: &str) -> AudiobookshelfShow {
    AudiobookshelfShow {
        library_item_id: id.into(),
        updated_at_ms: 0,
        title: title.into(),
        author: None,
        description: None,
        cover_path: None,
    }
}

/// One downloaded episode of a mocked per-show fetch.
fn episode(
    show_id: &str,
    episode_id: &str,
    published_at: Option<u64>,
) -> AudiobookshelfDownloadedEpisode {
    AudiobookshelfDownloadedEpisode {
        library_item_id: show_id.into(),
        episode_id: episode_id.into(),
        title: episode_id.into(),
        description: None,
        published_at,
        duration_seconds: Some(600.0),
    }
}

/// The in-flight per-show fetch marks, as `(show id, request serial)` pairs in
/// show order: the fan-out's one observable decision.
fn detail_marks(app: &App) -> Vec<(String, u64)> {
    podcast_state(app)
        .shows
        .iter()
        .filter_map(|show| {
            podcast_state(app)
                .detail_loading_ids
                .get(&show.library_item_id)
                .map(|request| (show.library_item_id.clone(), *request))
        })
        .collect()
}

/// Consume `count` of the fan-out's own detail completions off the lib
/// channel: each spawned worker reports exactly one, and consuming it raw
/// leaves the fetch marks in place for the test's own completion below.
fn consume_detail_completions(app: &mut App, count: usize) {
    let mut consumed = 0;
    while consumed < count {
        let event = app
            .channels
            .lib_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("the fan-out's detail fetch reported its completion");
        if matches!(
            event,
            LibEvent::Audiobookshelf(AudiobookshelfEvent::DetailFetched { .. })
        ) {
            consumed += 1;
        }
    }
}

/// Opening an unloaded podcast library starts the TUI's own show-list chain
/// and the fetched page lands on the App through the GUI tick's drain (row
/// 10.1, contract a): the open arms a single replacement batch, the page
/// event's handler publishes the show list the projection reads, and an open
/// of the loaded catalog starts no second chain.
#[test]
fn opening_a_podcast_library_lands_its_show_list_on_the_app() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    gui_podcast_open(&mut app, 1);
    let request = podcast_state(&app).catalog_request;
    assert_eq!(request, 1, "the open began one catalog chain");
    assert!(
        podcast_state(&app).replacement.is_some(),
        "the open's show-list chain is collecting"
    );

    app.send_lib_event_for_test(LibEvent::Audiobookshelf(
        AudiobookshelfEvent::ShowsFetched {
            generation: generation(),
            request,
            library_id: "lib-abs-podcasts".into(),
            result: Ok(AudiobookshelfShowPage {
                page: 0,
                limit: 20,
                total: 1,
                items: vec![show("show-a", "Show A")],
            }),
        },
    ));
    while app.drain_lib_event() {}

    let state = podcast_state(&app);
    assert_eq!(
        state
            .shows
            .iter()
            .map(|show| show.library_item_id.as_str())
            .collect::<Vec<_>>(),
        ["show-a"],
        "the fetched page landed on the App's browse state"
    );
    assert_eq!(state.total, 1, "the page's total is the catalog's");
    assert!(state.replacement.is_none(), "the completed batch published");

    let loaded_request = podcast_state(&app).catalog_request;
    gui_podcast_open(&mut app, 1);
    assert_eq!(
        podcast_state(&app).catalog_request,
        loaded_request,
        "a loaded catalog's open starts no second chain"
    );
}

/// A restored Podcasts destination starts its show-list load from one frame's
/// load pass, with no open command (gui-one-destination-ensure design D3; spec
/// `gui-client`, "A restored Audiobooks or Podcasts library loads its
/// catalog"): the root step arms the show list's replacement batch, and the
/// pass ends there, so the selection's loads wait for the catalog to land.
#[test]
fn a_restored_podcasts_destination_starts_its_show_list_load() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    crate::app::tests::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Podcasts { lib_idx: 1 },
    );

    let state = podcast_state(&app);
    assert_eq!(
        state.catalog_request, 1,
        "the frame began one catalog chain"
    );
    assert!(
        state.replacement.is_some(),
        "the show list's replacement batch is collecting"
    );
}

/// One frame's load pass for library 1's show catalog, as the catalog request
/// serial it left: an armed replacement batch bumps the serial.
fn pass_catalog_request(app: &mut App) -> u64 {
    crate::app::tests::ensure_gui_destination(app, GuiDestinationKind::Podcasts { lib_idx: 1 });
    podcast_state(app).catalog_request
}

/// A repaint beside an in-flight catalog starts no second chain
/// (gui-one-destination-ensure design D3, Risks; spec `gui-client`, "A repaint
/// starts no second load"): the root step keeps the open step's unloaded
/// guard, which counts the in-flight page marks and the collecting replacement
/// batch, so the frame's pass while the startup chain walks — and while a
/// refresh collects — arms nothing beside them.
#[test]
fn a_repaint_beside_an_in_flight_catalog_starts_no_second_chain() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.edit_audiobookshelf_browse_state_for_test(1, |state| {
        state.loading_pages.insert(0);
    });

    assert_eq!(
        pass_catalog_request(&mut app),
        0,
        "the startup chain's in-flight page started no second chain"
    );

    app.edit_audiobookshelf_browse_state_for_test(1, |state| state.loading_pages.clear());
    let armed = app.edit_audiobookshelf_browse_state_for_test(
        1,
        AudiobookshelfBrowseState::begin_catalog_replacement,
    );
    assert_eq!(
        pass_catalog_request(&mut app),
        armed,
        "the collecting replacement batch started no second chain"
    );
}

/// A failed show-list load is not retried by a repaint, and an open retries it
/// (gui-one-destination-ensure design Risks; spec `gui-client`, "A failed load
/// is not retried by a repaint"): the loader leaves the failure in the browse
/// state's `error` beside the still-unloaded list, the frame's root step
/// honours that mark, and the open clears it.
#[test]
fn a_failed_show_list_load_is_not_retried_by_a_repaint_but_an_open_retries_it() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.edit_audiobookshelf_browse_state_for_test(1, |state| {
        state.error = Some("the show list failed to load".into());
    });

    assert_eq!(
        pass_catalog_request(&mut app),
        0,
        "the repaint honoured the failed mark"
    );

    gui_podcast_open(&mut app, 1);
    assert_eq!(
        podcast_state(&app).catalog_request,
        1,
        "the open cleared the failed mark and retried the show list"
    );
}

/// The All row lands every listed show's episodes through the TUI's bounded
/// fan-out (row 10.1, contract b; gui-collection-dropdown design D2, D5): the
/// All selection commits no show scope, so the fan-out requests both shows,
/// and each landed completion caches that show's episodes on the App — the
/// data the list's merged order is projected from.
#[test]
fn the_all_row_lands_every_shows_episodes_through_the_fan_out() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.edit_audiobookshelf_browse_state_for_test(1, |state| {
        state.shows = vec![show("show-a", "Show A"), show("show-b", "Show B")];
        state.total = 2;
    });

    select_all_shows(&mut app, 1);

    assert_eq!(
        crate::app::tests::stub_gui_state(|gui| {
            crate::app::gui_library::podcasts::podcast_effective_set(&app, gui, 1).into_owned()
        }),
        GuiCollectionSet::All
    );
    assert_eq!(
        podcast_state(&app).committed_show_pill,
        None,
        "All needs every listed show"
    );
    let marks = detail_marks(&app);
    assert_eq!(
        marks
            .iter()
            .map(|(show_id, _)| show_id.as_str())
            .collect::<Vec<_>>(),
        ["show-a", "show-b"],
        "the fan-out requested both shows' episodes"
    );
    consume_detail_completions(&mut app, marks.len());

    for (show_id, request) in &marks {
        app.send_lib_event_for_test(LibEvent::Audiobookshelf(
            AudiobookshelfEvent::DetailFetched {
                generation: generation(),
                request: *request,
                library_item_id: show_id.clone(),
                result: Ok(vec![episode(
                    show_id,
                    &format!("{show_id}-ep-1"),
                    Some(1_700_000_000),
                )]),
            },
        ));
    }
    while app.drain_lib_event() {}

    let state = podcast_state(&app);
    for show_id in ["show-a", "show-b"] {
        assert_eq!(
            state.detail_cache.get(show_id).map(Vec::len),
            Some(1),
            "the {show_id} episodes landed on the App"
        );
    }
}

/// A checked show lands that show's episodes only (row 10.1, contract c;
/// gui-collection-dropdown design D2): the single checked show commits the
/// show scope the bounded fan-out reads, so one show is requested and the
/// other show's cache stays empty.
#[test]
fn a_checked_show_lands_only_that_shows_episodes() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.edit_audiobookshelf_browse_state_for_test(1, |state| {
        state.shows = vec![show("show-a", "Show A"), show("show-b", "Show B")];
        state.total = 2;
    });

    toggle_show(&mut app, 1, "show-b");

    assert_eq!(
        crate::app::tests::stub_gui_state(|gui| {
            crate::app::gui_library::podcasts::podcast_effective_set(&app, gui, 1).into_owned()
        }),
        GuiCollectionSet::Checked(vec!["show-b".into()])
    );
    assert_eq!(
        podcast_state(&app).committed_show_pill.as_deref(),
        Some("show-b"),
        "the single checked show scopes the fan-out"
    );
    let marks = detail_marks(&app);
    assert_eq!(marks.len(), 1, "one checked show requests one show");
    assert_eq!(marks[0].0, "show-b");
    consume_detail_completions(&mut app, 1);

    app.send_lib_event_for_test(LibEvent::Audiobookshelf(
        AudiobookshelfEvent::DetailFetched {
            generation: generation(),
            request: marks[0].1,
            library_item_id: "show-b".into(),
            result: Ok(vec![episode("show-b", "show-b-ep-1", None)]),
        },
    ));
    while app.drain_lib_event() {}

    let state = podcast_state(&app);
    assert_eq!(
        state.detail_cache.get("show-b").map(Vec::len),
        Some(1),
        "the scoped show's episodes landed"
    );
    assert!(
        !state.detail_cache.contains_key("show-a"),
        "the unscoped show was never requested"
    );
}

/// An episode select stores the pick the Hero paints (row 10.1): the target's
/// provider identity is kept as the store names it, and an episode the landed
/// cache does not carry changes nothing.
#[test]
fn selecting_an_episode_stores_the_pick_and_ignores_a_stale_one() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.edit_audiobookshelf_browse_state_for_test(1, |state| {
        state.shows = vec![show("show-a", "Show A")];
        state.cache_detail("show-a".into(), vec![episode("show-a", "ep-1", None)]);
    });

    let target = PodcastEpisodeTarget::new("show-a".into(), "ep-1".into());
    gui_podcast_select(&mut app, 1, &target);
    assert_eq!(
        crate::app::tests::stub_gui_state(|gui| {
            crate::app::gui_library::podcasts::podcast_picked_episode(gui, 1).cloned()
        }),
        Some(target.clone()),
        "the picked episode is the select command's"
    );

    gui_podcast_select(
        &mut app,
        1,
        &PodcastEpisodeTarget::new("show-a".into(), "gone".into()),
    );
    assert_eq!(
        crate::app::tests::stub_gui_state(|gui| {
            crate::app::gui_library::podcasts::podcast_picked_episode(gui, 1).cloned()
        }),
        Some(target),
        "a stale episode id changes nothing"
    );
}

/// The presented destination set names both Audiobookshelf destinations, and
/// a location saved on the podcast library restores to it with no collection
/// persistence (row 10.1, contract d): the podcast library presents the
/// Podcasts tab at its own position, the active podcast destination's kind is
/// the Podcasts one, and the restore applies no saved checked set because the
/// Podcasts destination saves nothing (the spec's persistence rule).
#[test]
fn the_presented_tabs_include_a_podcast_library_and_a_saved_location_applies_no_selection() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    let tabs = gui_library_tabs(&app);
    let labels: Vec<&str> = tabs.tabs.iter().map(|tab| tab.label.as_str()).collect();
    assert_eq!(
        labels,
        vec!["Audiobooks", "Podcasts"],
        "each Audiobookshelf library presents its own destination"
    );
    assert_eq!(
        tabs.tabs.iter().map(|tab| tab.position).collect::<Vec<_>>(),
        [1, 2],
        "the podcast library's tab position follows the book library's"
    );
    assert_eq!(tabs.current, Some(2), "the active podcast tab is current");
    assert_eq!(
        gui_active_destination(&app),
        Some(crate::app::gui_library::GuiDestinationKind::Podcasts { lib_idx: 1 }),
        "the active podcast destination's kind is the Podcasts one"
    );

    app.seed_active_tab_for_test(TabSelection::AudiobookshelfLibrary(1));
    let saved = GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "lib-abs-podcasts".into(),
        },
        collections: Vec::new(),
        shows_view: mbv_config::GuiLaunchShowsView::All,
        sorts: Vec::new(),
    };
    let crate::app::GuiLaunchOpen::Opened(opened) =
        crate::app::tests::open_launch_destination(&mut app, &saved)
    else {
        panic!("the catalog is presented, so the start has opened");
    };
    assert!(
        crate::app::tests::restore_launch_collections(&mut app, &saved, &opened),
        "the Podcasts destination applies no saved pill and needs no retry"
    );
    assert_eq!(
        crate::app::tests::stub_gui_state(|gui| {
            crate::app::gui_library::podcasts::podcast_effective_set(&app, gui, 1).into_owned()
        }),
        GuiCollectionSet::All,
        "the restored destination keeps the default All selection"
    );
}

/// The Latest shelf's shared one-shot guard requests once (the shell half's
/// App-state move): the TUI's projection pass and the GUI's frame path call
/// the same method, and the request set makes the second call load nothing.
#[test]
fn the_latest_shelfs_shared_guard_requests_once() {
    let _guard = crate::app::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    assert!(!app.audiobookshelf_shelf_requested_for_test("lib-abs-podcasts"));
    assert!(
        app.ensure_audiobookshelf_latest_shelf("lib-abs-podcasts"),
        "the first call started the shelf fetch"
    );
    assert!(
        !app.ensure_audiobookshelf_latest_shelf("lib-abs-podcasts"),
        "the request set is single-flight"
    );
    assert!(app.audiobookshelf_shelf_requested_for_test("lib-abs-podcasts"));
}
