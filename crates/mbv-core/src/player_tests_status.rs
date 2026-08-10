fn make_status(current_idx: usize, queue_len: usize, active: bool, paused: bool) -> PlayerStatus {
    PlayerStatus {
        paused,
        current_idx,
        queue_len,
        active,
        ..Default::default()
    }
}

#[test]
fn next_idx_advances_when_room() {
    let status = make_status(1, 3, true, false);
    assert_eq!(status.next_idx(), Some(2));
}

#[test]
fn next_idx_none_at_end_of_queue() {
    // Regression test for the mpris.rs bug: next() had no upper-bound check
    // and would jump past the end of the queue.
    let status = make_status(2, 3, true, false);
    assert_eq!(status.next_idx(), None);
}

#[test]
fn next_idx_none_when_inactive() {
    let status = make_status(0, 3, false, false);
    assert_eq!(status.next_idx(), None);
}

#[test]
fn current_item_metadata_stores_art_item_id_never_a_token_url() {
    // Regression coverage for #158: `set_current_item_metadata` no longer
    // takes a server URL or token at all (it can't reconstruct an Emby
    // image URL that would embed `token` as a query-string api_key and
    // leak it onto the session D-Bus via mpris:artUrl). mbv-core has no
    // access to the on-disk image cache, so it only records the raw item
    // id; `src/mpris.rs::resolve_art_url` turns that into a file:// URI
    // (or omits mpris:artUrl) using the cache.
    let mut item = make_media_item("track-1");
    item.artist = "Artist".to_string();
    item.album = "Album".to_string();

    let mut status = PlayerStatus::default();
    status.set_current_item_metadata(&item);

    assert_eq!(status.title, item.display_name());
    assert_eq!(status.artist, "Artist");
    assert_eq!(status.album, "Album");
    assert_eq!(status.art_item_id, "track-1");
    // Episode (the default make_media_item item_type) isn't grouped by
    // album, so no album-cache key is recorded.
    assert_eq!(status.art_album_id, "");
}

#[test]
fn current_item_metadata_uses_album_id_for_grouped_audio_tracks() {
    let mut item = make_media_item("track-1");
    item.item_type = "Audio".to_string();
    item.album_id = "album-9".to_string();

    let mut status = PlayerStatus::default();
    status.set_current_item_metadata(&item);

    assert_eq!(status.art_item_id, "track-1");
    assert_eq!(status.art_album_id, "album-9");
}

#[test]
fn clear_current_item_metadata_clears_art_fields() {
    let mut item = make_media_item("track-1");
    item.item_type = "Audio".to_string();
    item.album_id = "album-9".to_string();

    let mut status = PlayerStatus::default();
    status.set_current_item_metadata(&item);
    status.clear_current_item_metadata();

    assert_eq!(status.art_item_id, "");
    assert_eq!(status.art_album_id, "");
}

#[test]
fn previous_idx_none_at_start() {
    let status = make_status(0, 3, true, false);
    assert_eq!(status.previous_idx(), None);
}

#[test]
fn previous_idx_steps_back() {
    let status = make_status(2, 3, true, false);
    assert_eq!(status.previous_idx(), Some(1));
}

#[test]
fn previous_idx_none_when_inactive() {
    let status = make_status(2, 3, false, false);
    assert_eq!(status.previous_idx(), None);
}

#[test]
fn toggle_to_reach_noop_when_already_in_state() {
    let status = make_status(0, 1, true, true);
    assert!(status.toggle_to_reach(true).is_none());
}

#[test]
fn toggle_to_reach_emits_toggle_when_state_differs() {
    let status = make_status(0, 1, true, false);
    assert!(matches!(
        status.toggle_to_reach(true),
        Some(PlayerCommand::TogglePause)
    ));
}

#[test]
fn set_initial_queue_seeds_status_without_starting_playback() {
    let (tx, _rx) = mpsc::channel();
    let player = Player::new(
        String::new(),
        String::new(),
        false,
        false,
        false,
        false,
        false,
        SubtitlePrefs::default(),
        tx,
        None,
    );
    let mut emby_items = [make_media_item("ep1"), make_media_item("ep2")];
    emby_items[1].playback_position_ticks = 123;
    emby_items[1].runtime_ticks = 456;
    let items: Vec<QueueItem> = emby_items
        .iter()
        .map(|i| QueueItem::Emby(Box::new(i.clone())))
        .collect();

    player.set_initial_queue(&items, 1);

    let status = player.status.lock().unwrap().clone();
    assert_eq!(status.current_idx, 1);
    assert_eq!(status.queue_len, 2);
    assert_eq!(status.position_ticks, 123);
    assert_eq!(status.runtime_ticks, 456);
    assert!(!status.active);
    assert_eq!(status.title, emby_items[1].display_name());
}

// ── lang_code_to_name (sync with parse_audio_info in api.rs) ─────────────
// Mirror of parse_audio_info_lang_table_matches_player_lang_code_to_name in
// api.rs::tests. Both tables must be updated together when adding a language.
#[test]
fn lang_code_to_name_matches_api_table() {
    let cases: &[(&str, &str)] = &[
        ("en", "English"),
        ("eng", "English"),
        ("fr", "French"),
        ("fre", "French"),
        ("fra", "French"),
        ("de", "German"),
        ("ger", "German"),
        ("deu", "German"),
        ("es", "Spanish"),
        ("spa", "Spanish"),
        ("it", "Italian"),
        ("ita", "Italian"),
        ("pt", "Portuguese"),
        ("por", "Portuguese"),
        ("ja", "Japanese"),
        ("jpn", "Japanese"),
        ("ko", "Korean"),
        ("kor", "Korean"),
        ("zh", "Chinese"),
        ("chi", "Chinese"),
        ("zho", "Chinese"),
        ("ru", "Russian"),
        ("rus", "Russian"),
        ("ar", "Arabic"),
        ("ara", "Arabic"),
        ("nl", "Dutch"),
        ("nld", "Dutch"),
        ("dut", "Dutch"),
        ("sv", "Swedish"),
        ("swe", "Swedish"),
        ("no", "Norwegian"),
        ("nor", "Norwegian"),
        ("da", "Danish"),
        ("dan", "Danish"),
        ("fi", "Finnish"),
        ("fin", "Finnish"),
        ("pl", "Polish"),
        ("pol", "Polish"),
        ("cs", "Czech"),
        ("cze", "Czech"),
        ("ces", "Czech"),
        ("tr", "Turkish"),
        ("tur", "Turkish"),
    ];
    for (code, expected) in cases {
        assert_eq!(
            lang_code_to_name(code),
            *expected,
            "lang_code_to_name({:?})",
            code
        );
    }
}

#[test]
fn disconnect_remote_is_a_no_op_for_a_local_player() {
    let status = Arc::new(Mutex::new(PlayerStatus::default()));
    let proxy = PlayerProxy::stub(status);
    assert!(!proxy.is_remote());

    proxy.disconnect_remote(); // must not panic
}

#[test]
fn disconnect_remote_disconnects_a_remote_player() {
    let (remote, _event_rx) = crate::remote_player::RemotePlayer::stub(Vec::new(), 0);
    let proxy = PlayerProxy {
        always_play_next: false,
        status: remote.status.clone(),
        subtitle_prefs: remote.subtitle_prefs.clone(),
        inner: PlayerProxyInner::Remote(remote),
    };
    assert!(proxy.is_remote());

    proxy.disconnect_remote(); // must not panic; a stub has no real
                               // socket, so this only exercises the
                               // dispatch, not the shutdown itself
                               // (that's covered by Task 2's tests).
}
