#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::library_browse_actions::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, QueueScope, TabSelection,
};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerEvent;
use std::collections::HashMap;
use std::sync::mpsc;
use tuirealm::component::AppComponent;

fn folder(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

#[test]
fn unavailable_album_playback_keeps_the_existing_queue() {
    let mut app = make_app_stub();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Playlist".into(),
    };

    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![track.clone()]);

    assert!(!app.play_album_track("album-1", &track));
    assert_eq!(app.player_tab.total_queue_len(), 1);
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { .. }
    ));
}

#[test]
fn album_playback_routes_with_album_queue_source() {
    let config = crate::config::Config::default();
    let (remote, player_rx, _cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    let observed_source = remote.queue_source.clone();
    let mut app = App::new_remote_with_config(
        mbv_core::api::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    );
    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![track.clone()]);

    assert!(app.play_album_track("album-1", &track));
    assert!(matches!(
        *observed_source.lock().unwrap(),
        crate::config::QueueSource::Album
    ));
}

/// A started remote-backed App (mirrors `album_playback_routes_with_album_queue_source`):
/// `play_album_track`'s Emby-availability gate passes and the resulting queue
/// is observable.
fn remote_playback_app() -> App {
    let config = crate::config::Config::default();
    let (remote, player_rx, _cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
    App::new_remote_with_config(
        mbv_core::api::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        config,
    )
}

/// The shell-owned artist-detail cache key one artist push writes: the
/// destination/generation/artist-ID/revision identity the projection reads.
fn artist_cache_key(
    app: &App,
    artist_id: &str,
) -> crate::app::music_artist_detail::ArtistDetailKey {
    crate::app::music_artist_detail::ArtistDetailKey {
        destination: crate::app::components::library_panel::LibraryKey::Service {
            service: mbv_core::config::ServiceKind::Emby,
            library_id: "lib-music".into(),
            kind: crate::app::components::LibraryKind::Music,
        },
        generation: app.emby_runtime.generation().value(),
        artist_id: artist_id.into(),
        revision: 7,
    }
}

fn queued_track_ids(app: &App) -> Vec<String> {
    let mut ids: Vec<String> = app
        .playback_queue()
        .emby_items()
        .iter()
        .map(|item| item.id.clone())
        .collect();
    ids.sort();
    ids
}

/// Task 6.3 correction: an artist root's Workspace rows are projected from the
/// shell-owned artist-detail cache, which the `ArtistIds` path fills without
/// ever touching `album_tracks_cache`. Activation resolves that cache as its
/// fallback source, so Enter/double-click on an artist row plays the album's
/// tracks from where the row came from.
#[test]
fn artist_workspace_track_plays_from_the_shell_owned_artist_cache() {
    let mut app = remote_playback_app();
    let mut first = make_item("First", "Audio");
    first.id = "artist-track-1".into();
    first.album_id = "album-1".into();
    let mut second = make_item("Second", "Audio");
    second.id = "artist-track-2".into();
    second.album_id = "album-1".into();
    let mut other_album = make_item("Other", "Audio");
    other_album.id = "other-track".into();
    other_album.album_id = "album-2".into();
    app.artist_detail_cache.insert(
        artist_cache_key(&app, "artist-alpha"),
        crate::app::music_artist_detail::ArtistDetailCacheEntry {
            tracks: vec![first, second.clone(), other_album],
            failed: false,
        },
    );
    assert!(
        app.album_tracks_cache.is_empty(),
        "the artist-ID path populates only the artist cache"
    );

    assert!(app.play_album_track("album-1", &second));
    assert_eq!(
        queued_track_ids(&app),
        ["artist-track-1", "artist-track-2"],
        "the row's album group becomes the queue from the artist cache"
    );
}

/// Ordinary album browsing must not change: an `album_tracks_cache` entry
/// still wins over the artist-cache fallback, even when the artist entry
/// carries more tracks for the same album.
#[test]
fn album_track_cache_still_precedes_the_artist_cache_fallback() {
    let mut app = remote_playback_app();
    let mut only = make_item("Only", "Audio");
    only.id = "album-track".into();
    only.album_id = "album-1".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![only.clone()]);
    let mut extra = make_item("Extra", "Audio");
    extra.id = "artist-extra".into();
    extra.album_id = "album-1".into();
    app.artist_detail_cache.insert(
        artist_cache_key(&app, "artist-alpha"),
        crate::app::music_artist_detail::ArtistDetailCacheEntry {
            tracks: vec![only.clone(), extra],
            failed: false,
        },
    );

    assert!(app.play_album_track("album-1", &only));
    assert_eq!(
        queued_track_ids(&app),
        ["album-track"],
        "the album cache's list is the browsing source and takes precedence"
    );

    // A failed or truncated per-album page (an entry that does not hold the
    // activated row) must not hide the artist group the row came from.
    app.album_tracks_cache.insert("album-1".into(), Vec::new());
    assert!(app.play_album_track("album-1", &only));
    assert_eq!(
        queued_track_ids(&app),
        ["album-track", "artist-extra"],
        "the candidate that holds the row wins when the album cache does not"
    );
}

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
        resting: crate::app::types_browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
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
        resting: crate::app::types_browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    };

    app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
        library_id: "music-lib".into(),
        nav_stack: vec![level],
    });

    // The App handler installs the path and persists the position; entering
    // inline track focus for the activated album is the shell's trigger into
    // `MusicWorkspaceComponent` (asserted at the shell boundary in
    // `shell_music_workspace.rs`).
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
