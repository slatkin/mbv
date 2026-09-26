use super::*;

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
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
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
            &std::collections::HashMap::default(),
        );
        catalog.revision = 7;
        catalog.parent_id = level.parent_id.clone();
        level.music_grouping = Some(crate::app::state::music_grouping::MusicGroupingState {
            revision: 7,
            candidate: None,
            settled: Some(catalog),
        });
    };

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
        Msg::Shell(Box::new(ShellRequest::MusicArtistTrackActivate {
            target: rebound.clone(),
            track_id: "artist-track-2".into(),
        })),
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
        Msg::Shell(Box::new(ShellRequest::MusicArtistTrackActivate {
            target: rebound,
            track_id: "missing-track".into(),
        })),
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
