use super::*;

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
            track.index_number = i64::try_from(index).unwrap() + 1;
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
