use super::*;
use crate::app::tests::install_test_emby;
use crate::app::{BrowseLevel, LibraryTab};
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::ServerRows;

#[test]
fn enqueue_route_conflict_rejects_mismatched_route() {
    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    assert!(app.enqueue_route_conflict(Some(&"movies".to_string())));
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

#[test]
fn enqueue_route_conflict_allows_enqueue_while_on_a_non_route_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_remote_player::RemotePlayer::stub(make_items(1), 0);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = remote_rx;
    // active_route stays None: this is a Sessions-panel direct-remote
    // connection, not a library route.
    assert!(!app.enqueue_route_conflict(Some(&"music".to_string())));
}

#[test]
fn play_item_submits_selected_item_to_direct_remote_owner() {
    let mut app = make_app_stub();
    let stale_item = make_item("Stale", "Movie");
    let (remote, remote_rx, command_rx) =
        mbv_remote_player::RemotePlayer::stub_with_command_rx(vec![stale_item], 0);
    let sess = mbv_emby::test_support::make_session("remote-mbv", "mbv");
    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    let mut selected = make_item("Selected", "Movie");
    selected.id = "selected-id".into();
    app.play_item(selected);

    let mut replacement = None;
    for command in command_rx.try_iter() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } = command {
            replacement = Some(slots);
            break;
        }
    }
    let slots = replacement.expect("play should submit a queue");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].item.id(), "selected-id");
}

/// Row 3.6: library autoplay (`select_item` with autoload) replaces a
/// populated queue without the replacement confirmation -- a browse
/// activation is not a user-initiated queue replacement.
#[test]
fn library_autoplay_on_a_populated_queue_does_not_raise_the_replace_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let http = MockHttp::new();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    app.config.lock().unwrap().autoload = true;
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    // Populated target queue + a browse level whose parent holds the item.
    let mut existing = make_item("Existing", "Movie");
    existing.id = "existing".into();
    app.local_view.adopt_items(vec![existing], 0);
    let mut anchor = make_item("Anchor", "Movie");
    anchor.id = "anchor-1".into();
    let mut sibling = make_item("Sibling", "Movie");
    sibling.id = "sibling-1".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "parent-1".into(),
            title: "Movies".into(),
            items: vec![anchor.clone()],
            rows: ServerRows::complete(1),

            resting: mbv_ui_model::browse::BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(anchor.clone())
    });

    // Request 1: `get_items_by_ids` resolves the activated row; request 2:
    // `get_direct_playable` resolves the autoload siblings.
    http.respond(
        200,
        r#"{"Items":[{"Id":"anchor-1","Name":"Anchor","Type":"Movie","MediaType":"Video"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[
            {"Id":"anchor-1","Name":"Anchor","Type":"Movie","MediaType":"Video"},
            {"Id":"sibling-1","Name":"Sibling","Type":"Movie","MediaType":"Video"}
        ]}"#,
    );

    app.select_item(0, anchor.clone());

    assert!(
        !matches!(
            app.pending_overlay,
            Some(mbv_ui_model::overlay::OverlayRequest::Confirm(_))
        ),
        "library autoplay is never gated"
    );
    assert!(!app.queue_deferrals.has_gated_replacement());
    // The queue itself changes only through the owner's answer (row 5.3):
    // this stub owner is legacy, so the view keeps its last accepted state
    // and the optimistic replacement content is not asserted here.
}

/// The episode item ids and `start_idx` of the last `UnifiedQueueReplace` the
/// owner received. Row 5.3 makes the owner's wire the queue's only
/// observable content.
fn last_replacement(
    command_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) -> Option<(Vec<String>, Option<usize>)> {
    command_rx
        .try_iter()
        .fold(None, |last, command| match command {
            mbv_ctrl::CtrlCmd::UnifiedQueueReplace {
                slots, start_idx, ..
            } => Some((
                slots
                    .into_iter()
                    .map(|slot| slot.item.id().to_string())
                    .collect(),
                start_idx,
            )),
            _ => last,
        })
}

fn tv_season(id: &str) -> EmbyItem {
    let mut season = make_item(id, "Season");
    season.id = id.into();
    season.is_folder = true;
    season
}

fn tv_episode(id: &str, name: &str, series_id: &str) -> EmbyItem {
    let mut episode = make_item(name, "Episode");
    episode.id = id.into();
    episode.series_id = series_id.into();
    episode
}

/// A TV-library app whose Emby calls reach `http` under the given autoload
/// policy, with the cached series detail the TV Workspace renders from:
/// the selected episode lives in `season-1`.
fn tv_episode_app(
    http: &MockHttp,
    autoload: bool,
) -> (App, std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>) {
    let (mut app, command_rx) =
        crate::app::tests::make_remote_app_stub_with_cmd_rx(Vec::new(), Vec::new());
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    app.config.lock().unwrap().autoload = autoload;
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    app.series_detail_cache.insert(
        "series-1".into(),
        mbv_ui_model::browse::SeriesDetail {
            seasons: vec![tv_season("season-1")],
            episodes: std::collections::HashMap::from([(
                "season-1".to_string(),
                vec![
                    tv_episode("episode-1", "Episode 1", "series-1"),
                    tv_episode("episode-2", "Episode 2", "series-1"),
                ],
            )]),
        },
    );
    (app, command_rx)
}

/// Regression: `TvEpisodeActivate` used to call `play_item` directly, so a TV
/// Workspace episode Enter ignored `mpv.autoload` and queued one episode while
/// a play from the generic browser queued its folder siblings. With autoload
/// on, the episode's cached season fills the queue from that episode.
#[test]
fn tv_episode_play_queues_the_cached_season_when_autoload_is_on() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let (mut app, command_rx) = tv_episode_app(&http, true);
    http.respond(
        200,
        r#"{"Items":[
            {"Id":"episode-1","Name":"Episode 1","Type":"Episode","MediaType":"Video"},
            {"Id":"episode-2","Name":"Episode 2","Type":"Episode","MediaType":"Video"}
        ]}"#,
    );

    app.play_tv_episode(tv_episode("episode-2", "Episode 2", "series-1"));

    assert_eq!(
        last_replacement(&command_rx),
        Some((
            vec!["episode-1".to_string(), "episode-2".to_string()],
            Some(1)
        )),
        "the activated episode's season fills the queue, starting at that episode"
    );
}

/// The same activation under a disabled autoload policy queues the episode
/// alone, so the policy alone decides the TV episode queue.
#[test]
fn tv_episode_play_queues_only_that_episode_when_autoload_is_off() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let (mut app, command_rx) = tv_episode_app(&http, false);

    app.play_tv_episode(tv_episode("episode-2", "Episode 2", "series-1"));

    assert_eq!(
        last_replacement(&command_rx),
        Some((vec!["episode-2".to_string()], Some(0))),
        "autoload off plays the activated episode alone"
    );
}
