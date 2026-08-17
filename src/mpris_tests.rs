    use super::*;

    fn string_value(metadata: &HashMap<String, zvariant::Value<'static>>, key: &str) -> String {
        metadata
            .get(key)
            .unwrap_or_else(|| panic!("missing metadata key {key}"))
            .downcast_ref::<String>()
            .unwrap_or_else(|_| panic!("metadata key {key} is not a string"))
    }

    #[test]
    fn active_metadata_includes_cover_art_as_file_uri_when_cached() {
        // #158 regression coverage: mpris:artUrl must be a local file://
        // URI, never an Emby URL (which would carry the API token onto the
        // session bus).
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                active: true,
                title: "Song".to_string(),
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                art_item_id: "track-1".to_string(),
                ..PlayerStatus::default()
            },
            |key| {
                assert_eq!(key, "track-1:card", "expected the track-id cache key first");
                Some(std::path::PathBuf::from("/cache/images/track-1_card"))
            },
        );

        assert_eq!(
            string_value(&metadata, "mpris:artUrl"),
            "file:///cache/images/track-1_card"
        );
        assert!(metadata.contains_key("xesam:artist"));
        assert_eq!(string_value(&metadata, "xesam:album"), "Album");
    }

    #[test]
    fn active_metadata_prefers_album_cache_key_for_grouped_audio_tracks() {
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                active: true,
                title: "Song".to_string(),
                art_item_id: "track-1".to_string(),
                art_album_id: "album-9".to_string(),
                ..PlayerStatus::default()
            },
            |key| {
                (key == "album-9:card")
                    .then(|| std::path::PathBuf::from("/cache/images/album-9_card"))
            },
        );

        assert_eq!(
            string_value(&metadata, "mpris:artUrl"),
            "file:///cache/images/album-9_card"
        );
    }

    #[test]
    fn active_metadata_omits_art_url_when_not_cached() {
        // Per the #158 triage decision: when cached art isn't available,
        // omit mpris:artUrl entirely rather than falling back to a
        // token-bearing Emby URL.
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                active: true,
                title: "Song".to_string(),
                art_item_id: "track-1".to_string(),
                ..PlayerStatus::default()
            },
            |_key| None,
        );

        assert!(!metadata.contains_key("mpris:artUrl"));
    }

    #[test]
    fn inactive_metadata_omits_track_details_and_never_touches_the_cache() {
        let metadata = make_metadata_with_art_resolver(
            &PlayerStatus {
                artist: "Artist".to_string(),
                album: "Album".to_string(),
                art_item_id: "track-1".to_string(),
                ..PlayerStatus::default()
            },
            |_key| panic!("art cache should never be consulted for an inactive/no-track state"),
        );

        assert!(!metadata.contains_key("mpris:artUrl"));
        assert!(!metadata.contains_key("xesam:artist"));
        assert!(!metadata.contains_key("xesam:album"));
    }

    #[test]
    fn art_cache_key_candidates_prefers_album_then_track_id() {
        assert_eq!(
            art_cache_key_candidates("track-1", "album-9"),
            vec!["album-9:card", "album-9:album_card", "track-1:card"]
        );
        assert_eq!(
            art_cache_key_candidates("track-1", ""),
            vec!["track-1:card"]
        );
        assert!(art_cache_key_candidates("", "").is_empty());
    }

    #[test]
    fn effective_status_forces_inactive_when_disconnected() {
        // #160: once the daemon connection drops, published MPRIS state
        // must go to Stopped/NoTrack even if `status` still has stale
        // "still playing" data in it.
        let playing = PlayerStatus {
            active: true,
            title: "Song".to_string(),
            art_item_id: "track-1".to_string(),
            ..PlayerStatus::default()
        };

        let untouched = effective_status(playing.clone(), false);
        assert!(untouched.active);

        let forced = effective_status(playing, true);
        assert!(!forced.active);
        // make_metadata should now take the inactive/NoTrack branch.
        let metadata = make_metadata_with_art_resolver(&forced, |_| {
            panic!("art cache should never be consulted once disconnected")
        });
        assert!(!metadata.contains_key("xesam:title"));
        assert!(!metadata.contains_key("mpris:artUrl"));
    }

    #[test]
    fn rebind_repoints_a_handle_at_a_new_status_and_sender() {
        // #175: `App::switch_to_direct_remote` / `restore_local_mode` call
        // `rebind` to re-point an already-registered MPRIS service at
        // whichever `Player`/`RemotePlayer` now owns playback. This test
        // exercises `rebind` directly (no real D-Bus/tokio involved) --
        // it's the smallest reproduction of the propagation break: before
        // `rebind` existed, nothing updated the `Arc<Mutex<PlayerStatus>>`
        // MPRIS's polling loop was watching after such a swap.
        let status_a = Arc::new(Mutex::new(PlayerStatus {
            active: false,
            ..PlayerStatus::default()
        }));
        let status_b = Arc::new(Mutex::new(PlayerStatus {
            active: true,
            title: "Remote Song".to_string(),
            ..PlayerStatus::default()
        }));
        let sent = Arc::new(Mutex::new(Vec::<PlayerCommand>::new()));

        let handle: MprisHandle = Arc::new(Mutex::new(MprisSource {
            status: status_a.clone(),
            send: Arc::new(|_: PlayerCommand| {}),
            disconnected: None,
        }));

        let sent_for_rebind = sent.clone();
        let disconnected_b = Arc::new(std::sync::atomic::AtomicBool::new(false));
        rebind(
            &handle,
            status_b.clone(),
            move |cmd| sent_for_rebind.lock().unwrap().push(cmd),
            Some(disconnected_b.clone()),
        );

        let source = handle.lock().unwrap();
        assert!(
            Arc::ptr_eq(&source.status, &status_b),
            "rebind must repoint the handle's status at the new source, not stay on the old one"
        );
        assert!(!Arc::ptr_eq(&source.status, &status_a));
        assert!(source
            .disconnected
            .as_ref()
            .is_some_and(|d| Arc::ptr_eq(d, &disconnected_b)));
        (source.send)(PlayerCommand::TogglePause);
        drop(source);
        let sent = sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert!(matches!(sent[0], PlayerCommand::TogglePause));
    }
