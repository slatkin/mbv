//! Regression guards for `feed-view-lazy-folder-groups` task 4.1 (design D4,
//! spec `feed-home-video-group-view`, "Marking played keeps the row"): the
//! feed-view arms of the played-removal paths are gone, so marking a video
//! played — from its context menu or by its playback completing — leaves the
//! group view's loaded selection rows in place.

use super::youtube_root_level;
use crate::app::state::service_runtime::EmbyRuntime;
use crate::app::tests::*;
use mbv_emby::EmbyClient;
use mbv_net::mock_http::MockHttp;
use mbv_ui_model::browse::ServerRows;
use std::collections::HashMap;
use std::sync::Mutex;

fn app_with_feed_view_library(http: &MockHttp) -> crate::app::App {
    let mut app = make_app_stub();
    app.config.lock().unwrap().server_url = "http://127.0.0.1:1".into();
    let mut client =
        EmbyClient::new(app.config.lock().unwrap().clone()).with_test_agent(http.agent());
    client.user_id = "user".into();
    app.emby_runtime = EmbyRuntime::ready(Arc::new(Mutex::new(client)));
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = make_item("YouTube", "CollectionFolder");
    library.id = "lib-youtube".into();
    library.collection_type = "homevideos".into();
    library.is_folder = true;
    let mut video = make_item("A1", "Movie");
    video.id = "video-a1".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![youtube_root_level(ServerRows::new(0), vec![], false)],
        feed_home_video: Some(FeedHomeVideoState {
            items: HashMap::from([(FeedHomeVideoSelection::All, vec![video])]),
            ..FeedHomeVideoState::default()
        }),
        ..LibraryTab::new(library)
    });
    app
}

#[test]
fn context_menu_mark_played_keeps_the_feed_view_selection_rows_in_place() {
    let http = MockHttp::new();
    http.respond(200, ""); // mark played
    http.respond(200, "[]"); // the refresh's latest snapshot
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#); // the refresh's page fetch
    let mut app = app_with_feed_view_library(&http);
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;

    app.execute_context_action(Some(ContextAction::MarkPlayed("video-a1".into())), None);

    assert!(
        http.requests()[0].contains("POST /Users/user/PlayedItems/video-a1"),
        "request: {:?}",
        http.requests()
    );
    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(state.selected_len(), 1, "the marked video must stay listed");
    assert_eq!(state.items[&FeedHomeVideoSelection::All][0].id, "video-a1");
}

#[test]
fn playback_completion_keeps_the_feed_view_selection_rows_in_place() {
    let http = MockHttp::new();
    http.respond(
        200,
        r#"[{"ItemId":"lib-youtube","Name":"YouTube","CollectionType":"homevideos"}]"#,
    ); // fetch_home: views
    http.respond(200, r#"{"Items":[]}"#); // fetch_home: continue watching
    http.respond(200, r#"{"Items":[]}"#); // fetch_home: latest
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#); // the lib's refresh fetch
    let mut app = app_with_feed_view_library(&http);

    app.refresh_after_stop();

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selected_len(),
        1,
        "the completed video must stay listed"
    );
    assert_eq!(state.items[&FeedHomeVideoSelection::All][0].id, "video-a1");
    assert!(
        !state.loading,
        "completion must not force the feed state to reload"
    );
}
