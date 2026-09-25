use super::super::landing::tick_key;
use super::super::*;

/// Tasks 6.1–6.3 correction: a Service `ArtistItems` root's Workspace rows come
/// from the shell-owned artist-detail cache, never `album_tracks_cache`. Enter
/// on a mounted artist Workspace row must cross the typed activation and play
/// the flattened artist discography through the shell dispatch arm's
/// artist-detail resolution.
#[test]
fn enter_on_a_mounted_artist_workspace_row_plays_the_artist_discography_from_selected_index() {
    let mut app = crate::app::render::make_music_group_app();
    let mut second_album = crate::app::tests::make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0]
        .nav_stack
        .last_mut()
        .expect("album level")
        .items
        .push(second_album);
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
    let destination = crate::app::components::library_panel::LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: crate::app::components::LibraryKind::Music,
    };
    let target = crate::app::components::msg::MusicArtistTarget {
        artist_id: Some("artist-alpha".into()),
        artist_name: "Alpha".into(),
        album_targets: vec!["album-1".into(), "album-2".into()],
        revision: 7,
    };
    let detail_key = app
        .artist_detail_key(&destination, &target)
        .expect("artist ID key");
    let mut first = crate::app::tests::make_item("Artist Track One", "Audio");
    first.id = "artist-track-1".into();
    first.album_id = "album-1".into();
    let mut selected = crate::app::tests::make_item("Artist Track Two", "Audio");
    selected.id = "artist-track-2".into();
    selected.album_id = "album-1".into();
    let mut later = crate::app::tests::make_item("Artist Track Three", "Audio");
    later.id = "artist-track-3".into();
    later.album_id = "album-2".into();
    app.artist_detail_cache.insert(
        detail_key,
        crate::app::state::music_artist_detail::ArtistDetailCacheEntry {
            tracks: vec![first, selected, later],
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
        .apply(crate::app::components::list::tree_browser::TreeOperation::First);
    assert!(
        harness.model().test_music_owner().selected_is_artist(),
        "the fixture focuses the Service artist root"
    );
    harness.model_mut().push_music_workspace_content();
    harness
        .model_mut()
        .test_music_owner_mut()
        .enter_track_focus();
    assert!(harness.model().test_music_owner().track_focused());
    tick_key(&mut harness, Key::Down);

    harness.inject(key(Key::Enter));
    let outcome = harness.step();
    let activate = outcome
        .messages
        .into_iter()
        .find(|message| {
            matches!(
                message,
                Msg::Shell(ref shell_boxed)
             if matches!(shell_boxed.as_ref(), ShellRequest::MusicArtistTrackActivate { .. }))
        })
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
        ["artist-track-1", "artist-track-2", "artist-track-3"],
        "the full flattened artist discography becomes the queue"
    );
    assert_eq!(
        harness.model().app.playback_queue().queue_cursor,
        1,
        "playback starts at the selected artist track"
    );
}
