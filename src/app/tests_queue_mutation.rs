use super::*;
use crate::app::tests::*;
use mbv_core::playback_queue::QueueItem;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[cfg(test)]
#[path = "tests_queue_mutation_playlist_save.rs"]
mod tests_queue_mutation_playlist_save;

#[test]
fn canceled_active_item_removal_leaves_queue_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(3), 1);
    app.player.status.lock().unwrap().active = true;
    app.player.status.lock().unwrap().current_idx = 1;
    app.remove_from_queue(1);
    assert!(app.pending_overlay.is_some());
    let mut model = Model::new(app);
    model.sync_modal_requests();
    model.handle_confirm_key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    let app = &model.app;

    assert!(app.pending_overlay.is_none());
    assert_eq!(app.player_tab.emby_items().len(), 3);
}

#[test]
fn confirmed_active_item_removal_removes_item_after_confirmation() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(3), 1);
    app.player.status.lock().unwrap().active = true;
    app.player.status.lock().unwrap().current_idx = 1;
    app.remove_from_queue(1);
    let mut model = Model::new(app);
    model.sync_modal_requests();
    model.handle_confirm_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let app = &model.app;

    assert_eq!(app.player_tab.emby_items().len(), 2);
}

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
fn queue_bulk_removal_sends_one_owner_edit_for_the_whole_range() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(make_items(2), make_items(5));
    // The remote tab is the playing queue; edits reach the owner over ctrl.
    let (first, second) = {
        let tab = app.remote_player_tab.as_ref().unwrap();
        (tab.slot_id_at(1).unwrap(), tab.slot_id_at(2).unwrap())
    };

    app.remove_slots_from_queue(QueueScope::Remote, &[first, second]);

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id0", "id3", "id4"]
    );
    // One batch command, not one per removed slot.
    match cmd_rx.try_recv() {
        Ok(mbv_core::ctrl::CtrlCmd::UnifiedQueueRemoveSlots { slot_ids }) => {
            assert_eq!(
                slot_ids,
                vec![
                    mbv_core::ctrl::slot_id_to_u64(first),
                    mbv_core::ctrl::slot_id_to_u64(second),
                ]
            );
        }
        _ => panic!("expected one UnifiedQueueRemoveSlots"),
    }
    assert!(
        cmd_rx.try_recv().is_err(),
        "bulk removal must not also send per-slot commands"
    );
}

#[test]
fn shell_bulk_remove_request_reaches_the_owner_as_one_edit() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (app, cmd_rx) = make_remote_app_stub_with_cmd_rx(make_items(2), make_items(4));
    let mut model = Model::new(app);
    model.app.set_queue_scope(QueueScope::Remote);
    let (second, third) = {
        let tab = model.app.remote_player_tab.as_ref().unwrap();
        (tab.slot_id_at(1).unwrap(), tab.slot_id_at(2).unwrap())
    };

    model.handle_queue_request(crate::app::components::QueueRequest::RemoveSelection {
        scope: QueueScope::Remote,
        slot_ids: vec![second, third],
    });

    assert_eq!(
        model
            .app
            .remote_player_tab
            .as_ref()
            .unwrap()
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id0", "id3"]
    );
    match cmd_rx.try_recv() {
        Ok(mbv_core::ctrl::CtrlCmd::UnifiedQueueRemoveSlots { slot_ids }) => {
            assert_eq!(
                slot_ids,
                vec![
                    mbv_core::ctrl::slot_id_to_u64(second),
                    mbv_core::ctrl::slot_id_to_u64(third),
                ]
            );
        }
        _ => panic!("expected one UnifiedQueueRemoveSlots"),
    }
    assert!(
        cmd_rx.try_recv().is_err(),
        "the shell handler must not also send per-slot commands"
    );
}

#[test]
fn bulk_removal_rests_the_cursor_on_the_item_before_the_range() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(5), 0);
    // Cursor is deliberately far from the range: the rule is about the range,
    // not about preserving wherever the user was.
    app.player_tab.queue_cursor = 4;
    let (a, b) = {
        let tab = &app.player_tab;
        (tab.slot_id_at(1).unwrap(), tab.slot_id_at(2).unwrap())
    };

    app.remove_slots_from_queue(QueueScope::Local, &[a, b]);

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id0", "id3", "id4"]
    );
    assert_eq!(app.player_tab.queue_cursor, 0, "item before the range");
}

#[test]
fn bulk_removal_from_the_queue_head_rests_the_cursor_on_the_first_survivor() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(5), 0);
    app.player_tab.queue_cursor = 3;
    let head: Vec<_> = (0..2)
        .map(|index| app.player_tab.slot_id_at(index).unwrap())
        .collect();

    app.remove_slots_from_queue(QueueScope::Local, &head);

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id2", "id3", "id4"]
    );
    assert_eq!(
        app.player_tab.queue_cursor, 0,
        "nothing precedes the range, so the first survivor takes the cursor"
    );
}

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
    assert!(cmd_rx.try_recv().is_err());
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
    assert!(cmd_rx.try_recv().is_err());
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

    assert!(app.submit_queue_item(
        QueueItem::Emby(Box::new(make_item("b2", "Movie"))),
        false,
    ));

    assert_eq!(
        app.player_tab
            .emby_items()
            .iter()
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Item 0", "Item 1", "b2"]
    );
    assert_eq!(app.status, status_before);
    assert!(cmd_rx.try_recv().is_err());
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

    assert!(!app.submit_queue_item(
        QueueItem::Emby(Box::new(make_item("new", "Movie"))),
        false,
    ));

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
    assert_eq!(app.status, super::actions::CONNECTION_LOST_MESSAGE);
    assert_eq!(app.status_severity, super::notify_actions::ToastSeverity::Error);
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

    assert!(!app.submit_queue_item(
        QueueItem::Emby(Box::new(make_item("new", "Movie"))),
        true,
    ));

    assert_eq!(app.remote_player_tab.as_ref().unwrap().emby_items().len(), 3);
    assert_eq!(app.status, super::actions::CONNECTION_LOST_MESSAGE);
    assert_eq!(app.status_severity, super::notify_actions::ToastSeverity::Warning);
}

#[test]
fn removing_from_remote_queue_dispatches_slot_addressed_command() {
    let _guard = crate::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(make_items(2), make_items(3));
    app.set_queue_scope(QueueScope::Remote);

    app.remove_from_queue(1);

    assert!(matches!(
        cmd_rx.try_recv(),
        Ok(mbv_core::ctrl::CtrlCmd::UnifiedQueueRemoveSlot { .. })
    ));
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
            Some(super::types_overlay::OverlayRequest::Confirm(_))
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
        Some(super::types_overlay::OverlayRequest::Confirm(_))
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
        Some(super::types_overlay::OverlayRequest::Confirm(_))
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

#[test]
fn context_menu_remove_targets_displayed_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

    app.open_context_menu(false, None);

    let menu = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu"),
    };
    let action = menu
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("remove from queue action");
    assert_eq!(action, 2);

    app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)), None);

    let item_ids = |items: &[EmbyItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        item_ids(&app.player_tab.emby_items()),
        item_ids(&local_items)
    );
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().emby_items()),
        vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
    );
    assert_eq!(app.remote_queue_undo_stack.len(), 1);
}

#[test]
fn stale_context_menu_remove_remote_queue_index_is_ignored() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

    app.open_context_menu(false, None);

    let menu = match app.pending_overlay.as_ref() {
        Some(super::types_overlay::OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("context menu"),
    };
    let action = menu
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("remove from queue action");
    // Simulate the remote queue shrinking while the context menu is open.
    // We truncate the slots directly to preserve the cursor position
    // (simulating a race where the cursor hasn't been clamped yet).
    {
        let tab = app.remote_player_tab.as_mut().unwrap();
        tab.queue.truncate_slots(2);
    }

    app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)), None);

    let item_ids = |items: &[EmbyItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(
        item_ids(&app.player_tab.emby_items()),
        item_ids(&local_items)
    );
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().emby_items()),
        vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert!(app.remote_queue_undo_stack.is_empty());
}
