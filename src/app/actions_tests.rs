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

#[test]
fn client_playback_sequence_expands_before_direct_daemon_dispatch() {
    let item = make_item("Episode 1", "Episode");
    let mut episode_two = make_item("Episode 2", "Episode");
    episode_two.id = "episode-2".into();
    let mut episode_three = make_item("Episode 3", "Episode");
    episode_three.id = "episode-3".into();

    let sequence = client_playback_sequence(
        item,
        true,
        vec![
            make_item("Episode 1", "Episode"),
            episode_two,
            episode_three,
        ],
    );

    let (remote, _events, commands) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    remote.play_queue(
        sequence.clone(),
        0,
        crate::config::QueueSource::Series,
        std::sync::Arc::new(mbv_core::api::EmbyClient::new(
            crate::config::Config::default(),
        )),
        100,
    );

    let command = commands.recv().unwrap();
    match command {
        mbv_core::ctrl::CtrlCmd::PlaybackIntent(intent) => match intent.action {
            mbv_core::ctrl::PlaybackIntentAction::Play { item_ids, .. } => {
                assert_eq!(
                    item_ids,
                    sequence
                        .iter()
                        .map(|item| item.id.clone())
                        .collect::<Vec<_>>()
                );
            }
            _ => panic!("unexpected playback action"),
        },
        _ => panic!("unexpected command"),
    }
}

#[test]
fn client_playback_sequence_does_not_append_when_disabled() {
    let item = make_item("Episode 1", "Episode");
    let mut episode_two = make_item("Episode 2", "Episode");
    episode_two.id = "episode-2".into();

    let sequence = client_playback_sequence(item.clone(), false, vec![item.clone(), episode_two]);

    assert_eq!(
        sequence
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id"]
    );
}

#[test]
fn client_expansion_is_target_independent() {
    let mut first = make_item("Episode 1", "Episode");
    first.id = "episode-1".into();
    let mut second = make_item("Episode 2", "Episode");
    second.id = "episode-2".into();
    let following = vec![first.clone(), second.clone()];

    let local_sequence = client_playback_sequence(first.clone(), true, following.clone());
    let direct_sequence = client_playback_sequence(first, true, following);

    assert_eq!(
        local_sequence
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        direct_sequence
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>()
    );
}

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
        search: None,
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
fn recursive_album_search_matches_ancestor_labels() {
    let mut app = recursive_music_app();
    let target = album("album-1", "Needle Record");
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Ready(vec![AlbumSearchEntry {
            album: target,
            ancestors: vec![AlbumPathPart {
                id: "group-a".into(),
                name: "Deep Group".into(),
            }],
            display_label: "Deep Group / Needle Record".into(),
            search_text: "Deep Group / Needle Record".into(),
        }]),
    );

    assert!(app.open_recursive_album_search(0));
    app.libs[0].search.as_mut().unwrap().query = "deep grp".into();
    app.update_lib_search(0);

    assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
    assert_eq!(
        app.recursive_album_display_item(0, 0, album("album-1", "Needle Record"))
            .name,
        "Deep Group / Needle Record"
    );

    app.libs[0].search.as_mut().unwrap().query = "needle rec".into();
    app.update_lib_search(0);
    assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
}

#[test]
fn album_only_music_keeps_visible_list_search() {
    let mut app = recursive_music_app();
    app.music_levels = vec!["album".into()];
    app.libs[0].search = Some(super::super::LibSearch {
        query: "visible rec".into(),
        items: vec![album("album-1", "Visible Record")],
        results: Vec::new(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    assert!(!app.open_recursive_album_search(0));
    app.update_lib_search(0);

    assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
}

#[test]
fn album_index_completion_updates_the_open_current_query() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );
    assert!(app.open_recursive_album_search(0));
    app.libs[0].search.as_mut().unwrap().query = "remote group".into();

    app.handle_lib_event(LibEvent::AlbumIndexBuilt {
        library_id: "music-lib".into(),
        result: Ok(vec![AlbumSearchEntry {
            album: album("album-1", "Record"),
            ancestors: vec![AlbumPathPart {
                id: "group-a".into(),
                name: "Remote Group".into(),
            }],
            display_label: "Remote Group / Record".into(),
            search_text: "Remote Group / Record".into(),
        }]),
    });

    let search = app.libs[0].search.as_ref().unwrap();
    assert!(!search.loading);
    assert_eq!(search.query, "remote group");
    assert_eq!(search.results, vec![0]);
}

#[test]
fn failed_album_index_becomes_unavailable_and_clears_search_loading() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );
    assert!(app.open_recursive_album_search(0));

    app.handle_lib_event(LibEvent::AlbumIndexBuilt {
        library_id: "music-lib".into(),
        result: Err("index failed".into()),
    });

    assert!(matches!(
        app.album_indexes.get("music-lib"),
        Some(AlbumIndexState::Unavailable)
    ));
    assert!(!app.libs[0].search.as_ref().unwrap().loading);
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
fn recursive_activation_keeps_panel_focus_and_enters_inline_tracks() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = recursive_music_app();
    app.library_tab = 1;
    app.panel_focus = PanelFocus::Library;
    app.libs[0].nav_stack.push(BrowseLevel {
        parent_id: "group-a".into(),
        title: "Group A".into(),
        items: vec![folder("artist-a", "Artist A")],
        total_count: 1,
        cursor: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        scroll: 0,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    });
    let default_position = app.libs[0].library_position_snapshot();
    app.library_position_state
        .libraries
        .insert("music-lib".into(), default_position.clone());
    app.libs[0].search = Some(super::super::LibSearch {
        query: String::new(),
        items: Vec::new(),
        results: Vec::new(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });
    let level = BrowseLevel {
        parent_id: "artist-c".into(),
        title: "Artist C".into(),
        items: vec![album("album-1", "Record")],
        total_count: 1,
        cursor: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        scroll: 0,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    };

    app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
        library_id: "music-lib".into(),
        nav_stack: vec![level],
    });

    assert!(app.libs[0].search.is_none());
    assert_eq!(app.libs[0].album_track_focus, Some(0));
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
