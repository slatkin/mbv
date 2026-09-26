use crate::app::tests::*;
use mbv_core::playback_queue::QueueItem;

#[test]
fn attached_session_remove_stays_local_and_does_not_replay_daemon_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut daemon_items = make_items(2);
    for (index, item) in daemon_items.iter_mut().enumerate() {
        item.id = format!("a{index}");
    }
    let mut client_items = make_items(3);
    for (index, item) in client_items.iter_mut().enumerate() {
        item.id = format!("b{index}");
    }
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(client_items, daemon_items);
    app.set_queue_scope(QueueScope::Local);
    app.connected_session_id = Some("session".into());
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-b".into()),
        name: "Playlist B".into(),
    };
    app.player.status.lock().unwrap().active = true;
    app.player.status.lock().unwrap().current_idx = 1;

    app.remove_from_queue(0);

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b1", "b2"]
    );
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { ref id, ref name }
            if id.as_deref() == Some("playlist-b") && name == "Playlist B"
    ));
    cmd_rx.try_recv().unwrap_err();
}

#[test]
fn attached_session_move_stays_local_without_owner_command() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut client_items = make_items(3);
    for (index, item) in client_items.iter_mut().enumerate() {
        item.id = format!("b{index}");
    }
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(client_items, make_items(2));
    app.connected_session_id = Some("session".into());
    app.player.status.lock().unwrap().active = true;

    assert!(app.apply_queue_move(QueueScope::Local, 0, 1));

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["b1", "b0", "b2"]
    );
    cmd_rx.try_recv().unwrap_err();
}

#[test]
fn attached_session_append_stays_local_without_rollback_or_owner_command() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut client_items = make_items(2);
    for (index, item) in client_items.iter_mut().enumerate() {
        item.id = format!("b{index}");
    }
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(client_items, make_items(2));
    app.set_queue_scope(QueueScope::Local);
    app.connected_session_id = Some("session".into());
    let status_before = app.status.clone();

    assert!(app.submit_queue_item(QueueItem::Emby(Box::new(make_item("b2", "Movie"))), false,));

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Item 0", "Item 1", "b2"]
    );
    assert_eq!(app.status, status_before);
    cmd_rx.try_recv().unwrap_err();
}

#[test]
fn enqueue_on_disconnected_remote_rolls_back_and_shows_connection_lost_error() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, _) = make_remote_app_stub_with_cmd_rx(make_items(2), make_items(3));
    app.set_queue_scope(QueueScope::Remote);
    app.player
        .disconnected_flag()
        .unwrap()
        .store(true, std::sync::atomic::Ordering::SeqCst);

    assert!(!app.submit_queue_item(QueueItem::Emby(Box::new(make_item("new", "Movie"))), false,));

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id0", "id1", "id2"]
    );
    assert_eq!(
        app.status,
        crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE
    );
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Error
    );
}

#[test]
fn playback_submission_on_disconnected_remote_keeps_queue_and_warns() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, _) = make_remote_app_stub_with_cmd_rx(make_items(2), make_items(3));
    app.set_queue_scope(QueueScope::Remote);
    app.player
        .disconnected_flag()
        .unwrap()
        .store(true, std::sync::atomic::Ordering::SeqCst);

    assert!(!app.submit_queue_item(QueueItem::Emby(Box::new(make_item("new", "Movie"))), true,));

    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().emby_items().len(),
        3
    );
    assert_eq!(
        app.status,
        crate::app::dispatch::actions::CONNECTION_LOST_MESSAGE
    );
    assert_eq!(
        app.status_severity,
        crate::app::dispatch::notify::ToastSeverity::Warning
    );
}
