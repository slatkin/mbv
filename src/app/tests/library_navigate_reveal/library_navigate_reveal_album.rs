use super::*;
use crate::app::state::types::events::NavigateLanding;
use std::time::Duration;

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
            track_id: None,
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
            track_id: None,
        },
        switch_tab: true,
    });

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    app.handle_lib_event(ev);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert_eq!(
        app.libs[0].nav_stack.len(),
        2,
        "grouped: root + artist level"
    );
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
            track_id: None,
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home);
    assert_eq!(
        app.pending_navigate_tab_switch, None,
        "no deferred switch armed on a failed spawn"
    );
    assert!(
        app.status.contains("Could not start"),
        "flash: {}",
        app.status
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
}

#[test]
fn album_landing_with_a_stale_library_index_flashes_instead_of_panicking() {
    // U2 correction (finding 2): a catalog change between resolution and the
    // drain can leave `lib_idx` out of range; the Album arm must flash (the
    // activation returns false) rather than index `self.libs` unchecked.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = app_with_loaded_tv_library();
    app.tab = TabSelection::Home;

    let mut album = make_item("The Album", "MusicAlbum");
    album.id = "alb1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 7,
        landing: NavigateLanding::Album {
            reveal: Box::new(album),
            ancestors: Vec::new(),
            track_id: None,
        },
        switch_tab: true,
    });

    assert_eq!(app.tab, TabSelection::Home, "active tab unchanged");
    assert_eq!(app.pending_navigate_tab_switch, None);
    assert!(
        app.status.contains("Could not start"),
        "flash: {}",
        app.status
    );
    assert_eq!(app.status_severity, ToastSeverity::Error);
}

#[test]
fn recursive_album_activation_rests_the_cursor_on_a_non_folder_album() {
    // U2 correction (finding 4c): the activation walks raw parent listings
    // (`fetch_all_album_index_items` applies no `is_folder` filter), so a
    // MusicAlbum record that is not a folder still lands with the cursor on
    // it -- the is_folder filter belongs to the album INDEX walk only.
    let _guard = crate::config::TestStateDirGuard::new();
    let http = MockHttp::new();
    let mut app = app_with_mock_emby(&http);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.tab = TabSelection::Home;

    // The album's parent listing: a folder record first, the non-folder
    // MusicAlbum second; the cursor must be the album's own position.
    http.respond(
        200,
        r#"{"Items":[{"Id":"fold1","Name":"Unmatched Folder","Type":"Folder","IsFolder":true},{"Id":"alb1","Name":"The Album","Type":"MusicAlbum","IsFolder":false}],"TotalRecordCount":2}"#,
    );

    let mut album = make_item("The Album", "MusicAlbum");
    album.id = "alb1".into();
    album.is_folder = false;
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Album {
            reveal: Box::new(album),
            ancestors: Vec::new(),
            track_id: None,
        },
        switch_tab: true,
    });

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    app.handle_lib_event(ev);

    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    let level = &app.libs[0].nav_stack[0];
    assert_eq!(level.items.len(), 2);
    assert_eq!(
        level.resting().cursor(),
        1,
        "cursor on the non-folder album, not clamped to the folder at index 0"
    );
    assert_eq!(level.items[1].id, "alb1");
}

#[test]
fn manual_tab_change_drops_the_deferred_album_switch_and_never_yanks_back() {
    // U2 correction (finding 4b): a manual tab change clears the deferred
    // navigation, so the later activation drain cannot switch back.
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
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );

    let mut album = make_item("The Album", "MusicAlbum");
    album.id = "alb1".into();
    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: NavigateLanding::Album {
            reveal: Box::new(album),
            ancestors: Vec::new(),
            track_id: None,
        },
        switch_tab: true,
    });
    assert_eq!(app.pending_navigate_tab_switch, Some(0));

    // The user moves on before the activation drains.
    app.set_library_tab(0);
    assert_eq!(
        app.pending_navigate_tab_switch, None,
        "manual change clears"
    );

    let ev = app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    app.handle_lib_event(ev);

    assert_eq!(
        app.tab,
        TabSelection::Home,
        "the drain must not yank the tab after the user moved on"
    );
    assert_eq!(
        app.libs[0].nav_stack[0].items[app.libs[0].nav_stack[0].resting().cursor()].id,
        "alb1",
        "the landing itself still applied"
    );
}

#[test]
fn foreign_library_album_drain_leaves_the_deferred_switch_armed() {
    // U2 correction (finding 4a): the deferred switch belongs to one
    // library; another library's activation drain must leave it armed.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    for (id, name) in [("lib-music-a", "Music A"), ("lib-music-b", "Music B")] {
        let mut library = make_item(name, "CollectionFolder");
        library.id = id.into();
        library.collection_type = "music".into();
        app.libs.push(LibraryTab::new(library));
    }
    app.tab = TabSelection::Home;
    app.pending_navigate_tab_switch = Some(0);

    app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
        library_id: "lib-music-b".into(),
        nav_stack: Vec::new(),
    });
    assert_eq!(
        app.pending_navigate_tab_switch,
        Some(0),
        "another library's activation leaves the switch armed"
    );
    assert_eq!(app.tab, TabSelection::Home, "no switch on a foreign drain");

    app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
        library_id: "lib-music-a".into(),
        nav_stack: Vec::new(),
    });
    assert_eq!(app.pending_navigate_tab_switch, None, "consumed on its own");
    assert_eq!(
        app.tab,
        TabSelection::EmbyLibrary(0),
        "switched to its library"
    );
}
