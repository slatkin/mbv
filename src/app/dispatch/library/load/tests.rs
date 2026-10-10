use super::*;
use crate::app::tests::make_app_stub;
use crate::app::{DestinationLatestSource, LibraryTab};
use mbv_net::mock_http::MockHttp;

fn library_of(id: &str, name: &str, collection_type: &str) -> LibraryTab {
    let mut item = mbv_emby_model::test_support::make_item(name, "CollectionFolder");
    item.id = id.into();
    item.collection_type = collection_type.into();
    LibraryTab::new(item)
}

fn ready_emby_app(http: &MockHttp) -> crate::app::App {
    let mut app = crate::app::tests::make_app_stub();
    let config = crate::config::Config {
        server_url: "http://127.0.0.1:1".into(),
        ..crate::config::Config::default()
    };
    let client = mbv_emby::EmbyClient::new(config).with_test_agent(http.agent());
    app.emby_runtime = crate::app::state::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    app
}

fn model_content_events(app: &crate::app::App, expected: usize) -> Vec<ModelContentEvent> {
    (0..expected)
        .map(|_| match app.channels.lib_rx.recv().expect("fetch event") {
            LibEvent::ModelContent(event) => match event {
                ModelContentEvent::HomeRecentlyAddedFetched { .. } => event,
                other => panic!("unexpected ModelContentEvent: {other:?}"),
            },
            other => panic!("unexpected LibEvent: {other:?}"),
        })
        .collect()
}

/// Spec `home-latest-sections`, "Recently added combines every Latest-bearing
/// destination": Home's Emby fetch keeps each
/// destination's own Latest rule — a TV library sends the episode query,
/// every other eligible library sends `get_latest` — and an ineligible
/// Music or Playlists library contributes no request at all.
#[test]
fn home_emby_fetch_uses_destination_latest_rules_per_eligible_library() {
    let http = MockHttp::new();
    let mut app = ready_emby_app(&http);
    app.libs = vec![
        library_of("lib-tv", "Shows", "tvshows"),
        library_of("lib-movies", "Movies", "movies"),
        library_of("lib-music", "Music", "music"),
        library_of("lib-playlists", "Playlists", "playlists"),
    ];
    // The eligible libraries' responses in send order: TV first, then
    // Movies; music and playlists get none.
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep-1","Name":"Episode","Type":"Episode"}],"TotalRecordCount":1}"#,
    );
    http.respond(200, r#"[{"Id":"movie-1","Name":"Movie","Type":"Movie"}]"#);

    app.spawn_home_recently_added(mbv_queue::ServiceKind::Emby);
    let events = model_content_events(&app, 2);

    let ids_of = |event: &ModelContentEvent| match event {
        ModelContentEvent::HomeRecentlyAddedFetched { source, items, .. } => (
            source.clone(),
            items
                .iter()
                .filter_map(|item| item.as_emby())
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
        ),
        _ => unreachable!(),
    };
    assert_eq!(
        ids_of(&events[0]),
        (
            DestinationLatestSource::Emby("lib-tv".into()),
            vec!["ep-1".into()]
        )
    );
    assert_eq!(
        ids_of(&events[1]),
        (
            DestinationLatestSource::Emby("lib-movies".into()),
            vec!["movie-1".into()]
        )
    );
    let requests = http.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0].contains("IncludeItemTypes=Episode"),
        "TV must use the episode query: {}",
        requests[0]
    );
    assert!(
        requests[1].contains("GroupItems=true"),
        "non-TV must use get_latest: {}",
        requests[1]
    );
}

/// Spec ("Recently added loads at launch independent of destinations",
/// "One Service is down"): an unconfigured Audiobookshelf contributes
/// nothing, without a network call and without blocking the others.
#[test]
fn home_audiobookshelf_fetch_without_setup_contributes_nothing() {
    let app = make_app_stub();

    app.spawn_home_recently_added(mbv_queue::ServiceKind::Audiobookshelf);

    app.channels.lib_rx.try_recv().unwrap_err();
}
