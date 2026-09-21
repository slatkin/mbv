use super::*;
use super::landing::{draw_music_frame, mounted_music_app_at, music_panel, music_panel_mut, tick_key};

/// Tasks 6.1–6.3 correction: a Service `ArtistItems` root's Workspace rows come
/// from the shell-owned artist-detail cache, never `album_tracks_cache`. Enter
/// on a mounted artist Workspace row must cross the typed activation and play
/// the row's album group through the shell dispatch arm's artist-cache
/// resolution.
#[test]
fn enter_on_a_mounted_artist_workspace_row_plays_its_album_group() {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    app.panel_focus = PanelFocus::Library;
    // A configured-but-unroutable client: `play_album_track`'s availability
    // gate passes without a live server.
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));
    // The album carries a Service artist identity and a settled revision; the
    // completed artist-ID query is the only source of the Workspace rows.
    {
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        for item in &mut level.items {
            item.artist_items = vec![mbv_core::api::EmbyArtistRef {
                name: "Alpha".into(),
                id: "artist-alpha".into(),
            }];
        }
        let mut catalog = crate::app::music_grouping::build_grouped_album_catalog(
            &level.items,
            &Default::default(),
        );
        catalog.revision = 7;
        catalog.parent_id = level.parent_id.clone();
        level.music_grouping = Some(crate::app::music_grouping::MusicGroupingState {
            revision: 7,
            candidate: None,
            settled: Some(catalog),
        });
    }
    let destination = crate::app::components::library_panel::LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: crate::app::components::LibraryKind::Music,
    };
    let target = crate::app::components::msg::MusicArtistTarget {
        artist_id: Some("artist-alpha".into()),
        artist_name: "Alpha".into(),
        album_targets: vec!["album-1".into()],
        revision: 7,
    };
    let detail_key = app
        .artist_detail_key(&destination, &target)
        .expect("artist ID key");
    let mut track = crate::app::tests::make_item("Artist Track", "Audio");
    track.id = "artist-track-1".into();
    track.album_id = "album-1".into();
    app.artist_detail_cache.insert(
        detail_key,
        crate::app::music_artist_detail::ArtistDetailCacheEntry {
            tracks: vec![track],
            failed: false,
        },
    );
    assert!(
        app.album_tracks_cache.is_empty(),
        "the artist-ID path populates only the artist cache"
    );

    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .select_first_visible();
    assert!(
        harness.model().test_music_owner().selected_is_artist(),
        "the fixture focuses the Service artist root"
    );
    harness.model_mut().push_music_workspace_content();
    harness.model_mut().test_music_owner_mut().enter_track_focus();
    assert!(harness.model().test_music_owner().track_focused());

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    let activate = outcome
        .messages
        .into_iter()
        .find(|message| matches!(message, Msg::Shell(ShellRequest::MusicTrackActivate { .. })))
        .expect("Enter on the artist Workspace row activates its track");
    let (mut music_resize, mut tv_resize) = (false, false);
    harness
        .model_mut()
        .handle_terminal_message(activate, &mut music_resize, &mut tv_resize);

    assert_eq!(
        harness
            .model()
            .app
            .playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["artist-track-1"],
        "the activated row's artist-cache album group becomes the queue"
    );
}

// ── Tasks 6.4/6.5: artist Workspace entry, atomic Hero switch, prefetch ──

/// A grouped Music app with five Alpha albums in settled order
/// (album-1..album-5), one cached track each: deterministic artist groups,
/// album titles, and neighbour window.
fn mounted_neighbour_app() -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    app.image_protocol_enabled = true;
    {
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items[0].name = "Album 1".into();
        for number in 2..=5 {
            let mut album =
                crate::app::tests::make_item(&format!("Album {number}"), "MusicAlbum");
            album.id = format!("album-{number}");
            album.artist = "Alpha".into();
            album.production_year = 2001;
            level.items.push(album);
        }
        level.total_count = 5;
    }
    for number in 1..=5 {
        let mut track = crate::app::tests::make_item(&format!("Track {number}"), "Audio");
        track.id = format!("track-{number}");
        track.album_id = format!("album-{number}");
        app.album_tracks_cache
            .insert(format!("album-{number}"), vec![track]);
    }
    app
}

/// Task 6.4: Right on a collapsed artist root only expands it; a later Right
/// enters the artist Workspace — non-Wide through the Library Hero overlay,
/// whose artist Hero and grouped Workspace paint in the same push.
#[test]
fn right_on_a_collapsed_artist_root_expands_first_then_opens_its_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 81, 30);
    // The first Left leaves the adopted album leaf for Alpha's root and
    // requests its detail; the second collapses the expanded root.
    tick_key(&mut harness, Key::Left);
    assert!(
        harness.model().test_music_owner().selected_is_artist(),
        "Left moves the leaf to its artist parent"
    );
    tick_key(&mut harness, Key::Left);
    let root = harness
        .model()
        .test_music_owner()
        .browser
        .selected_id()
        .expect("artist root selected");
    assert!(
        !harness
            .model()
            .test_music_owner()
            .browser
            .root_is_expanded(root),
        "Left collapses the expanded root"
    );

    // First Right: expansion only, no overlay.
    tick_key(&mut harness, Key::Right);
    assert!(
        harness
            .model()
            .test_music_owner()
            .browser
            .root_is_expanded(root),
        "the first Right expands the root"
    );
    assert!(
        !music_panel(&harness).test_hero_overlay_open(),
        "expanding must not open the overlay"
    );

    // Later Right: the artist Workspace opens in the Library Hero overlay.
    tick_key(&mut harness, Key::Right);
    assert!(
        music_panel(&harness).test_hero_overlay_open(),
        "the later Right opens the artist Library Hero overlay"
    );
    let hero = music_panel_mut(&mut harness)
        .active_hero_data()
        .expect("the overlay paints the artist Hero");
    assert_eq!(hero.facts.title, "Alpha");
    assert_eq!(
        hero.facts.meta_rows,
        vec!["5 albums".to_string(), "2001".to_string()]
    );
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        10,
        "five album headings plus five grouped track rows"
    );
}

/// Task 6.4: in Wide geometry the later Right enters the inline artist-track
/// Workspace locally, with no overlay and no request.
#[test]
fn right_on_an_expanded_artist_root_enters_the_wide_workspace() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    assert!(harness.model().test_music_owner().selected_is_artist());
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the tree rail still owns the focus"
    );

    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "Wide Right takes the inline artist Workspace's cursor"
    );
    assert!(!music_panel(&harness).test_hero_overlay_open());
}

/// Task 6.4: the Wide Hero and its Workspace switch atomically between the
/// album and artist arms — the title, facts, and rows never come from
/// different selections.
#[test]
fn artist_and_album_hero_workspaces_switch_atomically_in_wide() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);

    {
        let album = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("album hero");
        assert_eq!(album.facts.title, "Album 1");
        assert_eq!(
            album.facts.meta_rows,
            vec!["Alpha".to_string(), "2001".to_string()]
        );
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        1,
        "the album Workspace paints that album's track"
    );

    tick_key(&mut harness, Key::Left);
    {
        let artist = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("artist hero");
        assert_eq!(artist.facts.title, "Alpha");
        assert_eq!(
            artist.facts.meta_rows,
            vec!["5 albums".to_string(), "2001".to_string()]
        );
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        10,
        "the artist Workspace replaces the album rows in the same push"
    );
    assert!(matches!(
        harness.model().test_music_owner().track_list.rows().first(),
        Some(crate::app::components::media_list::MediaListRow::Heading { text }) if text == "Album 1"
    ));

    tick_key(&mut harness, Key::Down);
    {
        let album = music_panel_mut(&mut harness)
            .active_hero_data()
            .expect("album hero returns");
        assert_eq!(album.facts.title, "Album 1");
    }
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        1
    );
}

/// A two-root Grouped Music app (Alpha over `album-1`, Beta over two albums)
/// with no cached track rows anywhere: every artist Workspace entry arms with
/// its rows still in flight, so the arrival path is observable hermetically.
fn two_artist_app() -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    let level = app.libs[0].nav_stack.last_mut().expect("album level");
    for (name, id) in [("Beta Session", "album-beta-1"), ("Beta Nights", "album-beta-2")] {
        let mut album = crate::app::tests::make_item(name, "MusicAlbum");
        album.id = id.into();
        album.artist = "Beta".into();
        level.items.push(album);
    }
    level.total_count = level.items.len();
    app
}

/// Deposits one cached track for `album_id` (the fallback artist Workspace's
/// aggregation source) and re-projects the workspace: the rows "arrive".
fn arrive_album_tracks(harness: &mut TickHarness, album_id: &str, track_id: &str) {
    let mut track = crate::app::tests::make_item(track_id, "Audio");
    track.album_id = album_id.into();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert(album_id.to_string(), vec![track]);
    harness.model_mut().push_music_workspace_content();
    harness.model_mut().sync_mounted_surfaces();
}

/// Task 6.4: a Wide artist-Workspace entry armed before the rows arrived
/// takes the cursor when its own root's rows land — and only then.
#[test]
fn armed_artist_workspace_entry_takes_the_cursor_when_its_own_rows_arrive() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    assert!(harness.model().test_music_owner().selected_is_artist());

    // Right on the expanded root with no rows: the entry arms, the cursor
    // stays with the tree.
    tick_key(&mut harness, Key::Right);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "an armed entry does not take the cursor before its rows arrive"
    );
    assert!(
        harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "the entry is armed for the focused root"
    );

    // The armed root's own rows land: the entry takes the cursor once.
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    assert!(
        harness.model().test_music_owner().track_focused(),
        "the armed entry takes the cursor when its own root's rows arrive"
    );
    assert_eq!(
        harness.model().test_music_owner().track_list.rows().len(),
        2,
        "the focused Workspace holds the armed root's own heading and track"
    );
}

/// Task 6.4: the armed entry carries the root it was armed on. A push for
/// another root never takes the cursor, returning to the armed root does not
/// resurrect the voided entry, and a fresh Right enters the Workspace again.
#[test]
fn armed_artist_workspace_entry_does_not_seize_focus_for_another_root() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Right);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the entry is armed on the Alpha root with no rows"
    );

    // The selection leaves Alpha for the Beta root; the entry voids with it,
    // and Beta's rows then land.
    tick_key(&mut harness, Key::End);
    tick_key(&mut harness, Key::Left);
    assert!(
        !harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "leaving the armed root voids the entry"
    );
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .artist_detail_target()
            .map(|target| target.artist_name),
        Some("Beta".to_string()),
        "the selection moved to the Beta root"
    );
    arrive_album_tracks(&mut harness, "album-beta-1", "beta-track-1");
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "another root's arriving rows never take the cursor"
    );

    // Back on Alpha, its rows arrive too: the voided entry stays dead.
    tick_key(&mut harness, Key::Home);
    assert!(harness.model().test_music_owner().selected_is_artist());
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "returning to the armed root does not resurrect the voided entry"
    );

    // A fresh Right on the same root enters its now-resident Workspace.
    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "a fresh Right enters the artist Workspace"
    );
}

/// Task 6.4: the armed entry belongs to the Wide inline pane. A Narrow
/// transition voids it: arriving rows never take the cursor on the narrow
/// pane nothing paints, and the entry stays dead when the Wide pane returns
/// — a fresh Right is required to enter the Workspace again.
#[test]
fn armed_artist_workspace_entry_is_voided_by_a_narrow_transition() {
    let (mut harness, _id) = mounted_music_app_at(two_artist_app(), 160, 40);
    tick_key(&mut harness, Key::Left);
    tick_key(&mut harness, Key::Right);
    assert!(!harness.model().test_music_owner().track_focused());

    // Wide -> Narrow: the same owner survives, the inline pane does not, and
    // the armed entry is cleared with it.
    harness.model_mut().app.terminal_width = 60;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_narrow_geometry().is_some(),
        "the transition painted the narrow skeleton"
    );
    assert!(
        !harness
            .model()
            .test_music_owner()
            .pending_artist_workspace_focus_for_test(),
        "the Narrow transition clears the armed Wide entry"
    );

    // The armed root's rows land while Narrow, then a frame draws: neither
    // the push nor the draw may take the cursor on the pane nothing paints.
    arrive_album_tracks(&mut harness, "album-1", "alpha-track-1");
    draw_music_frame(&mut harness);
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "a voided entry never re-seizes the focus on the narrow pane"
    );

    // Back to Wide: the entry was voided by the transition, so even its own
    // now-resident rows do not take the cursor without a fresh Right.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_wide_geometry().is_some(),
        "the return painted the wide skeleton"
    );
    assert!(
        !harness.model().test_music_owner().track_focused(),
        "the entry voided by the Narrow transition stays dead in Wide"
    );
    tick_key(&mut harness, Key::Right);
    assert!(
        harness.model().test_music_owner().track_focused(),
        "a fresh Right still enters the now-resident artist Workspace"
    );
}

/// Task 6.5 (design D4): the tree resolves the neighbour window from its
/// completed paint and emits the typed payload in visible order in both
/// presentations; the shell re-resolves no cursor.
#[test]
fn neighbour_prefetch_payload_is_the_painted_trees_order_in_both_presentations() {
    for (width, height) in [(160u16, 40u16), (81, 30)] {
        let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), width, height);
        // The production loop drains the panel's post-paint message before the
        // next frame; clear the adopt-frame request so this test observes the
        // payload for the selection it makes.
        let (mut music_resize, mut tv_resize) = (false, false);
        harness
            .model_mut()
            .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
        assert!(
            harness
                .model_mut()
                .test_music_owner_mut()
                .browser
                .select_album_target("album-3"),
            "{width}x{height}: the fixture interns album-3"
        );
        harness.model_mut().sync_mounted_surfaces();
        assert_eq!(
            harness
                .model()
                .test_music_owner()
                .browser
                .selected_album_target(),
            Some("album-3"),
            "{width}x{height}: the selection survives the sync"
        );
        draw_music_frame(&mut harness);

        match music_panel_mut(&mut harness).take_deferred_msg() {
            Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets, .. })) => {
                assert_eq!(
                    targets,
                    vec![
                        "album-2".to_string(),
                        "album-4".to_string(),
                        "album-5".to_string(),
                    ],
                    "{width}x{height}: one behind and three ahead over the visible leaves"
                );
            }
            other => panic!("{width}x{height}: expected the neighbour request, got {other:?}"),
        }
    }
}

/// Task 6.5: the shell applies the existing idle gate to the typed targets —
/// a closed gate suppresses every fetch — and an artist-root focus ships no
/// request at all.
#[test]
fn neighbour_prefetch_is_idle_gated_and_suppressed_on_an_artist_root() {
    let (mut harness, _id) = mounted_music_app_at(mounted_neighbour_app(), 160, 40);
    let (mut music_resize, mut tv_resize) = (false, false);
    // Clear the adopt-frame request (the production loop drains between
    // frames), then select the album whose window this test asserts.
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    assert!(
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .select_album_target("album-3")
    );
    harness.model_mut().sync_mounted_surfaces();
    // Drop the selected hero's own non-idle fetch so the assertions isolate
    // the neighbour window's reservations.
    harness.model_mut().app.card_image_loading.clear();
    draw_music_frame(&mut harness);
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    for key in ["album-2:P", "album-4:P", "album-5:P"] {
        assert!(
            harness.model().app.card_image_loading.contains(key),
            "{key} prefetches while the idle gate is open"
        );
    }
    assert!(
        !harness.model().app.card_image_loading.contains("album-1:P"),
        "a leaf outside the window is not prefetched"
    );

    // A fresh navigation closes the idle gate: the tree still resolves and
    // emits the window, but the shell makes no reservation for it.
    harness.model_mut().app.card_image_loading.clear();
    harness.model_mut().app.last_nav_at = std::time::Instant::now();
    draw_music_frame(&mut harness);
    let before = harness.model().app.card_image_fetch_calls;
    let targets = match music_panel_mut(&mut harness).take_deferred_msg() {
        Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets, .. })) => targets,
        other => panic!("expected the neighbour request, got {other:?}"),
    };
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::MusicNeighbourPrefetch { targets }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness.model().app.card_image_fetch_calls,
        before,
        "the shell's idle gate makes no fetch reservation"
    );
    assert!(harness.model().app.card_image_loading.is_empty());

    // An artist-root focus suppresses the artwork window entirely, so the
    // tree posts no post-paint payload at all (source pagination for this
    // album level is unconditional and no longer rides this message).
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .select_first_visible();
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel_mut(&mut harness).take_deferred_msg().is_none(),
        "an artist-root focus ships no post-paint payload"
    );
}
