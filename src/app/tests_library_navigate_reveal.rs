// Unit tests for change `per-destination-item-navigation` tasks 1.1/1.2/4.2:
// the D1 reveal-item table, the `LibEvent::NavigateTo` landing payload, and
// the drained resolve-failure error event. Mocked Emby boundary only
// (`MockHttp`), per the AGENTS.md mocks-only policy.

use super::*;
use crate::app::tests::{install_test_emby, make_app_stub, make_item};
use crate::app::library_browse_actions::{RevealTarget, resolve_reveal_target};
use crate::app::types_browse::BrowseResting;
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

/// A TV library tab with an already-loaded root series level.
fn app_with_loaded_tv_library() -> App {
    let mut app = make_app_stub();
    let mut library = make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.collection_type = "tvshows".into();
    app.libs.push(LibraryTab::new(library));
    app.libs[0].library_total = Some(2);
    let mut other = make_item("Other Show", "Series");
    other.id = "ser0".into();
    let mut show = make_item("The Show", "Series");
    show.id = "ser1".into();
    app.libs[0].nav_stack.push(BrowseLevel {
        parent_id: "lib-tv".into(),
        title: "TV".into(),
        items: vec![other, show],
        total_count: 2,
        resting: BrowseResting::new(0, 0),
        item_types: Some("Series".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    app
}

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
            let NavigateLanding::Series { reveal } = landing else {
                panic!("expected a Series landing");
            };
            assert_eq!(reveal.id, "ser1");
        }
        _ => panic!("expected NavigateTo"),
    }
    assert_eq!(http.request_count(), 1, "the item fetch is the reveal fetch");
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
    assert!(app.status.contains("Library error"), "flash: {}", app.status);
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
            let NavigateLanding::Album { reveal, ancestors } = landing else {
                panic!("expected an Album landing");
            };
            assert_eq!(reveal.id, "alb1");
            assert_eq!(reveal.item_type, "MusicAlbum");
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
fn series_landing_applies_the_searched_series_activation_on_the_root_level() {
    // Task 2.2: the App-side Series arm lands the show through
    // `activate_searched_series` on the target library's current root level
    // (letter pill included), then replaces the saved Library position with
    // the landed state before the tab switch (D4). Root-level-only: no new
    // browse level is pushed.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_loaded_tv_library();
    app.tab = TabSelection::Home;

    let mut show = make_item("The Show", "Series");
    show.id = "ser1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(show),
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(
        app.libs[0].nav_stack.len(),
        1,
        "root-level-only landing: no level below the series list"
    );
    let level = &app.libs[0].nav_stack[0];
    assert!(level.letter_filter.is_some(), "letter pill applied");
    let cursor = level.resting().cursor();
    assert_eq!(
        level.items[cursor].id, "ser1",
        "cursor rests on the navigated show within the filtered corpus"
    );
    assert!(!level.loading);
    // The landed state is the saved position (D4).
    let saved = app
        .saved_library_position(0)
        .expect("landing replaces the saved position");
    assert_eq!(saved.levels[0].focused_item_id.as_deref(), Some("ser1"));
    assert_eq!(saved.levels[0].cursor_index, cursor);
}

#[test]
fn series_landing_miss_flashes_and_leaves_the_active_tab_unchanged() {
    // Task 2.2: a miss (series absent from the level corpus) flashes the
    // library-error path and leaves the active tab unchanged, mirroring
    // task 4.2's failure handling.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_loaded_tv_library();
    app.tab = TabSelection::Home;
    let (stack_len, first_parent, first_cursor) = (
        app.libs[0].nav_stack.len(),
        app.libs[0].nav_stack[0].parent_id.clone(),
        app.libs[0].nav_stack[0].resting().cursor(),
    );

    let mut absent = make_item("Missing Show", "Series");
    absent.id = "ser-absent".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Series {
            reveal: Box::new(absent),
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert_eq!(app.libs[0].nav_stack.len(), stack_len);
    assert_eq!(app.libs[0].nav_stack[0].parent_id, first_parent);
    assert_eq!(
        app.libs[0].nav_stack[0].resting().cursor(),
        first_cursor,
        "the root level is untouched by a miss"
    );
    assert!(app.status.contains("Could not land on"), "flash: {}", app.status);
    assert_eq!(app.status_severity, ToastSeverity::Error);
}

#[test]
fn album_landing_flat_library_replaces_the_stack_on_the_activated_drain() {
    // Task 2.3, flat shape: an album directly under the library root (empty
    // folder chain) lands as a single root level whose cursor rests on the
    // album; the tab switch is deferred to the `RecursiveAlbumActivated`
    // drain, after the landing replaced the saved position (D4).
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::Home;

    // The activation thread's whole-library fetch: the album present at root.
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb0","Name":"Other Album","Type":"MusicAlbum"},{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":2}"#,
    );

    let mut album = make_item("The Album", "MusicAlbum");
    album.id = "alb1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Album {
            reveal: Box::new(album),
            ancestors: vec![],
        },
        switch_tab: true,
    });
    // The landing is async: no tab change until the activation drains.
    assert_eq!(app.tab, TabSelection::Home, "switch deferred to the drain");

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    assert!(
        matches!(ev, LibEvent::RecursiveAlbumActivated { .. }),
        "expected RecursiveAlbumActivated"
    );
    app.handle_lib_event(ev);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(app.libs[0].nav_stack.len(), 1, "flat: single root level");
    let level = &app.libs[0].nav_stack[0];
    assert_eq!(level.parent_id, "lib-music");
    let cursor = level.resting().cursor();
    assert_eq!(level.items[cursor].id, "alb1", "cursor on the album");
    // The landed state is the saved position (D4).
    let saved = app
        .saved_library_position(0)
        .expect("landing replaces the saved position");
    assert_eq!(saved.levels[0].focused_item_id.as_deref(), Some("alb1"));
}

#[test]
fn album_landing_grouped_library_walks_the_folder_chain() {
    // Task 2.3, grouped shape: an album nested under an artist folder lands
    // as the recursive activation's two-level stack — the group root level
    // with the cursor on the artist, then the artist level with the cursor
    // on the album.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::Home;

    // Activation fetches: the library root (artist present), then the
    // artist's children (album present).
    http.respond(
        200,
        r#"{"Items":[{"Id":"art0","Name":"Other Artist","Type":"MusicArtist"},{"Id":"art1","Name":"The Artist","Type":"MusicArtist"}],"TotalRecordCount":2}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb0","Name":"Other Album","Type":"MusicAlbum"},{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":2}"#,
    );

    let mut album = make_item("The Album", "MusicAlbum");
    album.id = "alb1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Album {
            reveal: Box::new(album),
            ancestors: vec![AlbumPathPart {
                id: "art1".into(),
                name: "The Artist".into(),
            }],
        },
        switch_tab: true,
    });

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    app.handle_lib_event(ev);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(app.libs[0].nav_stack.len(), 2, "grouped: root + artist level");
    let root = &app.libs[0].nav_stack[0];
    assert_eq!(root.parent_id, "lib-music");
    assert_eq!(root.items[root.resting().cursor()].id, "art1");
    let artist = &app.libs[0].nav_stack[1];
    assert_eq!(artist.parent_id, "art1");
    assert_eq!(artist.items[artist.resting().cursor()].id, "alb1");
}

#[test]
fn failed_album_activation_flashes_and_never_switches_tabs_later() {
    // Task 2.3: a failed activation spawn flashes the library-error path;
    // no deferred tab switch is armed, so no later album activation can
    // fire it.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::Home;

    let mut album = make_item("The Album", "MusicAlbum");
    album.id = "alb1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Album {
            reveal: Box::new(album),
            ancestors: vec![],
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home);
    assert_eq!(
        app.pending_navigate_tab_switch, None,
        "no deferred switch armed on a failed spawn"
    );
    assert!(app.status.contains("Could not start"), "flash: {}", app.status);
    assert_eq!(app.status_severity, ToastSeverity::Error);
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
