use super::*;

#[test]
fn start_or_supersede_creates_candidate_for_music_group_level() {
    let mut a1 = make_item("First Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut app = make_music_app(vec![a1]);

    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    // All albums have artist tags, so the candidate commits immediately.
    assert_eq!(state.revision, 1);
    assert!(
        state.settled.is_some(),
        "should settle immediately when all terminal"
    );
}

#[test]
fn advance_removes_from_unresolved_and_resolves() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);
    app.start_or_supersede_music_grouping(0);

    app.advance_music_grouping_candidates("album-1", "Resolved Artist");

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    assert!(
        state.candidate.is_none(),
        "candidate should be committed after all resolved"
    );
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries.len(), 1);
    assert_eq!(catalog.entries[0].artist, "Resolved Artist");
}

#[test]
fn silent_artist_lookups_expire_to_fallback() {
    let mut album = make_item("Unknown Album", "MusicAlbum");
    album.id = "album-1".into();
    album.artist = String::new();
    let mut app = make_music_app(vec![album]);
    app.start_or_supersede_music_grouping(0);

    app.libs[0]
        .nav_stack
        .last_mut()
        .unwrap()
        .music_grouping
        .as_mut()
        .unwrap()
        .candidate
        .as_mut()
        .unwrap()
        .created_at = Instant::now() - Duration::from_secs(4);

    app.expire_music_grouping_candidates();

    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    assert_eq!(
        state.settled.as_ref().unwrap().entries[0].artist,
        "Unknown Artist"
    );
}

#[test]
fn obsolete_candidate_does_not_commit() {
    let mut a1 = make_item("Album One", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut a2 = make_item("Album Two", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    let mut app = make_music_app(vec![a1, a2]);
    app.start_or_supersede_music_grouping(0);

    // Advance the first album to settle the candidate
    app.advance_music_grouping_candidates("album-1", "Artist One");

    // Replace items and restart grouping (supersedes)
    let mut a3 = make_item("Album Three", "MusicAlbum");
    a3.id = "album-3".into();
    a3.artist = String::new();
    let mut a4 = make_item("Album Four", "MusicAlbum");
    a4.id = "album-4".into();
    a4.artist = String::new();

    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items = vec![a3, a4];
    }
    app.start_or_supersede_music_grouping(0);

    // The old album-2 result should not affect the new candidate
    app.advance_music_grouping_candidates("album-2", "Stale Artist");

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    if let Some(candidate) = &state.candidate {
        assert!(
            !candidate.resolved.contains_key("album-2"),
            "stale result must not be in the new candidate"
        );
    }
}

#[test]
fn catalog_preserves_artist_identity_across_settle() {
    let mut a1 = make_item("Alpha Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Beta Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();
    let mut app = make_music_app(vec![a1, a2]);

    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let state = level.music_grouping.as_ref().expect("grouping state");
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries.len(), 2);
    assert_eq!(catalog.entries[0].artist, "Alpha");
    assert_eq!(catalog.entries[1].artist, "Beta");
}

#[test]
fn commit_anchors_cursor_to_selected_album() {
    let mut a1 = make_item("Alpha Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut a2 = make_item("Beta Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = "Beta".into();
    let mut app = make_music_app(vec![a1, a2]);

    // First settle
    app.start_or_supersede_music_grouping(0);
    // Set cursor to album-2
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.set_resting_cursor(1);
    }

    // Second settle (replacement) should anchor to album-2
    let items = app.libs[0].nav_stack.last().unwrap().items.clone();
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items = items;
    }
    app.start_or_supersede_music_grouping(0);

    let level = app.libs[0].nav_stack.last().unwrap();
    let cursor_id = level.items[level.resting().cursor()].id.clone();
    assert_eq!(
        cursor_id, "album-2",
        "cursor should be anchored to the previously selected album"
    );
}

#[test]
fn level_event_bulk_fills_cache_and_settles_candidate() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut a2 = make_item("Other Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    let mut app = make_music_app(vec![a1, a2]);
    app.start_or_supersede_music_grouping(0);
    app.album_artist_levels.insert(
        "level-1".into(),
        LevelFillState::Loading { orphan_risk: false },
    );

    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "level-1".into(),
        artists: vec![
            ("album-1".into(), "Artist One".into()),
            ("album-2".into(), "Artist Two".into()),
        ],
    });

    assert_eq!(
        app.album_artist_levels.get("level-1"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
    assert_eq!(
        app.album_artist_cache.get("album-1").map(String::as_str),
        Some("Artist One")
    );
    assert_eq!(
        app.album_artist_cache.get("album-2").map(String::as_str),
        Some("Artist Two")
    );
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(
        state.candidate.is_none(),
        "arrival resolves every waiting album at once"
    );
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries[0].artist, "Artist One");
    assert_eq!(catalog.entries[1].artist, "Artist Two");
}

#[test]
fn level_event_empty_artists_marks_failed_without_filling() {
    let mut app = make_music_app(vec![]);
    app.album_artist_levels.insert(
        "level-1".into(),
        LevelFillState::Loading { orphan_risk: false },
    );

    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "level-1".into(),
        artists: vec![],
    });

    assert_eq!(
        app.album_artist_levels.get("level-1"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.album_artist_cache.is_empty());
}

#[test]
fn service_reset_clears_album_artist_state() {
    let mut app = make_music_app(vec![]);
    app.album_artist_cache.insert("album-1".into(), "A".into());
    app.album_artist_levels.insert(
        "level-1".into(),
        LevelFillState::Filled { orphan_risk: false },
    );
    app.pending_level_artist_warmups.push_back("level-2".into());
    app.level_artist_warmups_in_flight.insert("level-3".into());

    app.remove_emby_confirmed();

    assert!(app.album_artist_cache.is_empty());
    assert!(app.album_artist_levels.is_empty());
    assert!(app.pending_level_artist_warmups.is_empty());
    assert!(app.level_artist_warmups_in_flight.is_empty());
}

#[test]
fn level_event_empty_artist_pair_fills_only_non_empty_pair() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut a2 = make_item("Other Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    let mut app = make_music_app(vec![a1, a2]);
    app.start_or_supersede_music_grouping(0);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );

    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![
            ("album-1".into(), String::new()),
            ("album-2".into(), "Artist Two".into()),
        ],
    });

    // The empty-artist pair must not poison the cache with an empty
    // tombstone; only the non-empty pair fills.
    assert_eq!(
        app.album_artist_cache.get("album-1"),
        None,
        "empty artist must not be cached"
    );
    assert_eq!(
        app.album_artist_cache.get("album-2").map(String::as_str),
        Some("Artist Two")
    );
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
    // The arrival still resolves every waiting album: the empty-artist
    // album settles to the folder fallback instead of the cache.
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    let catalog = state.settled.as_ref().expect("settled catalog");
    assert_eq!(catalog.entries[0].album_id, "album-2");
    assert_eq!(catalog.entries[0].artist, "Artist Two");
    assert_eq!(catalog.entries[1].album_id, "album-1");
    assert_eq!(catalog.entries[1].artist, "Unknown Artist");
}

#[test]
fn filled_level_with_unresolvable_albums_settles_immediately() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: false },
    );

    app.start_or_supersede_music_grouping(0);

    // The fill already had its chance: the album is terminal via the
    // fallback, with no candidate left waiting on `SETTLE_WINDOW`.
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    assert_eq!(
        state.settled.as_ref().unwrap().entries[0].artist,
        "Unknown Artist"
    );
}

#[test]
fn warmup_orphan_risk_gets_one_browse_upgrade_then_stays_terminal() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: true },
    );

    // With the test stub's absent client, this transition proves the
    // browse-triggered upgrade was requested rather than falling back from
    // the warm-up Filled state.
    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_some());

    // Model the one upgrade's arrival with an unknown artist. It clears the
    // orphan risk, while the waiting album takes the existing fallback path.
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );
    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![("album-1".into(), String::new())],
    });
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );

    // A later candidate does not request another fill, even though the
    // album remains unresolved after the upgrade.
    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_none());
}

#[test]
fn candidate_filled_level_is_terminal_without_orphan_upgrade() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: false },
    );

    app.start_or_supersede_music_grouping(0);

    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_none());
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
}

#[test]
fn orphan_risk_filled_level_with_no_unresolved_items_does_not_upgrade() {
    let mut tagged = make_untagged_album("album-1");
    tagged.artist = "Tagged Artist".into();
    let mut app = make_music_app(vec![tagged]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: true },
    );

    app.start_or_supersede_music_grouping(0);

    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: true })
    );
    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap()
        .candidate
        .is_none());
}
