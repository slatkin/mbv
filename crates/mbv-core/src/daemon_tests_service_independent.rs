#[test]
fn packaged_startup_context_is_service_independent() {
    let startup = DaemonStartupContext::new(Config::default(), DaemonRole::Packaged);
    assert_eq!(startup.role, DaemonRole::Packaged);
    assert!(startup.emby.is_none());
}

#[test]
fn packaged_context_loads_unreachable_emby_without_authenticating() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut config = Config::default();
    config.emby_setup = Some(crate::config::EmbySetup::new(
        "http://127.0.0.1:1",
        "owner-user",
    ));
    crate::config::save_service_secret(crate::config::ServiceKind::Emby, "unreachable-token")
        .unwrap();
    let owner =
        EmbyOwnerContext::from_packaged_storage_result(&config).expect("owner context loads");
    assert_eq!(owner.revision, 1);
    assert_eq!(
        owner.client.lock().unwrap().config.server_url,
        "http://127.0.0.1:1"
    );
}

#[test]
fn emby_absence_keeps_feed_admission_and_rejects_emby_admission() {
    let feed = QueueItem::Feed(FeedEntry {
        guid: "feed-1".into(),
        title: "Episode".into(),
        enclosure_url: Some("https://example.test/episode.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(crate::config::FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    });
    assert!(daemon_admits(&feed, false, false));
    assert!(!daemon_admits(
        &emby_qi("old", "Video", "Movie"),
        false,
        false
    ));
}

#[test]
fn absent_emby_websocket_is_a_noop_for_ctrl_and_queue_state() {
    let player = cold_player();
    let registry = Arc::new(Mutex::new(CtrlClients::default()));
    let (id, rx) = {
        let mut clients = registry.lock().unwrap();
        connect_client(&mut clients)
    };
    let mut queue = PlaybackQueue::default();
    let mut source = QueueSource::Unknown;
    handle_ws(
        WsEvent::TogglePause,
        None,
        &player,
        false,
        &mut queue,
        &mut source,
        &shared_queue_state(),
        &registry,
    );
    assert!(registry.lock().unwrap().has_client(id));
    assert!(rx.try_recv().is_err());
    assert!(queue.is_empty());
}

#[test]
fn owner_administration_is_packaged_local_only() {
    assert!(owner_admin_transport_allowed(
        DaemonRole::Packaged,
        Some(CtrlTransport::Local)
    ));
    assert!(!owner_admin_transport_allowed(
        DaemonRole::Packaged,
        Some(CtrlTransport::Tcp)
    ));
    assert!(!owner_admin_transport_allowed(
        DaemonRole::Local,
        Some(CtrlTransport::Local)
    ));
    assert!(!owner_admin_transport_allowed(DaemonRole::Packaged, None));
}

#[test]
fn every_setup_rejection_reason_is_wire_representable() {
    for reason in [
        ServiceSetupRejection::UnsupportedService,
        ServiceSetupRejection::RevisionMismatch,
        ServiceSetupRejection::StorageUnavailable,
        ServiceSetupRejection::TransitionRejected,
    ] {
        let event = CtrlEvent::ServiceSetupRejected {
            kind: crate::config::ServiceKind::Emby,
            revision: 4,
            reason,
        };
        let decoded: CtrlEvent =
            serde_json::from_str(&serde_json::to_string(&event).unwrap()).unwrap();
        assert!(
            matches!(decoded, CtrlEvent::ServiceSetupRejected { reason: decoded_reason, .. } if decoded_reason == reason)
        );
    }
}
use super::{
    daemon_admits, owner_admin_transport_allowed, DaemonRole, DaemonStartupContext,
    EmbyOwnerContext,
};
use crate::ctrl::ServiceSetupRejection;
