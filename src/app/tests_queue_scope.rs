use super::*;
use crate::app::tests::*;
use mbv_core::ctrl::CtrlCmd;

#[test]
fn stale_remote_queue_scope_falls_back_to_local_when_not_in_direct_remote_mode() {
    let mut app = make_app_stub();
    app.remote_player_tab = Some(PlayerTab::from_emby_items(make_items(2), 1));
    app.queue_scope = QueueScope::Remote;

    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);

    app.set_queue_scope(QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
    assert_eq!(app.queue_scope, QueueScope::Local);
}

#[test]
fn queue_scope_resolution_matrix_without_remote_queue() {
    let mut app = make_app_stub();
    app.queue_scope = QueueScope::Local;

    assert!(!app.has_direct_remote_queue());
    assert_eq!(app.playing_queue_scope(), QueueScope::Local);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_stale_remote_scope_without_direct_remote() {
    let mut app = make_app_stub();
    app.remote_player_tab = Some(PlayerTab::from_emby_items(make_items(2), 0));
    app.queue_scope = QueueScope::Remote;

    assert!(!app.has_direct_remote_queue());
    assert_eq!(app.playing_queue_scope(), QueueScope::Local);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_direct_remote_displaying_local() {
    let local_items = make_items(1);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items, remote_items);
    app.queue_scope = QueueScope::Local;

    assert!(app.has_direct_remote_queue());
    assert_eq!(app.playing_queue_scope(), QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Local);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn queue_scope_resolution_matrix_direct_remote_displaying_remote() {
    let local_items = make_items(1);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items, remote_items);
    app.queue_scope = QueueScope::Remote;

    assert!(app.has_direct_remote_queue());
    assert_eq!(app.playing_queue_scope(), QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
    assert!(app.local_queue_metadata_applies(QueueScope::Local));
    assert!(!app.local_queue_metadata_applies(QueueScope::Remote));
}

#[test]
fn direct_remote_play_items_keeps_local_queue_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let replacement = make_items(4);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.queue_source = crate::config::QueueSource::Album;

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: replacement.clone(),
        start_idx: 2,
        source: crate::config::QueueSource::Shuffle,
        autostart: true,
    });

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Album
    ));
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);
}

#[test]
fn clearing_a_local_daemon_queue_replaces_the_daemon_queue_with_empty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(make_items(2), 0);
    let mut app = App::new_remote(
        mbv_core::api::EmbyClient::new(crate::config::Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
    );

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.player_tab.emby_items().is_empty());
    assert!(cmd_rx.try_iter().any(|cmd| {
        matches!(
            cmd,
            CtrlCmd::UnifiedQueueClear
        )
    }));
}

#[test]
fn local_daemon_queue_refresh_prune_reaches_daemon_via_unified_slot_command() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(make_items(3), 0);
    let mut app = App::new_remote(
        mbv_core::api::EmbyClient::new(crate::config::Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
    );
    app.player_tab.set_items(make_items(3), 0); // id0, id1, id2
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    while cmd_rx.try_recv().is_ok() {}

    // Background fetch no longer returns id1 -> its slot must be pruned from
    // the Stay-alive owner's queue. `RemotePlayer::send_command` rejects the
    // legacy index-addressed QueueRemove, so this must take the unified path.
    let fresh = vec![
        app.player_tab.emby_items()[0].clone(),
        app.player_tab.emby_items()[2].clone(),
    ];
    app.handle_lib_event(crate::app::LibEvent::QueueEnriched { items: fresh });

    assert!(
        cmd_rx
            .try_iter()
            .any(|cmd| matches!(cmd, CtrlCmd::UnifiedQueueRemoveSlot { .. })),
        "a library-refresh prune on a local-daemon owner must reach the daemon \
         queue via the unified slot command"
    );
}

#[test]
fn direct_remote_track_changes_do_not_clobber_local_last_played() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    app.last_played_item_id = Some(local_items[1].id.clone());
    app.last_played_completed = true;

    app.handle_player_event(PlayerEvent::TrackChanged { slot_id: app.playback_queue().resolve_slot_at(2).unwrap(), transition: None });

    assert_eq!(
        app.last_played_item_id.as_deref(),
        Some(local_items[1].id.as_str())
    );
    assert!(app.last_played_completed);
}

#[test]
fn command_rejected_flashes_the_daemon_supplied_reason() {
    let mut app = make_app_stub();

    app.handle_player_event(PlayerEvent::CommandRejected(
        "Daemon is running in audio-only mode; can't play video items".to_string(),
    ));

    assert_eq!(
        app.status,
        "Daemon is running in audio-only mode; can't play video items"
    );
}
