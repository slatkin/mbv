#![allow(dead_code, unused_imports)]

use super::*;

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
