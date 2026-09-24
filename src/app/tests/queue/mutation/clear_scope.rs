use crate::app::tests::*;

#[test]
fn clearing_local_queue_in_direct_remote_mode_leaves_remote_queue_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items, remote_items.clone());
    app.set_queue_scope(QueueScope::Local);
    app.queue_source = crate::config::QueueSource::Album;
    app.queue_dirty = true;

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.player_tab.emby_items().is_empty());
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
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Unknown
    ));
    assert!(!app.queue_dirty);
}

#[test]
fn clearing_remote_queue_in_direct_remote_mode_leaves_local_queue_metadata_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app
        .remote_player_tab
        .as_ref()
        .unwrap()
        .emby_items()
        .is_empty());
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
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { .. }
    ));
    assert!(app.queue_dirty);
}

/// `c` (`request_clear_queue`) must raise the confirmation for a direct-remote
/// daemon queue -- the queue lives in `remote_player_tab`, so the old
/// `self.player_tab.total_queue_len() == 0` check (the always-empty local queue)
/// silently swallowed the key. Clearing itself is already supported
/// (`execute_pending_queue_action` -> unified queue clear).
#[test]
fn clear_queue_prompt_opens_for_direct_remote_daemon_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    // Empty local queue is the real socket-attached-mbvd state: the queue lives
    // only in `remote_player_tab`. Pre-fix this made the old `player_tab` check
    // early-return, so this test fails without the fix.
    let mut app = make_remote_app_stub(Vec::new(), make_items(3));
    app.set_queue_scope(QueueScope::Remote);
    assert_eq!(app.viewed_queue_scope(), QueueScope::Remote);

    app.request_clear_queue();

    assert!(
        matches!(
            app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
                _
            ))
        ),
        "clear-queue confirmation must open for a direct-remote daemon queue"
    );
}

/// A connected Emby session owns its queue on the remote device; `c` still
/// refuses it with the explanatory toast rather than opening the prompt.
#[test]
fn clear_queue_prompt_refused_for_connected_session_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_remote_app_stub(make_items(2), make_items(3));
    app.set_queue_scope(QueueScope::Remote);
    app.connected_session_id = Some("session".into());

    app.request_clear_queue();

    assert!(!matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
            _
        ))
    ));
}

#[test]
fn removing_from_local_queue_in_direct_remote_mode_does_not_touch_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Local);

    app.remove_from_queue(1);

    assert_eq!(app.player_tab.emby_items().len(), 2);
    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![local_items[0].id.as_str(), local_items[2].id.as_str()]
    );
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
    assert!(app.queue_dirty);
    assert_eq!(app.remote_queue_undo_stack.len(), 0);
}

#[test]
fn removing_from_remote_queue_in_direct_remote_mode_does_not_touch_local_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());

    app.remove_from_queue(1);

    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().emby_items().len(),
        2
    );
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![remote_items[0].id.as_str(), remote_items[2].id.as_str()]
    );
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
    assert!(!app.queue_dirty);
    assert_eq!(app.queue_undo_stack.len(), 0);
    assert_eq!(app.remote_queue_undo_stack.len(), 1);
}

#[test]
fn clearing_remote_queue_does_not_prompt_to_save_local_playlist() {
    let mut app = make_remote_app_stub(make_items(2), make_items(3));
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Saved".into(),
    };
    app.queue_dirty = true;

    app.replace_queue_or_prompt(PendingQueueAction::ClearQueue);

    assert!(!matches!(
        app.pending_overlay,
        Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
            _
        ))
    ));
    assert!(app.pending_queue_action.is_none());
    assert!(app
        .remote_player_tab
        .as_ref()
        .unwrap()
        .emby_items()
        .is_empty());
    assert!(app.queue_dirty);
}
