use super::{
    all_audio, audio_only_rejection, broadcast, handle_ctrl, handle_ws,
    take_authority_for_emby_remote, AuthorityHolder, CtrlClients, CtrlEvent, CtrlOutbound,
    CtrlRequest, CtrlTransport, DaemonEvent, DaemonPlayerOwner, PlaybackIntentState,
    PlayerOwnerState,
    SharedQueueState,
};
use crate::api::EmbyItem;
use crate::config::{Config, QueueSource};
use crate::ctrl::DisconnectReason;
use crate::ctrl::{
    CtrlCmd, PlaybackIntent, PlaybackIntentAction, PlaybackIntentOutcome, WireCommand,
};
use crate::playback_queue::{FeedEntry, PlaybackQueue, QueueItem};
use crate::player::{Player, PlayerCommand, PlayerEvent, PlayerStatus, SubtitlePrefs};
use crate::ws::WsEvent;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

fn item(name: &str, media_type: &str, item_type: &str) -> EmbyItem {
    EmbyItem {
        id: name.into(),
        name: name.into(),
        item_type: item_type.into(),
        is_folder: false,
        media_type: media_type.into(),
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

fn emby_qi(name: &str, media_type: &str, item_type: &str) -> QueueItem {
    QueueItem::Emby(Box::new(item(name, media_type, item_type)))
}

fn video_feed_qi(guid: &str) -> QueueItem {
    QueueItem::Feed(FeedEntry {
        guid: guid.into(),
        title: guid.into(),
        enclosure_url: None,
        link: None,
        mime_type: Some("video/mp4".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Video),
        feed_id: None,
        position_ticks: 0,
        played: false,
    })
}
/// Connects a client the same way the accept thread does.
fn connect_client(clients: &mut CtrlClients) -> (u64, mpsc::Receiver<CtrlOutbound>) {
    let (tx, rx) = mpsc::channel();
    let id = clients.connect(tx, CtrlTransport::Local, true, true, true, true);
    (id, rx)
}

fn shared_queue_state() -> SharedQueueState {
    SharedQueueState {
        queue: Arc::new(Mutex::new(PlaybackQueue::default())),
        source: Arc::new(Mutex::new(QueueSource::Unknown)),
    }
}

fn cold_player() -> Player {
    let (event_tx, _event_rx) = mpsc::channel::<PlayerEvent>();
    Player::new(
        String::new(),
        String::new(),
        false,
        false,
        true,
        false,
        SubtitlePrefs::default(),
        event_tx,
        None,
    )
}

fn recv_event(rx: &mpsc::Receiver<CtrlOutbound>) -> CtrlEvent {
    match rx.recv().unwrap() {
        CtrlOutbound::Event(json) => serde_json::from_str(&json).unwrap(),
        CtrlOutbound::Flush(_) => panic!("expected a control event"),
    }
}

/// Helper: builds a `PlaybackQueue` from a list of `EmbyItem`s with an active index.
fn queue_from_items(items: &[EmbyItem], active: usize) -> PlaybackQueue {
    let qi: Vec<QueueItem> = items
        .iter()
        .cloned()
        .map(|i| QueueItem::Emby(Box::new(i)))
        .collect();
    PlaybackQueue::from_queue_items(qi, Some(active))
}

#[test]
fn shutdown_notification_is_flushed_before_writers_are_released() {
    let mut clients = CtrlClients::default();
    let (_client_id, rx) = connect_client(&mut clients);
    let writer = std::thread::spawn(move || {
        match rx.recv().unwrap() {
            CtrlOutbound::Event(json) => {
                assert!(matches!(
                    serde_json::from_str::<CtrlEvent>(&json).unwrap(),
                    CtrlEvent::Disconnected {
                        reason: DisconnectReason::DaemonShutdown
                    }
                ));
            }
            CtrlOutbound::Flush(_) => panic!("shutdown event must precede the flush barrier"),
        }
        match rx.recv().unwrap() {
            CtrlOutbound::Flush(ack) => ack.send(()).unwrap(),
            CtrlOutbound::Event(_) => panic!("flush barrier must follow the shutdown event"),
        }
    });

    clients.notify_disconnected_all(DisconnectReason::DaemonShutdown);
    clients.flush_writers(Duration::from_secs(1));
    writer.join().unwrap();
}

#[test]
fn connecting_ctrl_client_becomes_driver_immediately() {
    let mut clients = CtrlClients::default();
    let (id, rx) = connect_client(&mut clients);
    assert!(clients.has_driver());
    assert!(clients.has_client(id));

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 55,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 55),
        _ => panic!("expected status update"),
    }
}

#[test]
fn emby_remote_takeover_notifies_ctrl_client_and_keeps_connection() {
    let mut clients = CtrlClients::default();
    let (driver_id, driver_rx) = connect_client(&mut clients);
    assert!(clients.has_client(driver_id));

    clients.take_authority_for_emby_remote();

    match recv_event(&driver_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    assert!(clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn ctrl_connect_during_emby_authority_does_not_override_authority() {
    let mut clients = CtrlClients::default();
    let (_old_id, old_rx) = connect_client(&mut clients);
    clients.take_authority_for_emby_remote();

    match recv_event(&old_rx) {
        CtrlEvent::Disconnected { reason } => {
            assert_eq!(reason, DisconnectReason::TakenOverByEmbyRemote);
        }
        _ => panic!("expected structured disconnect notification"),
    }
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let (new_id, new_rx) = connect_client(&mut clients);
    assert!(clients.has_client(new_id));
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);

    let registry = Arc::new(Mutex::new(clients));
    broadcast(
        &registry,
        &CtrlEvent::StatusOnly(PlayerStatus {
            volume: 66,
            ..PlayerStatus::default()
        }),
    );

    match recv_event(&old_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update on old client"),
    }
    match recv_event(&new_rx) {
        CtrlEvent::StatusOnly(status) => assert_eq!(status.volume, 66),
        _ => panic!("expected status update on new client"),
    }
}

#[test]
fn emby_remote_takeover_without_ctrl_client_still_records_authority() {
    let mut clients = CtrlClients::default();

    clients.take_authority_for_emby_remote();

    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}

#[test]
fn sole_client_disconnect_clears_registry_without_touching_playback() {
    let mut clients = CtrlClients::default();
    let (id, _rx) = connect_client(&mut clients);
    assert!(clients.has_driver());

    clients.remove(id);

    assert!(!clients.has_driver());
    assert!(!clients.has_client(id));
}

#[test]
fn cold_ctrl_player_command_keeps_connection_as_driver() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, _reply_rx) = mpsc::channel();
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::from(PlayerCommand::TogglePause)),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
    );
    let _queue = owner.core.queue;

    assert!(registry.lock().unwrap().has_driver());
    assert!(sender_rx.try_recv().is_err());
}

#[test]
fn unified_adopt_queue_seeds_status_without_starting_playback_when_cold() {
    let player = cold_player();
    let player_cmd_rx = player.spy_on_commands();
    let mut client = crate::api::EmbyClient::new(Config::default());
    client.token = "test-token".to_string();
    let client = Arc::new(Mutex::new(client));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (reply_tx, _reply_rx) = mpsc::channel();
    let queue = PlaybackQueue::default();
    let source = QueueSource::Unknown;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("adopted", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Remote,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
    );
    let queue = owner.core.queue;

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "adopted");
    assert!(!player.status.lock().unwrap().active);
    assert!(player_cmd_rx.try_recv().is_err());
}

#[test]
fn unified_adopt_queue_rejection_sends_authoritative_state_to_sole_client() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (_sender_id, _sender_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let (reply_tx, reply_rx) = mpsc::channel();
    let queue = queue_from_items(&[item("existing", "Video", "Movie")], 0);
    let source = QueueSource::Remote;
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    let mut owner = DaemonPlayerOwner { core: PlayerOwnerState::new(queue, source), ..Default::default() };
    handle_ctrl(
        CtrlCmd::UnifiedAdoptQueue {
            items: vec![emby_qi("stale", "Video", "Movie")],
            cursor: 0,
            source: QueueSource::Unknown,
        },
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
    );
    let queue = owner.core.queue;

    assert_eq!(queue.len(), 1);
    assert_eq!(queue.slots()[0].item.id(), "existing");
    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => {
            assert_eq!(reason, "daemon already has a queue; adoption skipped");
        }
        _ => panic!("expected command rejection"),
    }
    match recv_event(&reply_rx) {
        CtrlEvent::UnifiedQueueState(state) => {
            assert_eq!(
                state.slots.iter().map(|s| s.item.id()).collect::<Vec<_>>(),
                vec!["existing"]
            );
            assert_eq!(state.active_slot, Some(state.slots[0].slot_id));
        }
        _ => panic!("expected authoritative state resync"),
    }
}

#[test]
fn cold_websocket_noop_does_not_evict_ctrl_driver() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (driver_id, driver_rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;

    handle_ws(
        WsEvent::TogglePause,
        Some(&client),
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
    );

    let clients = registry.lock().unwrap();
    assert!(clients.has_client(driver_id));
    drop(clients);
    assert!(driver_rx.try_recv().is_err());
}

// ── design D6: stale identity is rejected, never repaired by position ────

#[test]
fn stale_client_jump_to_index_is_rejected_visibly() {
    let player = cold_player();
    let client = Arc::new(Mutex::new(crate::api::EmbyClient::new(Config::default())));
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (reply_tx, reply_rx) = mpsc::channel();
    let queue = queue_from_items(
        &[item("a", "Video", "Movie"), item("b", "Video", "Movie")],
        0,
    );
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };
    let (dummy_merged_tx, _dummy_rx) = mpsc::channel::<DaemonEvent>();

    handle_ctrl(
        CtrlCmd::PlayerCmd(WireCommand::JumpTo(1)),
        1,
        CtrlRequest {
            reply_tx: &reply_tx,
        },
        &client,
        &player,
        false,
        &mut owner,
        &shared_queue_state(),
        &registry,
        false,
        &dummy_merged_tx,
        false,
    );

    match recv_event(&reply_rx) {
        CtrlEvent::CommandRejected(reason) => assert!(reason.contains("index-addressed")),
        _ => panic!("expected a visible CommandRejected for an index-addressed jump"),
    }
    // The stale command is never repaired by position.
    assert_eq!(owner.core.queue.active_index(), Some(0));
    assert_eq!(owner.core.observed_active_slot(), None);
}

#[test]
fn stale_track_changed_report_leaves_queue_and_observed_slot_unchanged() {
    let queue = queue_from_items(
        &[item("a", "Video", "Movie"), item("b", "Video", "Movie")],
        1,
    );
    let mut owner = DaemonPlayerOwner {
        core: PlayerOwnerState::new(queue, QueueSource::Remote),
        ..Default::default()
    };

    // A genuine observation advances the observed active slot.
    let real = owner.core.queue.slots()[1].slot_id;
    assert_eq!(owner.core.observe_track_change(real), Some((1, real)));
    assert_eq!(owner.core.observed_active_slot(), Some(real));

    // A report naming a slot the owner no longer holds is discarded: the
    // caller (daemon_run's TrackChanged arm) emits nothing and canonical
    // queue + observed active slot are untouched (design D6, no clamp, no
    // neighbour fallback).
    let stale = crate::playback_queue::QueueSlotId::from_raw(9_999_999);
    assert!(owner.core.observe_track_change(stale).is_none());
    assert_eq!(owner.core.queue.active_slot_id(), Some(real));
    assert_eq!(owner.core.observed_active_slot(), Some(real));
}

#[test]
fn websocket_takeover_helper_records_emby_remote_authority() {
    let registry = Arc::new(Mutex::new(CtrlClients::default()));

    take_authority_for_emby_remote(&registry);

    let clients = registry.lock().unwrap();
    assert!(!clients.has_driver());
    assert_eq!(clients.authority, AuthorityHolder::EmbyRemote);
}
