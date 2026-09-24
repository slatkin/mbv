#![allow(dead_code, unused_imports)]

use super::*;

fn album(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "MusicAlbum");
    item.id = id.into();
    item.is_folder = true;
    item.media_type = "Audio".into();
    item
}

fn recursive_music_app() -> App {
    let mut app = make_app_stub();
    app.music_levels = vec!["group".into(), "artist".into(), "album".into()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "music-lib".into();
    library.collection_type = "music".into();
    library.is_folder = true;
    app.libs.push(LibraryTab::new(library));
    app
}
#[test]
fn album_index_eligibility_requires_grouped_music_ending_in_album() {
    assert!(recursive_album_search_eligible(
        "music",
        &["group".into(), "album".into()]
    ));
    assert!(recursive_album_search_eligible(
        "music",
        &["group".into(), "artist".into(), "album".into()]
    ));
    assert!(!recursive_album_search_eligible("music", &[]));
    assert!(!recursive_album_search_eligible("music", &["album".into()]));
    assert!(!recursive_album_search_eligible(
        "music",
        &["group".into(), "artist".into()]
    ));
    assert!(!recursive_album_search_eligible(
        "movies",
        &["group".into(), "album".into()]
    ));
}

#[test]
fn album_index_traverses_deep_branches_pages_and_ignores_non_albums() {
    let mut tree = HashMap::new();
    tree.insert(
        "music-lib".to_string(),
        vec![folder("group-a", "A"), folder("group-b", "B")],
    );
    tree.insert(
        "group-a".to_string(),
        vec![
            folder("artist-empty", "Empty"),
            folder("artist-a", "Artist A"),
        ],
    );
    tree.insert("artist-empty".to_string(), Vec::new());
    let mut many_albums: Vec<EmbyItem> = (0..201)
        .map(|index| album(&format!("album-a-{index}"), &format!("Record {index}")))
        .collect();
    many_albums.push(make_item("Not an album", "Audio"));
    tree.insert("artist-a".to_string(), many_albums);
    tree.insert("group-b".to_string(), vec![folder("artist-b", "Artist B")]);
    tree.insert("artist-b".to_string(), vec![album("album-b", "Record 0")]);
    let mut calls = Vec::new();
    let mut fetch = |parent: &str, start: usize, limit: usize| {
        calls.push((parent.to_string(), start));
        let all = tree.get(parent).cloned().unwrap_or_default();
        let page = all.iter().skip(start).take(limit).cloned().collect();
        Ok((page, all.len()))
    };

    let entries = build_album_index_with(
        "music-lib",
        &["group".into(), "artist".into(), "album".into()],
        &mut fetch,
    )
    .unwrap();

    assert_eq!(entries.len(), 202);
    assert_eq!(
        entries.last().unwrap().display_label,
        "B / Artist B / Record 0"
    );
    assert_eq!(
        entries.last().unwrap().ancestors,
        vec![
            AlbumPathPart {
                id: "group-b".into(),
                name: "B".into()
            },
            AlbumPathPart {
                id: "artist-b".into(),
                name: "Artist B".into()
            }
        ]
    );
    assert!(calls.contains(&("artist-a".into(), 200)));
    assert!(entries
        .iter()
        .all(|entry| entry.album.item_type == "MusicAlbum"));
}

#[test]
fn failed_album_index_becomes_unavailable() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );
    app.handle_lib_event(LibEvent::AlbumIndexBuilt {
        library_id: "music-lib".into(),
        result: Err("index failed".into()),
    });

    assert!(matches!(
        app.album_indexes.get("music-lib"),
        Some(AlbumIndexState::Unavailable)
    ));
    assert!(app.status.contains("index failed"));
}

#[test]
fn refresh_while_album_index_loads_coalesces_one_replacement() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );

    app.start_album_index(0, true);
    app.start_album_index(0, true);

    assert!(matches!(
        app.album_indexes.get("music-lib"),
        Some(AlbumIndexState::Loading {
            rebuild_pending: true
        })
    ));
}

#[test]
fn recursive_activation_keeps_panel_focus_and_installs_path() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = recursive_music_app();
    app.tab = TabSelection::EmbyLibrary(0);
    app.panel_focus = PanelFocus::Library;
    app.libs[0].nav_stack.push(BrowseLevel {
        fetched_rows: 0,
        parent_id: "group-a".into(),
        title: "Group A".into(),
        items: vec![folder("artist-a", "Artist A")],
        total_count: 1,
        resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    });
    let default_position = app.libs[0].library_position_snapshot();
    app.library_position_state
        .libraries
        .insert("music-lib".into(), default_position.clone());
    let level = BrowseLevel {
        fetched_rows: 0,
        parent_id: "artist-c".into(),
        title: "Artist C".into(),
        items: vec![album("album-1", "Record")],
        total_count: 1,
        resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    };

    app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
        library_id: "music-lib".into(),
        nav_stack: vec![level],
    });

    // The App handler installs the path and persists the position; entering
    // inline track focus for the activated album is the shell's trigger into
    // `MusicWorkspaceComponent` (asserted at the shell boundary in
    // `shell/music_workspace/mod.rs`).
    assert_eq!(app.libs[0].nav_stack.last().unwrap().parent_id, "artist-c");
    let position = app
        .library_position_state
        .libraries
        .get("music-lib")
        .unwrap();
    assert_eq!(
        position.levels.last().map(|level| level.parent_id.as_str()),
        Some("artist-c")
    );
}
