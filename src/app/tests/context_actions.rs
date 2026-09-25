use super::{make_app_stub, make_item};
use crate::app::dispatch::notify::ToastSeverity;
use crate::app::ContextAction;
use mbv_core::api::EmbyClient;
use mbv_core::mock_http::MockHttp;
use mbv_core::service_runtime::EmbyRuntime;
use std::sync::{Arc, Mutex};

fn app_with_mock_emby(http: &MockHttp) -> crate::app::App {
    let mut app = make_app_stub();
    app.config.lock().unwrap().server_url = "http://127.0.0.1:1".into();
    let mut client =
        EmbyClient::new(app.config.lock().unwrap().clone()).with_test_agent(http.agent());
    client.user_id = "user".into();
    app.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    app
}

#[test]
fn watched_toggle_ignores_folders_and_audio_before_service_access() {
    let mut app = make_app_stub();
    let severity = app.status_severity;
    let mut folder = make_item("folder", "Folder");
    folder.is_folder = true;
    let audio = make_item("audio", "Audio");

    app.toggle_watched_item(0, folder);
    app.toggle_watched_item(0, audio);

    assert_eq!(app.status_severity, severity);
}

#[test]
fn context_mark_without_emby_reports_unavailable() {
    let mut app = make_app_stub();

    app.execute_context_action(Some(ContextAction::MarkPlayed("item".into())), None);

    assert_eq!(app.status_severity, ToastSeverity::Warning);
}

#[test]
fn context_played_mark_uses_the_requested_remote_mutation() {
    let http = MockHttp::new();
    http.respond(200, "");
    let mut app = app_with_mock_emby(&http);
    app.tab = crate::app::TabSelection::Feeds;

    app.execute_context_action(Some(ContextAction::MarkPlayed("movie".into())), None);

    assert!(http.requests()[0].contains("POST /Users/user/PlayedItems/movie"));
    assert_ne!(app.status_severity, ToastSeverity::Error);
}

#[test]
fn watched_toggle_unmarks_a_played_item_through_emby() {
    let http = MockHttp::new();
    http.respond(200, "");
    let mut app = app_with_mock_emby(&http);
    let mut movie = make_item("movie", "Movie");
    movie.id = "movie".into();
    movie.played = true;
    let severity = app.status_severity;

    app.toggle_watched_item(0, movie);

    assert!(
        http.requests()[0].contains("DELETE /Users/user/PlayedItems/movie"),
        "request: {:?}",
        http.requests()
    );
    assert_eq!(app.status_severity, severity);
}
