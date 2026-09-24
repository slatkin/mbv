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

fn local_owner_app() -> (App, std::sync::mpsc::Receiver<CtrlCmd>) {
    let (remote, _events, commands) =
        mbv_core::remote_player::RemotePlayer::stub_owner_queue_load_with_command_rx(
            make_items(2),
            0,
        );
    let app = App::new_remote(
        mbv_core::api::EmbyClient::new(crate::config::Config::default()),
        remote,
        _events,
        mbv_core::remote_player::DaemonEndpoint::Local,
    );
    while commands.try_recv().is_ok() {}
    (app, commands)
}

#[test]
fn local_owner_idle_load_during_playback_sends_without_local_replacement() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, commands) = local_owner_app();
    let before = make_items(2);
    app.player_tab.set_items(before.clone(), 1);
    app.player_tab.sequence_generation = 17;
    app.queue_source = crate::config::QueueSource::Album;
    app.player.status.lock().unwrap().active = true;

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: make_items(3),
        start_idx: 1,
        source: crate::config::QueueSource::Shuffle,
        autostart: false,
    });

    let CtrlCmd::UnifiedQueueLoadIdle {
        request_id,
        slots,
        cursor,
        source,
    } = commands.try_recv().unwrap()
    else {
        panic!("expected owner idle load command");
    };
    assert_eq!(slots.len(), 3);
    assert_eq!(cursor, 1);
    assert_eq!(source, crate::config::QueueSource::Shuffle);
    assert_eq!(app.status, "");
    app.handle_player_event(PlayerEvent::UnifiedQueueLoadResult {
        request_id,
        result: mbv_core::ctrl::QueueLoadResult::Accepted,
    });
    assert_eq!(app.status, "Queue load accepted");
    assert_eq!(app.player_tab.emby_items(), before);
    assert_eq!(app.player_tab.sequence_generation, 17);
    assert_eq!(app.queue_source, crate::config::QueueSource::Album);
    assert!(app.player.status.lock().unwrap().active);
    assert_ne!(app.status, "Loaded: id1");
}

#[test]
fn local_owner_empty_idle_load_is_sent_without_clearing_confirmed_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, commands) = local_owner_app();
    let before = make_items(2);
    app.player_tab.set_items(before.clone(), 0);
    app.queue_source = crate::config::QueueSource::Album;

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: Vec::new(),
        start_idx: 0,
        source: crate::config::QueueSource::Shuffle,
        autostart: false,
    });

    assert!(
        matches!(commands.try_recv(), Ok(CtrlCmd::UnifiedQueueLoadIdle { slots, .. }) if slots.is_empty())
    );
    assert_eq!(app.player_tab.emby_items(), before);
    assert_eq!(app.queue_source, crate::config::QueueSource::Album);
}

#[test]
fn local_owner_idle_load_rejection_is_reported_without_local_mutation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, commands) = local_owner_app();
    let before = make_items(2);
    app.player_tab.set_items(before.clone(), 0);
    app.queue_source = crate::config::QueueSource::Album;

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: make_items(1),
        start_idx: 0,
        source: crate::config::QueueSource::Shuffle,
        autostart: false,
    });
    let CtrlCmd::UnifiedQueueLoadIdle { request_id, .. } = commands.try_recv().unwrap() else {
        panic!("expected owner idle load command");
    };
    app.handle_player_event(PlayerEvent::UnifiedQueueLoadResult {
        request_id,
        result: mbv_core::ctrl::QueueLoadResult::Rejected {
            reason: "owner busy".into(),
        },
    });

    assert_eq!(app.player_tab.emby_items(), before);
    assert_eq!(app.queue_source, crate::config::QueueSource::Album);
    assert!(app.status.contains("owner busy"));
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Error
    );
}

#[test]
fn local_owner_disconnect_before_idle_load_acceptance_is_unknown_without_local_mutation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, commands) = local_owner_app();
    let before = make_items(2);
    app.player_tab.set_items(before.clone(), 0);
    app.player
        .disconnected_flag()
        .unwrap()
        .store(true, std::sync::atomic::Ordering::SeqCst);

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: make_items(1),
        start_idx: 0,
        source: crate::config::QueueSource::Shuffle,
        autostart: false,
    });

    assert!(matches!(
        commands.try_recv(),
        Err(std::sync::mpsc::TryRecvError::Empty)
    ));
    assert_eq!(app.player_tab.emby_items(), before);
    assert_eq!(
        app.status,
        crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE
    );
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Warning
    );
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
fn local_daemon_play_submits_intended_source_and_adopts_owner_snapshot_source() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, commands) =
        crate::app::tests::make_local_daemon_app_stub_with_cmd_rx(make_items(1));
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("qixl".into()),
        name: "QIXL".into(),
    };
    let intended_source = crate::config::QueueSource::Playlist {
        id: Some("taskmaster".into()),
        name: "Taskmaster".into(),
    };

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: make_items(2),
        start_idx: 0,
        source: intended_source.clone(),
        autostart: true,
    });

    let sent_source = commands.try_iter().find_map(|command| match command {
        CtrlCmd::UnifiedQueueReplace { source, .. } => Some(source),
        _ => None,
    });
    assert_eq!(sent_source, Some(intended_source));

    let owner_source = crate::config::QueueSource::Playlist {
        id: Some("owner-minted".into()),
        name: "Owner playlist".into(),
    };
    let mut snapshot = crate::app::tests::emby_unified_state(&make_items(2), 0);
    snapshot.source = owner_source.clone();
    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(snapshot)));
    assert_eq!(app.queue_source, owner_source);
}

#[test]
fn disconnected_remote_rejects_tab_queue_submission_with_warning() {
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    app.player
        .disconnected_flag()
        .unwrap()
        .store(true, std::sync::atomic::Ordering::SeqCst);

    assert!(!app.submit_tab_queue(QueueScope::Remote, 0, app.queue_source.clone()));

    assert_eq!(
        app.status,
        crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE
    );
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Warning
    );
}

#[test]
fn deferred_play_on_disconnected_remote_does_not_replace_queue_or_acknowledge() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(1), make_items(2));
    let before = app.remote_player_tab.as_ref().unwrap().emby_items();
    app.player
        .disconnected_flag()
        .unwrap()
        .store(true, std::sync::atomic::Ordering::SeqCst);

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: make_items(3),
        start_idx: 1,
        source: crate::config::QueueSource::Shuffle,
        autostart: true,
    });

    assert_eq!(app.remote_player_tab.as_ref().unwrap().emby_items(), before);
    assert_eq!(
        app.status,
        crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE
    );
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Warning
    );
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
    assert!(cmd_rx
        .try_iter()
        .any(|cmd| { matches!(cmd, CtrlCmd::UnifiedQueueClear) }));
}

#[test]
fn local_daemon_play_uses_owner_slot_despite_generation_mismatch() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) =
        crate::app::tests::make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    app.player_tab.sequence_generation = 8;
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
        status.sequence_generation = 0;
    }
    let owner_slot = app.player_tab.slots()[1].slot_id;

    app.dispatch(crate::app::dispatch::action::Command::QueuePlayCursor(1));

    let commands: Vec<_> = cmd_rx.try_iter().collect();
    let requested_owner_slot = commands.iter().any(|cmd| {
        matches!(
            cmd,
            CtrlCmd::UnifiedQueuePlaySlot { slot_id }
                if *slot_id == mbv_core::ctrl::slot_id_to_u64(owner_slot)
        )
    });
    assert!(
        requested_owner_slot,
        "expected owner slot {owner_slot:?}; local-daemon={}, owner-queue={}, playback-scope={}",
        app.is_local_daemon(),
        app.local_queue_is_owner_queue(crate::app::QueueScope::Local),
        app.queue_scope_is_playback(crate::app::QueueScope::Local),
    );
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

    app.handle_player_event(PlayerEvent::TrackChanged {
        slot_id: app.playback_queue().resolve_slot_at(2).unwrap(),
        transition: None,
    });

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
