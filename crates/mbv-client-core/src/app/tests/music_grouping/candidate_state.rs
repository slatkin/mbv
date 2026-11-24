use super::*;
use crate::app::MusicEvent;

#[test]
fn start_or_supersede_creates_candidate_for_music_group_level() {
    let mut a1 = make_item("First Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = "Alpha".into();
    let mut app = make_music_app(vec![a1]);

    app.start_or_supersede_music_grouping_for_test(0);

    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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
    app.start_or_supersede_music_grouping_for_test(0);

    app.advance_music_grouping_candidates_for_test("album-1", "Resolved Artist");

    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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
    app.start_or_supersede_music_grouping_for_test(0);

    app.backdate_music_grouping_candidate_for_test(0, Duration::from_secs(4));

    app.expire_music_grouping_candidates_for_test();

    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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
    app.start_or_supersede_music_grouping_for_test(0);

    // Advance the first album to settle the candidate
    app.advance_music_grouping_candidates_for_test("album-1", "Artist One");

    // Replace items and restart grouping (supersedes)
    let mut a3 = make_item("Album Three", "MusicAlbum");
    a3.id = "album-3".into();
    a3.artist = String::new();
    let mut a4 = make_item("Album Four", "MusicAlbum");
    a4.id = "album-4".into();
    a4.artist = String::new();

    app.seed_music_albums_for_test(vec![a3, a4]);
    app.start_or_supersede_music_grouping_for_test(0);

    // The old album-2 result should not affect the new candidate
    app.advance_music_grouping_candidates_for_test("album-2", "Stale Artist");

    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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

    app.start_or_supersede_music_grouping_for_test(0);

    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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
    app.start_or_supersede_music_grouping_for_test(0);
    // Set cursor to album-2
    app.set_library_level_cursor_for_test(0, 1, 1);

    // Second settle (replacement) should anchor to album-2
    app.start_or_supersede_music_grouping_for_test(0);

    assert_eq!(
        app.emby_library_resting_item_id(0).as_deref(),
        Some("album-2"),
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
    app.start_or_supersede_music_grouping_for_test(0);
    app.mark_album_artist_level_loading_for_test("level-1", false);

    app.handle_lib_event(
        LibEvent::Music(MusicEvent::AlbumArtistLevelFetched {
            level_id: "level-1".into(),
            artists: vec![
                ("album-1".into(), "Artist One".into()),
                ("album-2".into(), "Artist Two".into()),
            ],
        }),
        None,
    );

    assert_eq!(
        app.album_artist_level_fill_for_test("level-1"),
        Some(LevelFillState::Filled { orphan_risk: false })
    );
    assert_eq!(
        app.album_artist_cache_for_test("album-1"),
        Some("Artist One")
    );
    assert_eq!(
        app.album_artist_cache_for_test("album-2"),
        Some("Artist Two")
    );
    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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
    app.mark_album_artist_level_loading_for_test("level-1", false);

    app.handle_lib_event(
        LibEvent::Music(MusicEvent::AlbumArtistLevelFetched {
            level_id: "level-1".into(),
            artists: vec![],
        }),
        None,
    );

    assert_eq!(
        app.album_artist_level_fill_for_test("level-1"),
        Some(LevelFillState::Failed)
    );
    assert!(app.album_artist_cache_is_empty_for_test());
}

#[test]
fn service_reset_clears_album_artist_state() {
    let mut app = make_music_app(vec![]);
    app.seed_album_artist_state_for_test(&[("album-1", "A")], "level-1", "level-2", "level-3");

    app.remove_emby_confirmed_for_test();

    assert_eq!(
        app.album_artist_state_is_cleared_for_test(),
        (true, true, true, true)
    );
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
    app.start_or_supersede_music_grouping_for_test(0);
    app.mark_album_artist_level_loading_for_test("group-0", false);

    app.handle_lib_event(
        LibEvent::Music(MusicEvent::AlbumArtistLevelFetched {
            level_id: "group-0".into(),
            artists: vec![
                ("album-1".into(), String::new()),
                ("album-2".into(), "Artist Two".into()),
            ],
        }),
        None,
    );

    // The empty-artist pair must not poison the cache with an empty
    // tombstone; only the non-empty pair fills.
    assert_eq!(
        app.album_artist_cache_for_test("album-1"),
        None,
        "empty artist must not be cached"
    );
    assert_eq!(
        app.album_artist_cache_for_test("album-2"),
        Some("Artist Two")
    );
    assert_eq!(
        app.album_artist_level_fill_for_test("group-0"),
        Some(LevelFillState::Filled { orphan_risk: false })
    );
    // The arrival still resolves every waiting album: the empty-artist
    // album settles to the folder fallback instead of the cache.
    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
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
    app.mark_album_artist_level_filled_for_test("group-0", false);

    app.start_or_supersede_music_grouping_for_test(0);

    // The fill already had its chance: the album is terminal via the
    // fallback, with no candidate left waiting on `SETTLE_WINDOW`.
    let state = app
        .open_music_grouping_state_for_test(0)
        .expect("grouping state");
    assert!(state.candidate.is_none());
    assert_eq!(
        state.settled.as_ref().unwrap().entries[0].artist,
        "Unknown Artist"
    );
}

#[test]
fn warmup_orphan_risk_gets_one_browse_upgrade_then_stays_terminal() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.mark_album_artist_level_filled_for_test("group-0", true);

    // With the test stub's absent client, this transition proves the
    // browse-triggered upgrade was requested rather than falling back from
    // the warm-up Filled state.
    app.start_or_supersede_music_grouping_for_test(0);
    assert_eq!(
        app.album_artist_level_fill_for_test("group-0"),
        Some(LevelFillState::Failed)
    );
    assert!(
        app.open_music_grouping_state_for_test(0)
            .expect("grouping state")
            .candidate
            .is_some()
    );

    // Model the one upgrade's arrival with an unknown artist. It clears the
    // orphan risk, while the waiting album takes the existing fallback path.
    app.mark_album_artist_level_loading_for_test("group-0", false);
    app.handle_lib_event(
        LibEvent::Music(MusicEvent::AlbumArtistLevelFetched {
            level_id: "group-0".into(),
            artists: vec![("album-1".into(), String::new())],
        }),
        None,
    );
    assert_eq!(
        app.album_artist_level_fill_for_test("group-0"),
        Some(LevelFillState::Filled { orphan_risk: false })
    );

    // A later candidate does not request another fill, even though the
    // album remains unresolved after the upgrade.
    app.start_or_supersede_music_grouping_for_test(0);
    assert_eq!(
        app.album_artist_level_fill_for_test("group-0"),
        Some(LevelFillState::Filled { orphan_risk: false })
    );
    assert!(
        app.open_music_grouping_state_for_test(0)
            .expect("grouping state")
            .candidate
            .is_none()
    );
}

#[test]
fn candidate_filled_level_is_terminal_without_orphan_upgrade() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.mark_album_artist_level_filled_for_test("group-0", false);

    app.start_or_supersede_music_grouping_for_test(0);

    assert!(
        app.open_music_grouping_state_for_test(0)
            .expect("grouping state")
            .candidate
            .is_none()
    );
    assert_eq!(
        app.album_artist_level_fill_for_test("group-0"),
        Some(LevelFillState::Filled { orphan_risk: false })
    );
}

#[test]
fn orphan_risk_filled_level_with_no_unresolved_items_does_not_upgrade() {
    let mut tagged = make_untagged_album("album-1");
    tagged.artist = "Tagged Artist".into();
    let mut app = make_music_app(vec![tagged]);
    app.mark_album_artist_level_filled_for_test("group-0", true);

    app.start_or_supersede_music_grouping_for_test(0);

    assert_eq!(
        app.album_artist_level_fill_for_test("group-0"),
        Some(LevelFillState::Filled { orphan_risk: true })
    );
    assert!(
        app.open_music_grouping_state_for_test(0)
            .expect("grouping state")
            .candidate
            .is_none()
    );
}
