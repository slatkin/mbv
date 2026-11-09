//! Tests of the GUI Library's Podcasts data path (add-gui-library-panel
//! design D12, row 10.1): an open of an unloaded show catalog starts the
//! TUI's own show-list chain and the fetched page lands on the App through
//! the GUI tick's drain, the Latest pill lands every listed show's episodes
//! through the TUI's bounded fan-out, a show pill lands that show's episodes
//! only, an episode select stores the pick, the presented destination set
//! names the podcast libraries and restores a saved podcast location, and the
//! Latest shelf's shared one-shot guard requests once. The Audiobookshelf
//! boundary is the lib event channel, so no request leaves the process beyond
//! a mocked loopback port's refusal and no worker outlives the assertion.

use super::*;
use mbv_audiobookshelf::{
    AudiobookshelfDownloadedEpisode, AudiobookshelfShow, AudiobookshelfShowPage,
};
use mbv_config::{GUI_LAUNCH_STATE_VERSION, GuiLaunchState, LaunchPanelFocus, TabIdentity};
use mbv_queue::ServiceKind;
use mbv_ui_model::audiobookshelf_browse::AudiobookshelfBrowseState;
use mbv_ui_msg::PodcastEpisodeTarget;

/// The generation the event arms accept while the test's runtime holds none.
fn generation() -> mbv_core::service_runtime::SetupGeneration {
    mbv_core::service_runtime::SetupGeneration::default()
}

/// One show of a mocked catalog page or browse state.
fn show(id: &str, title: &str) -> AudiobookshelfShow {
    AudiobookshelfShow {
        library_item_id: id.into(),
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

/// A stub `App` whose Audiobookshelf catalog holds one book library and one
/// podcast library, with the podcast destination active and a resolvable
/// Audiobookshelf setup over a closed loopback port: the state a settled
/// Audiobookshelf startup leaves before its show pages land, with the
/// loaders' workers sending their failure class instead of hanging. The
/// caller holds the test state dir, so the secret file and the workers'
/// connection attempts stay inside the scratch path.
fn podcast_app() -> App {
    let mut app = make_app_stub();
    app.seed_gui_abs_libraries_for_test();
    app.config.lock().unwrap().audiobookshelf_setup =
        Some(mbv_config::AudiobookshelfSetup::new("http://127.0.0.1:1"));
    mbv_config::save_service_secret(mbv_queue::ServiceKind::Audiobookshelf, "test-token")
        .expect("the secret is written under the test state dir");
    app.tab = TabSelection::AudiobookshelfLibrary(1);
    app
}

/// The podcast browse state the seeded catalog holds for library 1.
fn podcast_state(app: &App) -> &AudiobookshelfBrowseState {
    &app.audiobookshelf_browse[1]
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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    app.gui_podcast_open(1);
    let request = podcast_state(&app).catalog_request;
    assert_eq!(request, 1, "the open began one catalog chain");
    assert!(
        podcast_state(&app).replacement.is_some(),
        "the open's show-list chain is collecting"
    );

    let _ = app.channels.lib_tx.send(LibEvent::Audiobookshelf(
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
    app.gui_podcast_open(1);
    assert_eq!(
        podcast_state(&app).catalog_request,
        loaded_request,
        "a loaded catalog's open starts no second chain"
    );
}

/// The Latest pill lands every listed show's episodes through the TUI's
/// bounded fan-out (row 10.1, contract b): the pill commits no show scope, so
/// the fan-out requests both shows, and each landed completion caches that
/// show's episodes on the App — the data the list's merged order is projected
/// from.
#[test]
fn the_latest_pill_lands_every_shows_episodes_through_the_fan_out() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.audiobookshelf_browse[1].shows = vec![show("show-a", "Show A"), show("show-b", "Show B")];
    app.audiobookshelf_browse[1].total = 2;

    app.gui_podcast_set_pill(1, GuiPodcastPill::Latest);

    assert_eq!(app.gui_podcast_pill(1), GuiPodcastPill::Latest);
    assert_eq!(
        podcast_state(&app).committed_show_pill,
        None,
        "Latest needs every listed show"
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
        let _ = app.channels.lib_tx.send(LibEvent::Audiobookshelf(
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

/// A show pill lands that show's episodes only (row 10.1, contract c): the
/// pill commits the show scope the bounded fan-out reads, so one show is
/// requested and the other show's cache stays empty.
#[test]
fn a_show_pill_lands_only_that_shows_episodes() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.audiobookshelf_browse[1].shows = vec![show("show-a", "Show A"), show("show-b", "Show B")];
    app.audiobookshelf_browse[1].total = 2;

    app.gui_podcast_set_pill(1, GuiPodcastPill::Show("show-b".into()));

    assert_eq!(
        app.gui_podcast_pill(1),
        GuiPodcastPill::Show("show-b".into())
    );
    assert_eq!(
        podcast_state(&app).committed_show_pill.as_deref(),
        Some("show-b"),
        "the show pill scopes the fan-out"
    );
    let marks = detail_marks(&app);
    assert_eq!(marks.len(), 1, "one show pill requests one show");
    assert_eq!(marks[0].0, "show-b");
    consume_detail_completions(&mut app, 1);

    let _ = app.channels.lib_tx.send(LibEvent::Audiobookshelf(
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
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();
    app.audiobookshelf_browse[1].shows = vec![show("show-a", "Show A")];
    app.audiobookshelf_browse[1]
        .cache_detail("show-a".into(), vec![episode("show-a", "ep-1", None)]);

    let target = PodcastEpisodeTarget::new("show-a".into(), "ep-1".into());
    app.gui_podcast_select(1, &target);
    assert_eq!(
        app.gui_podcast_picked_episode(1),
        Some(&target),
        "the picked episode is the select command's"
    );

    app.gui_podcast_select(
        1,
        &PodcastEpisodeTarget::new("show-a".into(), "gone".into()),
    );
    assert_eq!(
        app.gui_podcast_picked_episode(1),
        Some(&target),
        "a stale episode id changes nothing"
    );
}

/// The presented destination set names both Audiobookshelf destinations, and
/// a location saved on the podcast library restores to it with no pill
/// persistence (row 10.1, contract d): the podcast library presents the
/// Podcasts tab at its own position, the active podcast destination's kind is
/// the Podcasts one, and the restore applies no saved pill because the
/// Podcasts destination keeps no pill in the launch file.
#[test]
fn the_presented_tabs_include_a_podcast_library_and_a_saved_location_restores_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    let tabs = app.gui_library_tabs();
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
        app.gui_active_destination(),
        Some(crate::app::gui_library::GuiDestinationKind::Podcasts { lib_idx: 1 }),
        "the active podcast destination's kind is the Podcasts one"
    );

    app.tab = TabSelection::Home;
    let saved = GuiLaunchState {
        version: GUI_LAUNCH_STATE_VERSION,
        destination: TabIdentity::ServiceLibrary {
            kind: ServiceKind::Audiobookshelf,
            library_id: "lib-abs-podcasts".into(),
        },
        panel_focus: LaunchPanelFocus::Library,
        collections: Vec::new(),
    };
    let opened = app
        .open_gui_launch_destination(&saved)
        .expect("the settled catalog resolves the saved library");
    assert_eq!(
        app.tab,
        TabSelection::AudiobookshelfLibrary(1),
        "the saved Podcasts destination restored"
    );
    assert!(
        app.restore_gui_launch_collections(&saved, &opened),
        "the Podcasts destination applies no saved pill and needs no retry"
    );
    assert_eq!(
        app.gui_podcast_pill(1),
        GuiPodcastPill::Latest,
        "the restored destination keeps the default Latest pill"
    );
}

/// The Latest shelf's shared one-shot guard requests once (row 10.1, the
/// shell half's App-state move): the TUI's projection pass and the GUI's
/// frame path call the same method, and the request set makes the second call
/// load nothing.
#[test]
fn the_latest_shelfs_shared_guard_requests_once() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = podcast_app();

    assert!(
        !app.audiobookshelf_shelf_requested
            .contains("lib-abs-podcasts")
    );
    assert!(
        app.ensure_audiobookshelf_latest_shelf("lib-abs-podcasts"),
        "the first call started the shelf fetch"
    );
    assert!(
        !app.ensure_audiobookshelf_latest_shelf("lib-abs-podcasts"),
        "the request set is single-flight"
    );
    assert!(
        app.audiobookshelf_shelf_requested
            .contains("lib-abs-podcasts")
    );
}
