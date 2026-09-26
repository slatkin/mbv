use super::*;
use crate::app::state::types::events::NavigateLanding;
use std::time::Duration;

#[test]
fn episode_navigation_emits_the_series_landing_without_extra_round_trips() {
    // Task 2.2: an Episode resolves its owning Series via `series_id` (no
    // ancestors round trip in the resolution); the landing carries the full
    // Series reveal item, fetched and type-verified. No chain build, no
    // level fetch.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep1","Name":"Pilot","Type":"Episode","SeriesId":"ser1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser1","Name":"The Show","Type":"Series"}]}"#,
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
            let NavigateLanding::Series { reveal, episode_id } = landing else {
                panic!("expected a Series landing");
            };
            assert_eq!(reveal.id, "ser1");
            assert_eq!(reveal.item_type, "Series");
            // Deep selection (task 6.1, design D6): the Episode reveal rides
            // its own id on the Series landing.
            assert_eq!(episode_id.as_deref(), Some("ep1"));
        }
        _ => panic!("expected NavigateTo"),
    }
    assert_eq!(
        http.request_count(),
        2,
        "item fetch + reveal fetch only: no ancestors, no level fetch"
    );
}

#[test]
fn track_navigation_emits_the_album_landing_with_its_folder_chain() {
    // Task 2.3: an Audio track resolves its album via `album_id`; the
    // landing carries the full album reveal item plus the root→album folder
    // chain the recursive activation walks (nearest→root ancestors, minus
    // the library folder + AggregateFolder, reversed).
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
    // get_ancestors(alb1): nearest→root artist, library folder, AggregateFolder.
    http.respond(
        200,
        r#"[{"Id":"art1","Name":"The Artist","Type":"MusicArtist"},{"Id":"lib-music","Name":"Music","Type":"CollectionFolder"},{"Id":"root","Name":"root","Type":"AggregateFolder"}]"#,
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
            let NavigateLanding::Album {
                reveal,
                ancestors,
                track_id,
            } = landing
            else {
                panic!("expected an Album landing");
            };
            assert_eq!(reveal.id, "alb1");
            assert_eq!(reveal.item_type, "MusicAlbum");
            // Deep selection (task 6.2, design D6): the Audio-track reveal
            // rides its own id on the Album landing.
            assert_eq!(track_id.as_deref(), Some("trk1"));
            assert_eq!(
                ancestors,
                vec![AlbumPathPart {
                    id: "art1".into(),
                    name: "The Artist".into()
                }],
                "root→album folder chain, library folder stripped"
            );
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
    assert!(
        app.status.contains("Library error"),
        "flash: {}",
        app.status
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
}
