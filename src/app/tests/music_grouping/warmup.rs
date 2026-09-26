use super::*;

#[test]
fn stale_warmup_listing_is_ignored_by_emby_generation() {
    let mut app = make_unopened_music_app();
    let status_before = app.status.clone();
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::new(1),
        groups: vec![make_group_item("stale-group", "Stale")],
    });

    assert!(app.album_artist_levels.is_empty());
    assert!(app.pending_level_artist_warmups.is_empty());
    assert_eq!(app.status, status_before);
    assert!(app.libs[0].nav_stack.is_empty());
}

#[test]
fn current_generation_warmup_listing_is_accepted() {
    let mut app = make_unopened_music_app();
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![make_group_item("current-group", "Current")],
    });

    assert_eq!(
        app.album_artist_levels.get("current-group"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.libs[0].nav_stack.is_empty());
}

#[test]
fn page_two_albums_resolve_from_whole_level_fill_without_new_request() {
    // Page-starvation regression (whole-level coverage): a fill requested
    // while only page-1 albums were listed still bulk-fills every album in
    // the level, so a page-2 candidate resolves from the cache instead of
    // clearing as terminal under the `Filled` state.
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);
    app.start_or_supersede_music_grouping(0);

    // One whole-level fill arrives, covering a page-2 album that was never
    // part of the listing in hand when the fill was requested.
    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![
            ("album-1".into(), "Artist One".into()),
            ("album-2".into(), "Artist Two".into()),
        ],
    });
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );

    // The user pages on: album-2 is appended and a new candidate is created.
    let mut a2 = make_item("Other Album", "MusicAlbum");
    a2.id = "album-2".into();
    a2.artist = String::new();
    if let Some(level) = app.libs[0].nav_stack.last_mut() {
        level.items.push(a2);
    }
    app.start_or_supersede_music_grouping(0);

    // album-2 resolved up front from the fill's cache row; the `Filled`
    // level did not starve it into the folder fallback.
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_none());
    let catalog = state.settled.as_ref().expect("settled catalog");
    let pos = catalog.id_to_entry["album-2"];
    assert_eq!(catalog.entries[pos].artist, "Artist Two");
}

#[test]
fn spawn_level_fetch_dedupes_on_loading_and_filled() {
    let mut app = make_music_app(vec![]);
    let albums = Vec::new();

    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );
    app.spawn_level_artist_fetch("group-0".into(), albums.clone());
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false }),
        "Loading level must not be re-requested"
    );

    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Filled { orphan_risk: false },
    );
    app.spawn_level_artist_fetch("group-0".into(), albums);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Filled { orphan_risk: false }),
        "Filled level must not be re-requested"
    );
}

#[test]
fn spawn_level_fetch_without_client_marks_failed_for_retry() {
    let mut app = make_music_app(vec![]); // stub has no Emby client

    app.spawn_level_artist_fetch("group-0".into(), Vec::new());

    // `Failed` (not a stuck `Loading`): the next candidate creation for
    // the level retries, per design D4.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
}

#[test]
fn failed_level_retries_on_next_candidate_creation() {
    let mut a1 = make_item("Unknown Album", "MusicAlbum");
    a1.id = "album-1".into();
    a1.artist = String::new();
    let mut app = make_music_app(vec![a1]);

    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed),
        "no-client stub fails the fill attempt"
    );

    app.start_or_supersede_music_grouping(0);
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed),
        "Failed level was retried (and failed again without a client)"
    );
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(
        state.candidate.is_some(),
        "candidate stays unresolved waiting for the retry's arrival"
    );
}

#[test]
fn warmup_fan_out_stays_bounded_before_level_arrivals() {
    let mut app = make_unopened_music_app();
    for index in 0..6 {
        let level_id = format!("in-flight-{index}");
        app.album_artist_levels.insert(
            level_id.clone(),
            LevelFillState::Loading { orphan_risk: false },
        );
        app.level_artist_warmups_in_flight.insert(level_id);
    }

    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: (0..7)
            .map(|index| make_group_item(&format!("group-{index}"), "Group"))
            .collect(),
    });

    assert_eq!(app.level_artist_warmups_in_flight.len(), 6);
    assert_eq!(
        app.pending_level_artist_warmups.len(),
        7,
        "warm-up levels beyond the six request slots stay queued"
    );
}

#[test]
fn pending_warmup_is_removed_when_candidate_wins_the_level_race() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );
    app.pending_level_artist_warmups.push_back("group-0".into());

    app.spawn_level_artist_fetch("group-0".into(), Vec::new());

    assert!(app.pending_level_artist_warmups.is_empty());
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false }),
        "the candidate observes the existing warm-up request rather than spawning"
    );
}

#[test]
fn warmup_group_id_matches_the_opened_level_parent_id() {
    let group = make_group_item("group-0", "A-D");
    let album_level = make_music_album_level(vec![make_untagged_album("album-1")]);

    assert_eq!(group.id, album_level.parent_id);
}

#[test]
fn warmup_listing_requests_one_fill_per_group_child_without_a_view() {
    let mut app = make_unopened_music_app();

    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![
            make_group_item("group-0", "A-D"),
            make_group_item("group-1", "E-H"),
        ],
    });

    // The stub has no Emby client, so each requested fill immediately marks
    // its level `Failed` for retry — the established client-less observable
    // for "a fill was requested". Both group children were requested.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
    assert_eq!(
        app.album_artist_levels.get("group-1"),
        Some(&LevelFillState::Failed)
    );
    // The view was never opened: browsing state is untouched.
    assert!(app.libs[0].nav_stack.is_empty());
    assert!(app.album_artist_cache.is_empty());
}

#[test]
fn warmup_library_selection_gates_on_group_config_and_music_collection() {
    let mut app = make_unopened_music_app();
    assert_eq!(
        app.music_group_warmup_library_ids(),
        vec!["lib-music".to_string()]
    );

    // Not a group-first level config: no warm-up targets (the same gate
    // `is_music_group_view` applies).
    app.music_levels = vec!["album".into()];
    assert!(app.music_group_warmup_library_ids().is_empty());

    // Group config restored, but the library is not music.
    app.music_levels = vec!["group".into(), "album".into()];
    app.libs[0].library.collection_type = "movies".into();
    assert!(app.music_group_warmup_library_ids().is_empty());
}

#[test]
fn warmup_dedupes_on_loading_and_filled_levels() {
    let mut app = make_unopened_music_app();
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );

    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![make_group_item("group-0", "A-D")],
    });

    // An in-flight level does no work through the same
    // `LevelFillState::action_for` decision candidates use. In this
    // client-less stub a re-request would have cycled `Loading` -> `Failed`,
    // so `Loading` surviving proves the warm-up spawned nothing.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false })
    );

    app.album_artist_levels.insert(
        "group-1".into(),
        LevelFillState::Filled { orphan_risk: false },
    );
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![make_group_item("group-1", "E-H")],
    });
    assert_eq!(
        app.album_artist_levels.get("group-1"),
        Some(&LevelFillState::Filled { orphan_risk: false })
    );
}

#[test]
fn warmup_and_candidate_share_one_fill_decision() {
    let mut app = make_music_app(vec![make_untagged_album("album-1")]);
    // Seed the in-flight state a real warm-up spawn marks (the stub has no
    // client, so simulate it).
    app.album_artist_levels.insert(
        "group-0".into(),
        LevelFillState::Loading { orphan_risk: false },
    );

    // Opening the grouped view while warm-up is in flight: the candidate
    // takes the NoWork arm and waits instead of starting a second fill.
    app.start_or_supersede_music_grouping(0);
    let state = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .music_grouping
        .as_ref()
        .unwrap();
    assert!(state.candidate.is_some());

    // The warm-up listing arrives too: still no second fill for the level.
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![make_group_item("group-0", "A-D")],
    });
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Loading { orphan_risk: false })
    );
}

#[test]
fn warmup_fill_failure_marks_failed_and_leaves_browsing_untouched() {
    let mut app = make_unopened_music_app();
    app.handle_lib_event(LibEvent::MusicGroupWarmupListed {
        generation: mbv_core::service_runtime::SetupGeneration::default(),
        groups: vec![make_group_item("group-0", "A-D")],
    });
    let status_before = app.status.clone();

    // The per-level fill failed: empty artists is the HTTP-failure shape.
    app.handle_lib_event(LibEvent::AlbumArtistLevelFetched {
        level_id: "group-0".into(),
        artists: vec![],
    });

    // Failed (retryable), and otherwise silent: no cache fill, no status/
    // toast, no queue, no browsing state.
    assert_eq!(
        app.album_artist_levels.get("group-0"),
        Some(&LevelFillState::Failed)
    );
    assert!(app.album_artist_cache.is_empty());
    assert_eq!(app.status, status_before);
    assert!(app.player_tab.all_queue_items().is_empty());
    assert!(app.libs[0].nav_stack.is_empty());

    // Grouped browsing remains usable: opening the level settles through
    // the existing fallback within the grouping resolution window.
    app.libs[0].nav_stack = vec![
        make_group_level(),
        make_music_album_level(vec![make_untagged_album("album-1")]),
    ];
    app.start_or_supersede_music_grouping(0);
    // Force the settle window to have elapsed, as the fallback test does.
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
        .created_at = Instant::now().checked_sub(Duration::from_secs(4)).unwrap();
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
