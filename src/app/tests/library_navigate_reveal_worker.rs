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
fn direct_series_navigation_reuses_the_fetched_item() {
    // Task 2.2: a Series reveals itself; the item's own record doubles as
    // the reveal item, so no reveal round trip is paid.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser1","Name":"The Show","Type":"Series"}]}"#,
    );

    app.spawn_navigate_to_item(
        "ser1".into(),
        "Series".into(),
        vec![(0, "lib-tv".into(), "tvshows".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    match ev {
        LibEvent::NavigateTo { landing, .. } => {
            let NavigateLanding::Series { reveal, episode_id } = landing else {
                panic!("expected a Series landing");
            };
            assert_eq!(reveal.id, "ser1");
            // A direct Series reveal is show-level: no deep selection.
            assert_eq!(episode_id, None);
        }
        _ => panic!("expected NavigateTo"),
    }
    assert_eq!(
        http.request_count(),
        1,
        "the item fetch is the reveal fetch"
    );
}

#[test]
fn season_navigation_keeps_the_show_level_landing() {
    // Task 6.1: a Season reveal lands on its Series with default selection
    // (design D6: no deep selection — the reveal was not an Episode).
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(
        200,
        r#"{"Items":[{"Id":"sea1","Name":"Season 1","Type":"Season","SeriesId":"ser1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser1","Name":"The Show","Type":"Series"}]}"#,
    );

    app.spawn_navigate_to_item(
        "sea1".into(),
        "Season".into(),
        vec![(0, "lib-tv".into(), "tvshows".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    match ev {
        LibEvent::NavigateTo { landing, .. } => {
            let NavigateLanding::Series { reveal, episode_id } = landing else {
                panic!("expected a Series landing");
            };
            assert_eq!(reveal.id, "ser1");
            assert_eq!(episode_id, None, "a Season reveal stays show-level");
        }
        _ => panic!("expected NavigateTo"),
    }
}

#[test]
fn series_id_resolving_to_a_non_series_record_is_a_resolve_failure() {
    // Task 2.2 (design D1 rule): an unexpected fetched item type is a
    // resolve failure (`LibEvent::Error`), never a silent misroute.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    app.tab = TabSelection::Home;
    http.respond(
        200,
        r#"{"Items":[{"Id":"ep1","Name":"Pilot","Type":"Episode","SeriesId":"ser1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"ser1","Name":"Not A Show","Type":"Movie"}]}"#,
    );

    app.spawn_navigate_to_item(
        "ep1".into(),
        "Episode".into(),
        vec![(0, "lib-tv".into(), "tvshows".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("error event");
    assert!(matches!(ev, LibEvent::Error(_)), "expected LibEvent::Error");
    app.handle_lib_event(ev);
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert!(
        app.status.contains("Library error"),
        "flash: {}",
        app.status
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
fn track_navigation_without_a_configured_album_path_flashes_and_keeps_the_tab() {
    // Task 6.2 (design D7): a Music item whose album is not reachable through
    // the configured `music.levels` album shape is a configured-path miss,
    // not a silent no-op: the existing library-error flash fires and the
    // active tab is unchanged.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    app.music_levels = vec!["group".into(), "album".into()];
    app.tab = TabSelection::Home;
    http.respond(
        200,
        r#"{"Items":[{"Id":"trk1","Name":"Song","Type":"Audio","AlbumId":"alb1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}]}"#,
    );
    // The configured walk: the library root has no group folder, so `alb1`
    // is unreachable through `music.levels`.
    http.respond(200, r#"{"Items":[],"TotalRecordCount":0}"#);

    app.spawn_navigate_to_item(
        "trk1".into(),
        "Audio".into(),
        vec![(0, "lib-music".into(), "music".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("error event");
    let LibEvent::Error(message) = ev else {
        panic!("a configured-path miss must not land");
    };
    assert!(
        message.contains("configured music levels"),
        "the failure names the configured path: {message}"
    );
    app.handle_lib_event(LibEvent::Error(message));
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert!(app.status.contains("Library error"), "{}", app.status);
}

#[test]
fn album_id_resolving_to_a_non_album_record_is_a_resolve_failure() {
    // Task 2.3 (design D1 rule): a fetched reveal record that is neither a
    // MusicAlbum nor a folder (unmatched album folders parse as plain
    // "Folder" records) is a resolve failure.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    app.tab = TabSelection::Home;
    http.respond(
        200,
        r#"{"Items":[{"Id":"trk1","Name":"Song","Type":"Audio","AlbumId":"alb1"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"Not An Album","Type":"Movie"}]}"#,
    );

    app.spawn_navigate_to_item(
        "trk1".into(),
        "Audio".into(),
        vec![(0, "lib-music".into(), "music".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("error event");
    assert!(matches!(ev, LibEvent::Error(_)), "expected LibEvent::Error");
    app.handle_lib_event(ev);
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
}

#[test]
fn movie_navigation_keeps_the_built_chain_stack() {
    // Task 1.2: the Movie/generic arm keeps the pre-change payload — a fully
    // built ancestor-chain nav stack whose deepest cursor rests on the item.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let app = app_with_mock_emby(&http);
    http.respond(
        200,
        r#"{"Items":[{"Id":"mov1","Name":"Film","Type":"Movie"}]}"#,
    );
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
    assert!(
        app.status.contains("Library error"),
        "flash: {}",
        app.status
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
}

#[test]
fn artist_navigation_is_a_resolve_failure_that_leaves_the_view_unchanged() {
    // D1: the artist kind does not land -- it has no single owning album and
    // a plain artist chain does not render on a grouped Music surface. The
    // worker sends the resolve-failure flash (the pre-U2 semantics) and the
    // active view is untouched: no landing, no tab switch, no saved position.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::Home;
    http.respond(
        200,
        r#"{"Items":[{"Id":"art1","Name":"The Artist","Type":"MusicArtist"}]}"#,
    );
    // get_ancestors(art1): the fallback round trip still runs before the
    // failure is reported; library folder + AggregateFolder only.
    http.respond(
        200,
        r#"[{"Id":"lib-music","Name":"Music","Type":"CollectionFolder"},{"Id":"root","Name":"root","Type":"AggregateFolder"}]"#,
    );

    app.spawn_navigate_to_item(
        "art1".into(),
        "MusicArtist".into(),
        vec![(0, "lib-music".into(), "music".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("resolve-failure event");
    let LibEvent::Error(message) = ev else {
        panic!("an artist must not land");
    };
    assert!(
        message.contains("artist's album"),
        "the failure names the unsatisfiable owning album: {message}"
    );

    app.handle_lib_event(LibEvent::Error(message));
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert_eq!(app.status_severity, ToastSeverity::Error);
    assert!(app.status.contains("Library error"), "{}", app.status);
    assert!(
        app.libs[0].nav_stack.is_empty(),
        "no browse level is materialized for an artist"
    );
    assert!(
        app.saved_library_position(0).is_none(),
        "a failed navigation saves nothing"
    );
}

#[test]
fn artist_navigation_on_a_deleted_item_fails_at_the_fetch() {
    // D1: the artist kind fails before any landing kind is chosen, so a
    // missing record is the same flash and the ancestors round trip is never
    // paid -- one request, no landing, no tab switch.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    app.tab = TabSelection::Home;
    http.respond(200, r#"{"Items":[]}"#);

    app.spawn_navigate_to_item(
        "art1".into(),
        "MusicArtist".into(),
        vec![(0, "lib-music".into(), "music".into())],
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("error event");
    assert!(matches!(ev, LibEvent::Error(_)), "expected LibEvent::Error");
    app.handle_lib_event(ev);
    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert_eq!(
        http.request_count(),
        1,
        "the deleted item fails at the fetch, before the ancestors round trip"
    );
}
