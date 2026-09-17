// Unit tests for change `per-destination-item-navigation` tasks 1.1/1.2/4.2:
// the D1 reveal-item table, the `LibEvent::NavigateTo` landing payload, and
// the drained resolve-failure error event. Mocked Emby boundary only
// (`MockHttp`), per the AGENTS.md mocks-only policy.

use super::*;
use crate::app::tests::{install_test_emby, make_app_stub, make_item};
use crate::app::library_browse_actions::{RevealTarget, resolve_reveal_target};
use crate::app::types_events::NavigateLanding;
use mbv_core::mock_http::MockHttp;
use rstest::rstest;
use std::time::Duration;

fn ancestor(id: &str, item_type: &str) -> EmbyItem {
    let mut item = make_item(id, item_type);
    item.id = id.into();
    item
}

/// D1 reveal table (task 1.1): item_type + the item's own back-references +
/// the ancestor chain (nearest→root) → the single reveal target. An empty
/// ancestors row means the worker never needed the round trip.
#[rstest]
#[case::episode_series_id_decides_without_the_round_trip(
    "Episode", "ser1", "", &[],
    Ok(RevealTarget::Series("ser1".into()))
)]
#[case::episode_falls_back_to_the_series_ancestor(
    "Episode", "", "", &[("se1", "Season"), ("ser1", "Series"), ("folder1", "Folder"), ("root", "AggregateFolder")],
    Ok(RevealTarget::Series("ser1".into()))
)]
#[case::episode_without_a_series_ancestor_is_a_resolve_failure(
    "Episode", "", "", &[("folder1", "Folder"), ("root", "AggregateFolder")],
    Err("Could not resolve the item's series")
)]
#[case::season_resolves_like_an_episode(
    "Season", "ser1", "", &[],
    Ok(RevealTarget::Series("ser1".into()))
)]
#[case::track_album_id_decides("Audio", "", "alb1", &[], Ok(RevealTarget::Album("alb1".into())))]
#[case::track_falls_back_to_the_album_ancestor(
    "Audio", "", "", &[("alb1", "MusicAlbum"), ("folder1", "Folder"), ("root", "AggregateFolder")],
    Ok(RevealTarget::Album("alb1".into()))
)]
#[case::track_without_an_album_ancestor_is_a_resolve_failure(
    "Audio", "", "", &[("folder1", "Folder"), ("root", "AggregateFolder")],
    Err("Could not resolve the item's album")
)]
#[case::album_resolves_itself("MusicAlbum", "", "", &[], Ok(RevealTarget::Album("item1".into())))]
#[case::artist_resolves_its_album_ancestor(
    "MusicArtist", "", "", &[("alb1", "MusicAlbum"), ("folder1", "Folder"), ("root", "AggregateFolder")],
    Ok(RevealTarget::Album("alb1".into()))
)]
#[case::artist_without_an_album_ancestor_is_a_resolve_failure(
    "MusicArtist", "", "", &[("folder1", "Folder"), ("root", "AggregateFolder")],
    Err("Could not resolve the artist's album")
)]
#[case::series_resolves_itself("Series", "", "", &[], Ok(RevealTarget::Series("item1".into())))]
#[case::movie_keeps_the_chain("Movie", "", "", &[], Ok(RevealTarget::Chain))]
#[case::generic_kind_keeps_the_chain("Folder", "", "", &[], Ok(RevealTarget::Chain))]
fn reveal_table(
    #[case] item_type: &str,
    #[case] series_id: &str,
    #[case] album_id: &str,
    #[case] ancestors: &[(&str, &str)],
    #[case] expected: Result<RevealTarget, &str>,
) {
    let mut item = make_item("Item", item_type);
    item.id = "item1".into();
    item.series_id = series_id.into();
    item.album_id = album_id.into();
    let chain: Vec<EmbyItem> = ancestors.iter().map(|(id, t)| ancestor(id, t)).collect();
    let got = resolve_reveal_target(
        item_type,
        &item,
        if chain.is_empty() { None } else { Some(&chain) },
    );
    match expected {
        Ok(expected) => assert_eq!(got, Ok(expected)),
        Err(msg) => assert_eq!(got.unwrap_err(), msg),
    }
}

/// App stub with a scripted in-memory Emby transport installed.
fn app_with_mock_emby(http: &MockHttp) -> App {
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

#[test]
fn episode_navigation_drains_a_series_landing_payload() {
    // Task 1.1/1.2: an Episode resolves its owning Series via `series_id`
    // (exactly the two scripted fetches — no ancestors round trip) and the
    // drained event carries the reveal item for the task 2.2 landing arm.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep1","Name":"Pilot","Type":"Episode","SeriesId":"ser1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser1","Name":"The Show","Type":"Series","SortName":"the show"}]}"#,
    );

    app.spawn_navigate_to_item(
        "ep1".into(),
        "Episode".into(),
        vec![(0, "lib-tv".into(), "tvshows".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    match ev {
        LibEvent::NavigateTo {
            lib_idx,
            landing,
            switch_tab,
        } => {
            assert_eq!(lib_idx, 0);
            assert!(switch_tab);
            let NavigateLanding::Series { reveal } = landing else {
                panic!("expected a Series landing");
            };
            assert_eq!(reveal.id, "ser1");
            assert_eq!(reveal.item_type, "Series");
        }
        _ => panic!("expected NavigateTo"),
    }
    assert_eq!(
        http.request_count(),
        2,
        "series_id resolves the reveal without the ancestors round trip"
    );
}

#[test]
fn track_navigation_drains_an_album_landing_payload() {
    // Task 1.1/1.2: an Audio track resolves its album via `album_id` and the
    // drained event carries the reveal album for the task 2.3 landing arm.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(
        200,
        r#"{"Items":[{"Id":"trk1","Name":"Song","Type":"Audio","AlbumId":"alb1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}]}"#,
    );

    app.spawn_navigate_to_item(
        "trk1".into(),
        "Audio".into(),
        vec![(0, "lib-music".into(), "music".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    match ev {
        LibEvent::NavigateTo { landing, .. } => {
            let NavigateLanding::Album { reveal } = landing else {
                panic!("expected an Album landing");
            };
            assert_eq!(reveal.id, "alb1");
            assert_eq!(reveal.item_type, "MusicAlbum");
        }
        _ => panic!("expected NavigateTo"),
    }
}

#[test]
fn movie_navigation_keeps_the_built_chain_stack() {
    // Task 1.2: the Movie/generic arm keeps the pre-change payload — a fully
    // built ancestor-chain nav stack whose deepest cursor rests on the item.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(200, r#"{"Items":[{"Id":"mov1","Name":"Film","Type":"Movie"}]}"#);
    // get_ancestors: physical folder + AggregateFolder only → root-level landing.
    http.respond(200, r#"[{"Id":"folder1","Name":"Movies","Type":"Folder"},{"Id":"root","Name":"root","Type":"AggregateFolder"}]"#);
    // The root browse level fetch: the movie itself, at cursor 0.
    http.respond(
        200,
        r#"{"Items":[{"Id":"other","Name":"Other","Type":"Movie"},{"Id":"mov1","Name":"Film","Type":"Movie"}],"TotalRecordCount":2}"#,
    );

    app.spawn_navigate_to_item(
        "mov1".into(),
        "Movie".into(),
        vec![(0, "lib-movies".into(), "movies".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    match ev {
        LibEvent::NavigateTo {
            lib_idx,
            landing,
            switch_tab,
        } => {
            assert_eq!(lib_idx, 0);
            assert!(switch_tab);
            let NavigateLanding::Chain { nav_stack } = landing else {
                panic!("expected a Chain landing");
            };
            assert_eq!(nav_stack.len(), 1);
            assert_eq!(nav_stack[0].parent_id, "lib-movies");
            assert_eq!(nav_stack[0].resting().cursor(), 1, "cursor on the movie");
        }
        _ => panic!("expected NavigateTo"),
    }
}

#[test]
fn resolve_failure_drains_the_error_event_and_flashes_without_a_tab_change() {
    // Task 4.2: a resolve failure (here: the item fetch fails server-side)
    // drains as `LibEvent::Error`, which flashes and leaves the active tab
    // unchanged.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    app.tab = TabSelection::Home;
    http.respond(404, "");

    app.spawn_navigate_to_item(
        "gone1".into(),
        "Episode".into(),
        vec![(0, "lib-tv".into(), "tvshows".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("error event");
    assert!(matches!(ev, LibEvent::Error(_)), "expected LibEvent::Error");
    // The drain feeds every lib event through `handle_lib_event`; the error
    // arm flashes without touching the tab.
    app.handle_lib_event(ev);
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert!(app.status.contains("Library error"), "flash: {}", app.status);
    assert_eq!(app.status_severity, ToastSeverity::Error);
}
