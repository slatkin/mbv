fn make_remote_session(audio_only: bool) -> mbv_core::api::SessionInfo {
    mbv_core::api::SessionInfo {
        media_info: mbv_core::api::SessionMediaInfo {
            audio_only,
            ..Default::default()
        },
        ..crate::app::tests::make_session("device", "Emby")
    }
}

#[test]
fn is_audio_item_reads_remote_session_audio_only_flag_when_true() {
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_remote_session(true));

    assert!(
        app.is_audio_item(),
        "a connected session's audio_only flag should decide is_audio_item(), \
         not local playlist/cursor state"
    );
}

#[test]
fn is_audio_item_reads_remote_session_audio_only_flag_when_false() {
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_remote_session(false));

    assert!(!app.is_audio_item());
}

#[test]
fn is_audio_item_falls_back_to_local_state_when_no_session() {
    let mut app = crate::app::tests::make_app_stub();
    assert!(app.connected_session_id.is_none());
    app.player_tab.set_items(
        vec![crate::app::tests::make_item("song", "Audio")],
        app.player_tab.queue_cursor,
    );
    app.player_tab.queue_cursor = 0;

    assert!(app.is_audio_item());
}

#[test]
fn toggle_mute_falls_back_to_cycle_audio_when_remote_session_connected() {
    // No session-level mute primitive exists (#88), so toggle_mute()
    // must hand off to cycle_audio()'s session-aware branch instead of
    // touching local ui_volume/pre_mute_volume state, which wouldn't
    // reflect a remote session's audio-only playback anyway.
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_remote_session(true));
    let ui_volume_before = app.ui_volume;

    app.toggle_mute();

    assert_eq!(
        app.ui_volume, ui_volume_before,
        "remote toggle_mute() must not touch local ui_volume state"
    );
    assert_eq!(
        app.connected_session_state.as_ref().unwrap().audio_index,
        2,
        "toggle_mute() should have delegated to cycle_audio()'s remote branch, \
         which advances the session's audio_index"
    );
}
#[test]
fn fetch_album_tracks_is_a_no_op_when_already_loading() {
    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_loading.insert("album-1".into());

    app.fetch_album_tracks("album-1".into());

    assert!(
        !app.album_tracks_cache.contains_key("album-1"),
        "a duplicate call while a fetch is already in flight must not \
         spawn a second fetch or fabricate a cache entry"
    );
}
