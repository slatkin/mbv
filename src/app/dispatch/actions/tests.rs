#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::components::list::tree_browser::TreeOperation;
use crate::app::components::msg::{Msg, MusicArtistTarget, ShellRequest};
use crate::app::dispatch::library::browse::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::render::make_music_group_app_with_second_album;
use crate::app::shell::Model;
use crate::app::tests::{
    confirm_replace_queue, install_test_emby, make_app_stub, make_item, make_items,
};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, PanelFocus, QueueScope, TabSelection,
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
    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(Vec::new(), 0);
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
    assert!(cmd_rx.try_iter().any(|command| matches!(
        command,
        mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace {
            source: crate::config::QueueSource::Album,
            ..
        }
    )));
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
) -> crate::app::state::music_artist_detail::ArtistDetailKey {
    crate::app::state::music_artist_detail::ArtistDetailKey {
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

fn artist_dispatch_model() -> (Model, MusicArtistTarget) {
    let fixture = make_music_group_app_with_second_album();
    let mut app = remote_playback_app();
    app.tab = fixture.tab;
    app.libs = fixture.libs;
    app.music_levels = fixture.music_levels;
    {
        let level = app.libs[0].nav_stack.last_mut().expect("music albums");
        for item in &mut level.items {
            item.artist_items = vec![mbv_core::api::EmbyArtistRef {
                name: "Alpha".into(),
                id: "artist-alpha".into(),
            }];
        }
        let mut catalog = crate::app::state::music_grouping::build_grouped_album_catalog(
            &level.items,
            &Default::default(),
        );
        catalog.revision = 7;
        catalog.parent_id = level.parent_id.clone();
        level.music_grouping = Some(crate::app::state::music_grouping::MusicGroupingState {
            revision: 7,
            candidate: None,
            settled: Some(catalog),
        });
    }

    let mut model = Model::new(app);
    model.app.panel_focus = PanelFocus::Library;
    model.sync_mounted_surfaces();
    model
        .test_music_owner_mut()
        .browser
        .apply(TreeOperation::First);
    let target = model
        .test_music_owner()
        .artist_detail_target()
        .expect("artist target");
    (model, target)
}

/// The direct shell arm accepts a revision-only artist rebind, resolves the
/// stable track identity from the projected detail, and sends the complete
/// flattened track order to the playback executor. A miss must not mutate it.
#[test]
fn artist_track_dispatch_resolves_revision_rebind_and_preserves_queue_on_miss() {
    let (mut model, target) = artist_dispatch_model();
    let mut first = make_item("First", "Audio");
    first.id = "artist-track-1".into();
    first.album_id = "album-1".into();
    first.media_type = "Audio".into();
    first.index_number = 1;
    let mut selected = make_item("Selected", "Audio");
    selected.id = "artist-track-2".into();
    selected.album_id = "album-1".into();
    selected.media_type = "Audio".into();
    selected.index_number = 2;
    let mut last = make_item("Last", "Audio");
    last.id = "artist-track-3".into();
    last.album_id = "album-1".into();
    last.media_type = "Audio".into();
    last.index_number = 3;
    model.app.artist_detail_cache.insert(
        artist_cache_key(&model.app, "artist-alpha"),
        crate::app::state::music_artist_detail::ArtistDetailCacheEntry {
            tracks: vec![last, first, selected],
            failed: false,
        },
    );
    model.push_music_workspace_content();

    let mut rebound = target;
    rebound.revision += 1;
    let (mut music_resize, mut tv_resize) = (false, false);
    model.handle_terminal_message(
        Msg::Shell(ShellRequest::MusicArtistTrackActivate {
            target: rebound.clone(),
            track_id: "artist-track-2".into(),
        }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        model
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["artist-track-1", "artist-track-2", "artist-track-3"],
        "the dispatch arm sends the full flattened artist track order"
    );
    assert_eq!(model.app.playback_queue().queue_cursor, 1);

    let queue_before_miss = queued_track_ids(&model.app);
    let cursor_before_miss = model.app.playback_queue().queue_cursor;
    model.handle_terminal_message(
        Msg::Shell(ShellRequest::MusicArtistTrackActivate {
            target: rebound,
            track_id: "missing-track".into(),
        }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(queued_track_ids(&model.app), queue_before_miss);
    assert_eq!(model.app.playback_queue().queue_cursor, cursor_before_miss);
    assert!(model.app.status.contains("Library error"));
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
        crate::app::state::music_artist_detail::ArtistDetailCacheEntry {
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

#[test]
fn artist_playback_keeps_preceding_tracks_queued_and_starts_at_selected_index() {
    let mut app = remote_playback_app();
    let mut first = make_item("First", "Audio");
    first.id = "artist-track-1".into();
    let mut selected = make_item("Selected", "Audio");
    selected.id = "artist-track-2".into();
    let mut last = make_item("Last", "Audio");
    last.id = "artist-track-3".into();

    assert!(app.play_artist_tracks(vec![first, selected, last], 1));
    assert_eq!(
        app.playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["artist-track-1", "artist-track-2", "artist-track-3"]
    );
    assert_eq!(app.playback_queue().queue_cursor, 1);
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
        crate::app::state::music_artist_detail::ArtistDetailCacheEntry {
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
    // The second activation targets a populated queue, so it goes through the
    // replacement gate before the queue changes.
    confirm_replace_queue(&mut app);
    assert_eq!(
        queued_track_ids(&app),
        ["album-track", "artist-extra"],
        "the candidate that holds the row wins when the album cache does not"
    );
}

/// Design D6: the one grouped-track resolver used by the tree's Enter chord
/// and its track double-click. With autoload enabled the queue is the album's
/// cached playable tracks in disc/track order, starting at the selected track
/// with the preceding tracks retained, and the complete
/// `PendingQueueAction::PlayItems` reaches the existing executor.
#[test]
fn grouped_track_with_autoload_queues_the_album_in_disc_order_from_the_selected_track() {
    let mut app = remote_playback_app();
    app.config.lock().unwrap().autoload = true;
    let tracks = [("track-3", 3), ("track-1", 1), ("track-2", 2)]
        .into_iter()
        .map(|(id, number)| {
            let mut track = make_item(id, "Audio");
            track.id = id.into();
            track.album_id = "album-1".into();
            track.media_type = "Audio".into();
            track.index_number = number;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);

    match app
        .grouped_track_play_action("album-1", "track-2")
        .expect("the cached album resolves the selected track")
    {
        PendingQueueAction::PlayItems {
            items,
            start_idx,
            source,
            autostart,
        } => {
            assert_eq!(
                items
                    .iter()
                    .map(|item| item.id.as_str())
                    .collect::<Vec<_>>(),
                ["track-1", "track-2", "track-3"],
                "the cached album enters in disc/track order, not cache order"
            );
            assert_eq!(start_idx, 1, "the selected track is the start index");
            assert!(autostart, "a track activation starts playback");
            assert!(matches!(source, crate::config::QueueSource::Album));
        }
        PendingQueueAction::ClearQueue => panic!("a track activation never clears the queue"),
    }

    assert!(app.play_grouped_track("album-1", "track-2"));
    assert_eq!(
        app.playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["track-1", "track-2", "track-3"],
        "the resolved album replaces the target queue"
    );
    assert_eq!(
        app.playback_queue().queue_cursor,
        1,
        "playback starts at the selected track with earlier tracks still queued"
    );
}

/// The same resolver honours the disabled autoload policy: only the selected
/// Audio item enters the replacement queue.
#[test]
fn grouped_track_without_autoload_queues_only_the_selected_track() {
    let mut app = remote_playback_app();
    app.config.lock().unwrap().autoload = false;
    let tracks = ["track-1", "track-2", "track-3"]
        .into_iter()
        .enumerate()
        .map(|(index, id)| {
            let mut track = make_item(id, "Audio");
            track.id = id.into();
            track.album_id = "album-1".into();
            track.media_type = "Audio".into();
            track.index_number = index as i64 + 1;
            track
        })
        .collect::<Vec<_>>();
    app.album_tracks_cache.insert("album-1".into(), tracks);

    assert!(app.play_grouped_track("album-1", "track-2"));
    assert_eq!(
        app.playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["track-2"],
        "autoload off resolves only the selected track"
    );
    assert_eq!(app.playback_queue().queue_cursor, 0);
}

/// Resolution failure flashes the existing library error and does not replace
/// a queue: the group's cached album carries no such track identity.
#[test]
fn grouped_track_resolution_failure_keeps_the_queue_and_reports_library_error() {
    let mut app = remote_playback_app();
    app.config.lock().unwrap().autoload = true;
    let mut cached = make_item("Cached", "Audio");
    cached.id = "cached-track".into();
    cached.album_id = "album-1".into();
    cached.media_type = "Audio".into();
    cached.index_number = 1;
    app.album_tracks_cache
        .insert("album-1".into(), vec![cached]);
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.remote_player_tab
        .as_mut()
        .expect("the direct remote fixture keeps a target queue")
        .set_items(vec![existing], 0);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Playlist".into(),
    };

    assert!(!app.play_grouped_track("album-1", "missing-track"));
    assert_eq!(
        queued_track_ids(&app),
        ["existing"],
        "a failed resolution leaves the target queue untouched"
    );
    assert_eq!(app.playback_queue().queue_cursor, 0);
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { .. }
    ));
    assert!(
        app.status.contains("Library error"),
        "resolution failure uses the existing library error channel: {}",
        app.status
    );
}

/// A directly-controlled owner holds the target queue itself, so the executor's
/// local-metadata gate never writes the source label; the staging path must
/// therefore set it, exactly as the shipped album/artist track paths do. The
/// status chrome and the saved-playlist predicate both read this field.
#[test]
fn grouped_track_direct_remote_staging_labels_the_queue_as_album() {
    let mut app = remote_playback_app();
    app.config.lock().unwrap().autoload = true;
    assert!(
        app.has_direct_remote_queue(),
        "the fixture must exercise the directly-controlled owner path"
    );
    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    track.album_id = "album-1".into();
    track.media_type = "Audio".into();
    track.index_number = 1;
    app.album_tracks_cache.insert("album-1".into(), vec![track]);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Playlist".into(),
    };

    assert!(app.play_grouped_track("album-1", "track-1"));
    assert!(
        matches!(app.queue_source, crate::config::QueueSource::Album),
        "a directly-controlled owner receives the album label, got {:?}",
        app.queue_source
    );
    assert!(
        !app.queue_is_saved_playlist(),
        "the stale playlist label must not survive the album replacement"
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

/// Row 3.1: an album track on a populated target queue asks before the routed
/// replacement runs; confirming plays the album through the routed path.
#[test]
fn populated_queue_album_track_asks_then_plays_the_routed_replacement() {
    let mut app = remote_playback_app();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.remote_player_tab
        .as_mut()
        .expect("the direct remote fixture keeps a target queue")
        .set_items(vec![existing], 0);
    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![track.clone()]);

    assert!(app.play_album_track("album-1", &track));

    assert!(matches!(
        &app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(modal))
            if modal.on_confirm == crate::app::ConfirmAction::ReplacePopulatedQueue
    ));
    assert!(matches!(
        app.pending_queue_replacement,
        Some((
            _,
            crate::app::state::types::playback::ReplacementExecutor::Routed(
                crate::app::state::types::playback::RoutedReplacementPrep::Album
            )
        ))
    ));
    assert_eq!(queued_track_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().queue_cursor, 0);

    confirm_replace_queue(&mut app);

    assert_eq!(queued_track_ids(&app), ["track-1"]);
    assert!(app.pending_queue_replacement.is_none());
}

/// Row 3.1 cancellation: Esc at the album-track replacement prompt changes
/// neither the queue nor playback and leaves no stored payload.
#[test]
fn cancelling_album_track_replacement_leaves_the_populated_queue_unchanged() {
    let mut app = remote_playback_app();
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.remote_player_tab
        .as_mut()
        .expect("the direct remote fixture keeps a target queue")
        .set_items(vec![existing], 0);
    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![track.clone()]);

    assert!(app.play_album_track("album-1", &track));
    app.apply_confirm_action(
        crate::app::ConfirmAction::ReplacePopulatedQueue,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );

    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(queued_track_ids(&app), ["existing"]);
    assert_eq!(app.playback_queue().queue_cursor, 0);
}

/// Row 3.1 cancellation / design D4: a folder play on a populated queue
/// defers the Collection source into the confirmed path, so Esc leaves
/// `queue_source` exactly as it was (the callers used to set it before the
/// gate).
#[test]
fn cancelling_a_folder_play_leaves_the_queue_source_unchanged() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let http = mbv_core::mock_http::MockHttp::new();
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

    // Populated target queue + a music library holding the played folder.
    let mut existing = make_item("Existing", "Audio");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "lib-music".into();
    library.collection_type = "music".into();
    app.libs.push(LibraryTab::new(library));
    app.queue_source = crate::config::QueueSource::Album;

    http.respond(
        200,
        r#"{"Items":[{"Id":"track-1","Name":"Track","Type":"Audio","MediaType":"Audio"}]}"#,
    );
    app.play_or_activate_lib_item(0, folder("album-1", "Album"));

    assert!(matches!(
        &app.pending_queue_replacement,
        Some((
            PendingQueueAction::PlayItems {
                source: crate::config::QueueSource::Collection { collection_type },
                ..
            },
            crate::app::state::types::playback::ReplacementExecutor::Routed(
                crate::app::state::types::playback::RoutedReplacementPrep::Folder
            )
        )) if collection_type == "music"
    ));
    assert_eq!(app.queue_source, crate::config::QueueSource::Album);

    app.apply_confirm_action(
        crate::app::ConfirmAction::ReplacePopulatedQueue,
        crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ),
    );

    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(app.queue_source, crate::config::QueueSource::Album);
}

/// Row 3.4: an album track on an empty target queue plays immediately; the
/// gate asks nothing.
#[test]
fn empty_queue_album_track_needs_no_replacement_confirmation() {
    let mut app = remote_playback_app();
    let mut track = make_item("Track", "Audio");
    track.id = "track-1".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![track.clone()]);

    assert!(app.play_album_track("album-1", &track));

    assert!(app.pending_queue_replacement.is_none());
    assert!(!matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
            _
        ))
    ));
    assert_eq!(queued_track_ids(&app), ["track-1"]);
}
