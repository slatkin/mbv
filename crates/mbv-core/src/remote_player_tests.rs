use super::*;
use crate::config::QueueSource;
use crate::ctrl::CtrlState;
use crate::ctrl::WireCommand;
use crate::playback_queue::QueueItem;
use crate::player::PlayerCommand;

fn make_media_item(id: &str) -> EmbyItem {
    EmbyItem {
        id: id.into(),
        name: "Test Item".into(),
        item_type: "Episode".into(),
        is_folder: false,
        media_type: "Video".into(),
        collection_type: String::new(),
        runtime_ticks: 0,
        played: false,
        playback_position_ticks: 0,
        series_id: String::new(),
        series_name: String::new(),
        album_id: String::new(),
        album: String::new(),
        index_number: 0,
        parent_index_number: 0,
        unplayed_item_count: 0,
        path: String::new(),
        artist: String::new(),
        sort_name: String::new(),
        production_year: 0,
        end_year: 0,
        overview: String::new(),
        premiere_date: String::new(),
        date_added: String::new(),
        total_count: 0,
        container: String::new(),
        director: String::new(),
        video_info: String::new(),
        audio_info: String::new(),
        genre: String::new(),
        playlist_item_id: String::new(),
    }
}

fn status_with_idx(current_idx: usize) -> PlayerStatus {
    status_with_idx_and_len(current_idx, 0)
}

fn status_with_idx_and_len(current_idx: usize, queue_len: usize) -> PlayerStatus {
    RemotePlayer::stub_status(current_idx, queue_len)
}

#[test]
fn daemon_endpoint_parses_local_and_unix_paths() {
    assert_eq!(
        DaemonEndpoint::parse("local").unwrap(),
        DaemonEndpoint::Local
    );
    assert_eq!(DaemonEndpoint::parse("").unwrap(), DaemonEndpoint::Local);
    assert_eq!(
        DaemonEndpoint::parse("unix:///tmp/mbv.sock").unwrap(),
        DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
    );
    assert_eq!(
        DaemonEndpoint::parse("/tmp/mbv.sock").unwrap(),
        DaemonEndpoint::Unix(PathBuf::from("/tmp/mbv.sock"))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://localhost:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://127.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 1], 1234)))
    );
    assert_eq!(
        DaemonEndpoint::parse("tcp://127.0.0.2:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([127, 0, 0, 2], 1234)))
    );
}

#[test]
fn daemon_endpoint_rejects_unsupported_schemes() {
    assert_eq!(
        DaemonEndpoint::parse("tcp://10.0.0.1:1234").unwrap(),
        DaemonEndpoint::Tcp(SocketAddr::from(([10, 0, 0, 1], 1234)))
    );
    assert!(DaemonEndpoint::parse("tcp://[::1]:4321").is_err());
    assert!(DaemonEndpoint::parse("unix://").is_err());
    assert!(DaemonEndpoint::parse("http://localhost:1234").is_err());
}

#[test]
fn status_only_preserves_event_confirmed_current_index() {
    let status = Arc::new(Mutex::new(status_with_idx(3)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx(5)),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
}

#[test]
fn state_uses_cursor_as_current_index() {
    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::State(CtrlState {
            status: status_with_idx(5),
            items: Vec::new(),
            cursor: 3,
            source: QueueSource::Unknown,
            feed_items: Vec::new(),
        }),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().current_idx, 3);
    assert!(matches!(
        rx.recv().unwrap(),
        PlayerEvent::QueueUpdated { cursor: 3, .. }
    ));
}

#[test]
fn status_only_preserves_current_idx_and_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(3, 7)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::StatusOnly(status_with_idx_and_len(5, 2)),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 3);
    assert_eq!(s.queue_len, 7);
}

#[test]
fn state_derives_queue_len_from_items_not_status() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    // s.status.queue_len (99) is stale relative to s.items.len() (2) — the
    // daemon broadcasts CtrlState before calling play_queue(...), so
    // items/cursor are authoritative over status at broadcast time.
    apply_ctrl_event(
        CtrlEvent::State(CtrlState {
            status: status_with_idx_and_len(5, 99),
            items: vec![make_media_item("a"), make_media_item("b")],
            cursor: 1,
            source: QueueSource::Unknown,
            feed_items: Vec::new(),
        }),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    assert_eq!(status.lock().unwrap().queue_len, 2);
}

#[test]
fn track_changed_updates_current_idx_but_not_queue_len() {
    let status = Arc::new(Mutex::new(status_with_idx_and_len(0, 5)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, _rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::Player(PlayerEvent::TrackChanged(2)),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    let s = status.lock().unwrap();
    assert_eq!(s.current_idx, 2);
    assert_eq!(s.queue_len, 5);
}

#[test]
fn command_rejected_forwards_reason_as_player_event() {
    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    apply_ctrl_event(
        CtrlEvent::CommandRejected("daemon is audio-only".to_string()),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    match rx.recv().unwrap() {
        PlayerEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon is audio-only");
        }
        _ => panic!("expected CommandRejected"),
    }
}

#[test]
fn unified_queue_state_preserves_canonical_coordinates_and_source() {
    use crate::ctrl::{UnifiedQueueSlot, UnifiedQueueStateData};

    let status = Arc::new(Mutex::new(status_with_idx(0)));
    let items = Arc::new(Mutex::new(Vec::<EmbyItem>::new()));
    let unified_queue = Arc::new(Mutex::new(None));
    let queue_source = Arc::new(Mutex::new(QueueSource::Unknown));
    let (tx, rx) = mpsc::channel();

    // Mixed queue: [Emby(e0), Feed(f1), Emby(e2), Feed(f3)]. The active
    // Feed slot must retain its canonical index; it cannot be represented by
    // an index into the legacy Emby-only projection.
    let e0 = make_media_item("e0");
    let e2 = make_media_item("e2");
    let f1 = crate::playback_queue::FeedEntry {
        guid: "f1".into(),
        title: "f1".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
    };
    let f3 = crate::playback_queue::FeedEntry {
        guid: "f3".into(),
        title: "f3".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: None,
    };

    let unified = UnifiedQueueStateData {
        status: status_with_idx_and_len(1, 4),
        slots: vec![
            UnifiedQueueSlot {
                slot_id: 10,
                item: QueueItem::Emby(Box::new(e0)),
            },
            UnifiedQueueSlot {
                slot_id: 20,
                item: QueueItem::Feed(f1),
            },
            UnifiedQueueSlot {
                slot_id: 30,
                item: QueueItem::Emby(Box::new(e2)),
            },
            UnifiedQueueSlot {
                slot_id: 40,
                item: QueueItem::Feed(f3),
            },
        ],
        active_slot: Some(20), // canonical slot_id for f1
        revision: 1,
        source: QueueSource::Playlist {
            id: Some("pl-1".into()),
            name: "My Playlist".into(),
        },
    };

    apply_ctrl_event(
        CtrlEvent::UnifiedQueueState(unified),
        &status,
        &items,
        &unified_queue,
        &queue_source,
        &tx,
        &Arc::new(Mutex::new(std::collections::HashMap::new())),
        true,
    );

    // queue_source carried from unified state
    assert!(
        matches!(
            *queue_source.lock().unwrap(),
            QueueSource::Playlist {
                id: Some(ref id),
                ..
            } if id == "pl-1"
        ),
        "queue_source should be carried from UnifiedQueueStateData"
    );

    // queue_len from canonical slots
    assert_eq!(status.lock().unwrap().queue_len, 4);

    assert_eq!(status.lock().unwrap().current_idx, 1);

    // Emby-only items: Feed entries stripped
    let emby = items.lock().unwrap();
    assert_eq!(emby.len(), 2, "only Emby items in legacy projection");
    assert_eq!(emby[0].id, "e0");
    assert_eq!(emby[1].id, "e2");

    let canonical = unified_queue.lock().unwrap().clone().unwrap();
    assert_eq!(canonical.active_slot, Some(20));
    assert_eq!(canonical.slots[1].slot_id, 20);

    // UnifiedQueueUpdated event emitted with full canonical data
    match rx.recv().unwrap() {
        PlayerEvent::UnifiedQueueUpdated(state) => {
            assert_eq!(state.slots.len(), 4);
            assert_eq!(state.active_slot, Some(20));
            assert_eq!(
                state.source,
                QueueSource::Playlist {
                    id: Some("pl-1".into()),
                    name: "My Playlist".into(),
                }
            );
            // Slot IDs preserved
            assert_eq!(state.slots[0].slot_id, 10);
            assert_eq!(state.slots[1].slot_id, 20);
            assert_eq!(state.slots[2].slot_id, 30);
            assert_eq!(state.slots[3].slot_id, 40);
        }
        _ => panic!("expected UnifiedQueueUpdated"),
    }
}
