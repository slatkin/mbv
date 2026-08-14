#[test]
fn packaged_startup_context_is_service_independent() {
    let startup = DaemonStartupContext::new(Config::default(), DaemonRole::Packaged);
    assert_eq!(startup.role, DaemonRole::Packaged);
    assert!(startup.emby.is_none());
    assert!(startup.audiobookshelf.is_none());
}

#[test]
fn audiobookshelf_owner_context_credential_is_never_serializable() {
    static_assertions::assert_not_impl_any!(
        AudiobookshelfOwnerContext: serde::Serialize,
        std::fmt::Debug
    );
}

#[test]
fn audiobookshelf_reconciliation_installs_context_but_keeps_admission_disabled() {
    let _guard = crate::config::TestStateDirGuard::new();
    crate::config::persist_audiobookshelf_setup_and_secret(
        &crate::config::AudiobookshelfSetup::new("https://books.example"),
        "owner-secret",
    )
    .unwrap();

    let item = abs_qi("library-a", "episode-1");
    assert!(!daemon_admits(&item, false, false));

    let mut current = None;
    reconcile_packaged_audiobookshelf(1, &mut current).unwrap();
    assert!(
        current.is_some(),
        "matching revision installs the Audiobookshelf owner context"
    );

    assert!(
        !daemon_admits(&item, false, false),
        "reconciliation must not enable Audiobookshelf admission"
    );
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
fn owner_administration_is_local_transport_only() {
    assert!(owner_admin_transport_allowed(
        DaemonRole::Packaged,
        Some(CtrlTransport::Local)
    ));
    assert!(owner_admin_transport_allowed(
        DaemonRole::Local,
        Some(CtrlTransport::Local)
    ));
    assert!(!owner_admin_transport_allowed(
        DaemonRole::Packaged,
        Some(CtrlTransport::Tcp)
    ));
    assert!(!owner_admin_transport_allowed(
        DaemonRole::Local,
        Some(CtrlTransport::Tcp)
    ));
    assert!(!owner_admin_transport_allowed(DaemonRole::Packaged, None));
    assert!(!owner_admin_transport_allowed(DaemonRole::Local, None));
}

#[test]
fn audiobookshelf_reconciliation_rejects_revision_mismatch_without_state_change() {
    let _guard = crate::config::TestStateDirGuard::new();
    crate::config::persist_audiobookshelf_setup_and_secret(
        &crate::config::AudiobookshelfSetup::new("https://books.example"),
        "owner-secret",
    )
    .unwrap();

    let mut current = None;
    reconcile_packaged_audiobookshelf(1, &mut current).unwrap();
    assert!(current.is_some(), "matching revision must install context");
    let pre = current.as_ref().unwrap().generation;

    let result = reconcile_packaged_audiobookshelf(2, &mut current);
    assert!(
        matches!(result, Err(ServiceSetupRejection::RevisionMismatch)),
        "mismatched revision must be rejected, got {result:?}"
    );
    assert_eq!(
        current.as_ref().unwrap().generation,
        pre,
        "a rejected reconciliation must not change the installed runtime"
    );
}

#[test]
fn audiobookshelf_reconciliation_reports_storage_unavailable_without_state_change() {
    let _guard = crate::config::TestStateDirGuard::new();
    crate::config::persist_audiobookshelf_setup_and_secret(
        &crate::config::AudiobookshelfSetup::new("https://books.example"),
        "owner-secret",
    )
    .unwrap();

    let mut current = None;
    reconcile_packaged_audiobookshelf(1, &mut current).unwrap();
    assert!(current.is_some());
    let pre = current.as_ref().unwrap().generation;

    // Drop the Service secret so the owner context can no longer be loaded.
    crate::config::clear_service_secret(crate::config::ServiceKind::Audiobookshelf);

    let result = reconcile_packaged_audiobookshelf(1, &mut current);
    assert!(
        matches!(result, Err(ServiceSetupRejection::StorageUnavailable)),
        "unreadable storage must be rejected, got {result:?}"
    );
    assert_eq!(
        current.as_ref().unwrap().generation,
        pre,
        "a rejected reconciliation must not change the installed runtime"
    );
}

#[test]
fn audiobookshelf_reconciliation_drops_context_when_setup_is_absent() {
    let _guard = crate::config::TestStateDirGuard::new();
    crate::config::persist_audiobookshelf_setup_and_secret(
        &crate::config::AudiobookshelfSetup::new("https://books.example"),
        "owner-secret",
    )
    .unwrap();

    let mut current = None;
    reconcile_packaged_audiobookshelf(1, &mut current).unwrap();
    assert!(
        current.is_some(),
        "setup must install context before removal"
    );

    crate::config::remove_audiobookshelf_setup_and_secret().unwrap();
    reconcile_packaged_audiobookshelf(1, &mut current).unwrap();
    assert!(
        current.is_none(),
        "removal signal must drop the Audiobookshelf owner context"
    );
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
    daemon_admits, owner_admin_transport_allowed, reconcile_packaged_audiobookshelf,
    AudiobookshelfOwnerContext, DaemonRole, DaemonStartupContext, EmbyOwnerContext,
};
use crate::ctrl::ServiceSetupRejection;
