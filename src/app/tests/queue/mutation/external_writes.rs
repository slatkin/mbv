use crate::app::tests::*;
use mbv_core::playback_queue::QueueItem;

/// Task 5.3d, Home typed-effect keyboard ownership: the Ctrl+A chord is
/// component-owned and reaches this effect as `ShellRequest::HomeEnqueue`
/// (see `home_component_tests` + `shell_home_effects`); the App boundary is
/// `App::home_enqueue_target`, which enqueues the shell-resolved CW item
/// immediately. Home content is Model-owned, so the test supplies the item
/// directly (the flat-cursor resolution is covered at the Model boundary).
#[test]
fn home_enqueue_from_home_view_applies_immediately() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let item = make_items(1).remove(0);

    app.home_enqueue_target(QueueItem::Emby(Box::new(item)), true);

    assert_eq!(app.player_tab.emby_items().len(), 1);
    assert_eq!(app.player_tab.emby_items()[0].id, "id0");
}

#[test]
fn home_enqueue_appends_to_direct_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(local_items, remote_items.clone());
    app.queue_scope = QueueScope::Remote;
    let item = make_items(1).remove(0);

    // The shell resolves the CW item at the Model boundary (task 5.3d); the
    // App effect receives it directly.
    app.home_enqueue_target(QueueItem::Emby(Box::new(item)), true);

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
            .chain(std::iter::once("id0"))
            .collect::<Vec<_>>()
    );
    assert!(matches!(
        cmd_rx.try_recv(),
        Ok(mbv_core::ctrl::CtrlCmd::UnifiedQueueAppend { items })
            if items.len() == 1 && items[0].id() == "id0"
    ));
    assert!(
        cmd_rx.try_recv().is_err(),
        "Ctrl+A append must not follow UnifiedQueueAppend with queue replacement"
    );
}

#[test]
fn queue_edit_forwards_to_local_daemon_while_daemon_is_idle() {
    // Reproduces: attaching to a tracked remote Emby session (which never
    // touches `self.player`) while the local daemon that owns this queue
    // isn't itself playing anything (`active == false`). Queue edits must
    // still reach the daemon over ctrl, or its authoritative copy diverges
    // from what the client shows and re-adopting it on the next launch
    // resurrects deleted items.
    let _guard = crate::config::TestStateDirGuard::new();
    use crate::config::Config;
    use mbv_core::api::EmbyClient;
    let (remote, player_rx, cmd_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(vec![], 0);
    let mut app = App::new_remote(
        EmbyClient::new(Config::default()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
    );
    app.player_tab
        .set_items(make_items(3), app.player_tab.queue_cursor);
    app.player_tab.queue_cursor = 0;
    app.player.status.lock().unwrap().active = false;
    assert!(app.remote_player_tab.is_none());
    // Drain the subtitle-prefs sync `App::new_remote` sends on attach so it
    // isn't mistaken for the removal's own command below.
    while cmd_rx.try_recv().is_ok() {}

    app.remove_from_queue(1);

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id0", "id2"]
    );
    // Unified-capable remote peer: removal is sent as UnifiedQueueRemoveSlot.
    let cmd = cmd_rx.try_recv().unwrap();
    assert!(
        matches!(cmd, mbv_core::ctrl::CtrlCmd::UnifiedQueueRemoveSlot { .. }),
        "expected UnifiedQueueRemoveSlot"
    );
}

#[test]
fn local_daemon_queue_delete_is_forwarded_by_owner_slot_id() {
    let _guard = crate::config::TestStateDirGuard::new();
    // The client has a replacement queue which the owner has not yet
    // broadcast. Slot edits still target owner slot IDs and are forwarded.
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    let taskmaster = make_items(2);
    app.replace_playback_queue(taskmaster.clone(), 0);
    // Drain the attach-time sync commands so only the delete's own traffic
    // is observable below.
    while cmd_rx.try_recv().is_ok() {}

    app.remove_from_queue(1);

    assert!(
        cmd_rx
            .try_iter()
            .any(|cmd| matches!(cmd, mbv_core::ctrl::CtrlCmd::UnifiedQueueRemoveSlot { .. })),
        "the slot edit must reach the authoritative owner"
    );
    assert_eq!(
        app.player_tab.emby_items().len(),
        1,
        "the removal still applies to the local canonical queue"
    );
    assert_eq!(app.player_tab.emby_items()[0].id, taskmaster[0].id);
}

#[test]
fn local_daemon_owner_broadcast_replaces_client_queue_and_source() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, _) = make_local_daemon_app_stub_with_cmd_rx(make_items(3));
    let mut other = make_local_daemon_app_stub(make_items(4));
    let taskmaster = make_items(2);
    app.replace_playback_queue(taskmaster.clone(), 0);
    other.replace_playback_queue(taskmaster, 0);
    for client in [&app, &other] {
        let mut status = client.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 1;
    }

    let replacement = make_items(3);
    let mut owner = emby_unified_state(&replacement, 0);
    owner.status.active = false;
    owner.source = crate::config::QueueSource::Shuffle;
    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(owner.clone())));
    other.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(owner)));

    for client in [&app, &other] {
        assert_eq!(
            client.player_tab.emby_items()[0].id,
            replacement[0].id,
            "each client adopts the owner's stopped replacement"
        );
        assert_eq!(client.queue_source, crate::config::QueueSource::Shuffle);
        assert!(!client.queue_row_playback_state().active);
    }
}

#[test]
fn local_daemon_play_waits_for_owner_snapshot_to_adopt_source() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, _) = make_local_daemon_app_stub_with_cmd_rx(make_items(2));
    let previous_source = crate::config::QueueSource::Album;
    app.queue_source = previous_source.clone();

    app.execute_pending_queue_action(PendingQueueAction::PlayItems {
        items: make_items(3),
        start_idx: 1,
        source: crate::config::QueueSource::Shuffle,
        autostart: true,
    });

    assert_eq!(
        app.queue_source, previous_source,
        "play must not predict owner source"
    );
    let mut owner = emby_unified_state(&app.player_tab.emby_items(), 1);
    owner.source = crate::config::QueueSource::Shuffle;
    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(owner)));
    assert_eq!(app.queue_source, crate::config::QueueSource::Shuffle);
}

#[test]
fn local_daemon_clear_waits_for_owner_snapshot_to_adopt_source() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_local_daemon_app_stub(make_items(2));
    let previous_source = crate::config::QueueSource::Album;
    app.queue_source = previous_source.clone();

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert_eq!(
        app.queue_source, previous_source,
        "clear must not predict owner source"
    );
    let mut owner = emby_unified_state(&[], 0);
    owner.source = crate::config::QueueSource::Unknown;
    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(owner)));
    assert_eq!(app.queue_source, crate::config::QueueSource::Unknown);
}

#[test]
fn local_daemon_queue_broadcast_adopts_owner_source() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_local_daemon_app_stub(make_items(4));
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl-taskmaster".into()),
        name: "Taskmaster".into(),
    };

    // The owner snapshot is the displayed source authority for Stay-alive.
    let mut unified = emby_unified_state(&app.player_tab.emby_items(), 0);
    unified.source = crate::config::QueueSource::Playlist {
        id: Some("pl-qixl".into()),
        name: "QIXL".into(),
    };
    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(unified)));

    assert_eq!(
        app.queue_source,
        crate::config::QueueSource::Playlist {
            id: Some("pl-qixl".into()),
            name: "QIXL".into(),
        },
        "the owner snapshot must set the displayed queue source"
    );
}

#[test]
fn removing_non_active_item_keeps_cursor_off_now_playing_after_daemon_ack() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_local_daemon_app_stub(make_items(4));
    app.player.status.lock().unwrap().active = true;
    // Track 0 is playing; the user has selected track 2 in the queue view.
    app.player_tab.queue_cursor = 2;

    app.remove_from_queue(2);

    // Simulate the daemon's async ack of the removal: its `active_slot` reports
    // the playback position (still track 0), not the UI's selection.
    app.handle_player_event(PlayerEvent::UnifiedQueueUpdated(Box::new(
        emby_unified_state(&app.player_tab.emby_items(), 0),
    )));

    assert_eq!(
        app.player_tab.queue_cursor, 2,
        "deleting a non-playing item must not snap the display cursor onto \
         the now-playing item"
    );
}
