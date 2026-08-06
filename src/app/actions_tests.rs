#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::library_browse_actions::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, QueueScope,
};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerEvent;
use std::collections::HashMap;
use std::sync::mpsc;

fn folder(id: &str, name: &str) -> MediaItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

fn album(id: &str, name: &str) -> MediaItem {
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
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
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
    let mut many_albums: Vec<MediaItem> = (0..201)
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
    assert!(calls.contains(&("artist-a".into(), 200)));
    assert!(entries
        .iter()
        .all(|entry| entry.album.item_type == "MusicAlbum"));
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
