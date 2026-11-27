use super::{make_app_stub, make_item};
use crate::app::ContextAction;
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::state::service_runtime::EmbyRuntime;
use mbv_emby::EmbyClient;
use mbv_net::mock_http::MockHttp;
use std::sync::{Arc, Mutex};

/// The mock-Emby app plus its owner command receiver: an answered local
/// owner must hold the channel open, because the played-state propagation
/// every accepted mark sends otherwise fails and its warning toast pollutes
/// the severity assertions.
fn app_with_mock_emby(
    http: &MockHttp,
) -> (
    crate::app::App,
    std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) {
    let mut app = make_app_stub();
    app.config.lock().unwrap().server_url = "http://127.0.0.1:1".into();
    let mut client =
        EmbyClient::new(app.config.lock().unwrap().clone()).with_test_agent(http.agent());
    client.user_id = "user".into();
    app.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    let (remote, player_rx, cmd_rx) =
        mbv_remote_player::RemotePlayer::stub_answered_queue_ops_with_command_rx(Vec::new(), 0);
    app.player = mbv_player::PlayerProxy::from_remote(remote, false);
    app.player_rx = player_rx;
    (app, cmd_rx)
}

#[test]
fn context_played_mark_uses_the_requested_remote_mutation() {
    let http = MockHttp::new();
    http.respond(200, "");
    let (mut app, _cmd_rx) = app_with_mock_emby(&http);
    app.tab = mbv_ui_model::tab_selection::TabSelection::Feeds;

    app.execute_context_action(Some(ContextAction::MarkPlayed("movie".into())), None);

    assert!(http.requests()[0].contains("POST /Users/user/PlayedItems/movie"));
    assert_ne!(app.newest_toast_severity(), ToastSeverity::Error);
}

/// An accepted Emby mark propagates the new played pair into the Player
/// owner's queue through `UnifiedQueueApplyProgress` (the same propagation
/// the Audiobookshelf accepted-mark apply uses), so a queued occurrence's row
/// repaints from the owner's broadcast. The server clears the resume position
/// on either mark, so the pair is `(0, played)`.
#[test]
fn context_played_mark_propagates_the_new_state_to_the_owner_queue() {
    let http = MockHttp::new();
    http.respond(200, "");
    let (mut app, cmd_rx) = app_with_mock_emby(&http);
    app.tab = mbv_ui_model::tab_selection::TabSelection::Feeds;

    app.execute_context_action(Some(ContextAction::MarkPlayed("movie".into())), None);

    assert!(
        matches!(cmd_rx.try_recv(), Ok(mbv_ctrl::CtrlCmd::UnifiedQueueApplyProgress { updates, .. })
        if updates == vec![mbv_ctrl::ProgressUpdate {
            content_id: mbv_queue::QueueItemContentId::Emby("movie".into()),
            position_ticks: 0,
            finished: true,
        }]),
        "the mark must reach the owner queue as one apply-progress update"
    );
}

#[test]
fn watched_toggle_unmarks_a_played_item_through_emby() {
    let http = MockHttp::new();
    http.respond(200, "");
    let (mut app, _cmd_rx) = app_with_mock_emby(&http);
    let mut movie = make_item("movie", "Movie");
    movie.id = "movie".into();
    movie.played = true;
    let severity = app.newest_toast_severity();

    app.toggle_watched_item(0, &movie);

    assert!(
        http.requests()[0].contains("DELETE /Users/user/PlayedItems/movie"),
        "request: {:?}",
        http.requests()
    );
    assert_eq!(app.newest_toast_severity(), severity);
}
