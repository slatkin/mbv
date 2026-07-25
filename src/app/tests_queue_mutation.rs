use super::*;
use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn ctrl_a_enqueues_from_home_view() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert!(!handled);
    assert_eq!(app.player_tab.items.len(), 1);
    assert_eq!(app.player_tab.items[0].id, "id0");
}

#[test]
fn ctrl_a_appends_to_direct_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(local_items, remote_items.clone());
    app.queue_scope = QueueScope::Remote;
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert!(!handled);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
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
        Ok(mbv_core::ctrl::CtrlCmd::PlayerCmd(
            mbv_core::ctrl::WireCommand::QueueAppend { items }
        )) if items.len() == 1 && items[0].id == "id0"
    ));
    assert!(
        cmd_rx.try_recv().is_err(),
        "Ctrl+A append must not follow QueueAppend with ReplaceQueue"
    );
}

#[test]
fn ctrl_a_rejects_v2_direct_remote_append_without_replace_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_v2_remote_app_stub_with_cmd_rx(local_items, remote_items);
    app.queue_scope = QueueScope::Remote;
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;

    let handled = app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));

    assert!(!handled);
    assert!(
        cmd_rx.try_recv().is_err(),
        "v2 direct remote append must not fall back to ReplaceQueue"
    );
    assert_eq!(
        app.status,
        "Remote append is not supported by this direct mbv peer"
    );
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["id0", "id1", "id2"]
    );
}

#[test]
fn rejected_v2_direct_remote_append_preserves_remote_undo_slot_identity() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, _cmd_rx) = make_v2_remote_app_stub_with_cmd_rx(local_items, remote_items);
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_up();
    let moved_slot = app
        .remote_player_tab
        .as_ref()
        .unwrap()
        .resolve_slot_at(0)
        .expect("moved slot should be at destination");

    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;
    app.handle_key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::ALT));

    assert!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .slot_id_matches_at(0, moved_slot),
        "rejected append rollback must preserve existing remote queue slot IDs"
    );

    app.undo_last_queue_edit(QueueScope::Remote);

    assert_ne!(app.status, "Can't undo move: queue changed since then");
    assert!(app.remote_queue_undo_stack.is_empty());
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["id0", "id1", "id2"]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
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

    assert!(app.player_tab.items.is_empty());
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
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

    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
    assert_eq!(
        app.player_tab
            .items
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

#[test]
fn removing_from_local_queue_in_direct_remote_mode_does_not_touch_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Local);

    app.remove_from_queue(1);

    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![local_items[0].id.as_str(), local_items[2].id.as_str()]
    );
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
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

    assert_eq!(app.remote_player_tab.as_ref().unwrap().items.len(), 2);
    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![remote_items[0].id.as_str(), remote_items[2].id.as_str()]
    );
    assert_eq!(
        app.player_tab
            .items
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

    assert!(!app.show_save_playlist_modal);
    assert!(app.pending_queue_action.is_none());
    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
    assert!(app.queue_dirty);
}

#[test]
fn removing_from_inactive_remote_queue_is_rejected() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items, remote_items.clone());
    app.player.status.lock().unwrap().active = false;

    app.remove_from_queue(1);

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.status, "Remote queue can only be edited while active");
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

    app.open_context_menu();

    let action = app
        .context_menu
        .as_ref()
        .expect("context menu")
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("remove from queue action");
    assert_eq!(action, 2);

    app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)));

    let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(item_ids(&app.player_tab.items), item_ids(&local_items));
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().items),
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

    app.open_context_menu();

    let action = app
        .context_menu
        .as_ref()
        .expect("context menu")
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(ContextAction::RemoveFromQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("remove from queue action");
    app.remote_player_tab.as_mut().unwrap().items.truncate(2);

    app.execute_context_action(Some(ContextAction::RemoveFromQueue(action)));

    let item_ids = |items: &[MediaItem]| items.iter().map(|i| i.id.clone()).collect::<Vec<_>>();
    assert_eq!(item_ids(&app.player_tab.items), item_ids(&local_items));
    assert_eq!(
        item_ids(&app.remote_player_tab.as_ref().unwrap().items),
        vec![remote_items[0].id.clone(), remote_items[1].id.clone()]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert!(app.remote_queue_undo_stack.is_empty());
}

#[test]
fn move_queue_item_up_swaps_items_and_cursor_follows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;

    app.move_queue_item_up();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            items[1].id.as_str(),
            items[0].id.as_str(),
            items[2].id.as_str()
        ]
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert_eq!(app.queue_undo_stack.len(), 1);
}

#[test]
fn move_queue_item_down_swaps_items_and_cursor_follows() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;

    app.move_queue_item_down();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            items[0].id.as_str(),
            items[2].id.as_str(),
            items[1].id.as_str()
        ]
    );
    assert_eq!(app.player_tab.queue_cursor, 2);
    assert_eq!(app.queue_undo_stack.len(), 1);
}

#[test]
fn move_queue_item_up_is_noop_at_start_of_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_up();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 0);
    assert!(app.queue_undo_stack.is_empty());
}

#[test]
fn move_queue_item_down_is_noop_at_end_of_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 2;

    app.move_queue_item_down();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 2);
    assert!(app.queue_undo_stack.is_empty());
}

#[test]
fn undo_reverses_a_move_and_cursor_follows_back() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;

    app.move_queue_item_up();
    assert_eq!(app.player_tab.queue_cursor, 0);

    app.undo_last_queue_edit(QueueScope::Local);

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>()
    );
    assert_eq!(app.player_tab.queue_cursor, 1);
    assert!(app.queue_undo_stack.is_empty());
}

#[test]
fn undo_of_move_does_not_disturb_prior_removal_undo_history() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    // A removal, then a move -- undoing once should only reverse the move.
    app.remove_from_queue(0);
    app.player_tab.queue_cursor = 0;
    app.move_queue_item_down();
    assert_eq!(app.queue_undo_stack.len(), 2);

    app.undo_last_queue_edit(QueueScope::Local);

    assert_eq!(app.queue_undo_stack.len(), 1);
    assert!(matches!(
        app.queue_undo_stack.last(),
        Some(UndoEntry::Remove(0, _))
    ));
}

#[test]
fn undo_of_move_is_refused_if_the_moved_item_is_no_longer_at_to() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_down(); // items[0] now sits at index 1
    assert_eq!(app.queue_undo_stack.len(), 1);

    // Something untracked by this undo stack happens to the queue
    // afterwards (e.g. a natural consume) removing the item that's now
    // at index 1, so the undo entry's `to` position no longer holds the
    // item that was actually moved.
    app.player_tab.items.remove(1);

    app.undo_last_queue_edit(QueueScope::Local);

    // Refused rather than blindly swapping whatever now sits at 0/1.
    assert_eq!(app.status, "Can't undo move: queue changed since then");
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![items[1].id.as_str(), items[2].id.as_str()]
    );
}

#[test]
fn undo_of_move_is_refused_when_duplicate_id_masks_changed_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut items = make_items(3);
    items[0].id = "duplicate".into();
    items[0].name = "First duplicate".into();
    items[0].playlist_item_id = "slot-a".into();
    items[1].id = "duplicate".into();
    items[1].name = "Second duplicate".into();
    items[1].playlist_item_id = "slot-b".into();
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_down(); // First duplicate now sits at index 1.
    assert_eq!(app.queue_undo_stack.len(), 1);

    app.player_tab.items.remove(1);
    app.player_tab.items.insert(1, items[1].clone());

    app.undo_last_queue_edit(QueueScope::Local);

    assert_eq!(app.status, "Can't undo move: queue changed since then");
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Second duplicate", "Second duplicate", "Item 2"]
    );
}

#[test]
fn resolve_slot_at_maps_index_to_slot_and_rejects_out_of_range() {
    let tab = PlayerTab::new(make_items(3), 0);
    let s0 = tab.queue.slots()[0].slot_id;
    let s2 = tab.queue.slots()[2].slot_id;
    assert_eq!(tab.resolve_slot_at(0), Some(s0));
    assert_eq!(tab.resolve_slot_at(2), Some(s2));
    assert_eq!(tab.resolve_slot_at(3), None);
}

#[test]
fn queue_edit_preserves_updated_item_fields_after_shadow_model_was_built() {
    let mut app = make_app_stub();
    app.player_tab.set_items(make_items(2), 0);
    let _slot_id = app.player_tab.slot_id_at(0).unwrap();

    app.player_tab.items[0].playback_position_ticks = 42;
    app.player_tab.items[0].played = true;

    app.player_tab.append_item(make_item("new", "Movie"));

    assert_eq!(app.player_tab.items[0].playback_position_ticks, 42);
    assert!(app.player_tab.items[0].played);
}

#[test]
fn move_queue_item_for_remote_scope_sends_move_command_and_preserves_local_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) =
        make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_up();

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            remote_items[1].id.as_str(),
            remote_items[0].id.as_str(),
            remote_items[2].id.as_str()
        ]
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
    assert_eq!(
        app.player_tab
            .items
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
    assert!(matches!(
        cmd_rx.try_recv(),
        Ok(mbv_core::ctrl::CtrlCmd::PlayerCmd(
            mbv_core::ctrl::WireCommand::QueueMove(1, 0)
        ))
    ));
}

#[test]
fn move_queue_item_for_inactive_remote_scope_is_rejected() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(3);
    let remote_items = make_items(3);
    let (mut app, cmd_rx) = make_remote_app_stub_with_cmd_rx(local_items, remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;
    app.player.status.lock().unwrap().active = false;

    app.move_queue_item_up();

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        remote_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 1);
    assert_eq!(app.status, "Remote queue can only be edited while active");
    assert!(cmd_rx.try_recv().is_err());
}

#[test]
fn remote_queue_update_reconciles_remote_queue_without_touching_local_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    let updated_remote = vec![
        remote_items[2].clone(),
        remote_items[0].clone(),
        remote_items[1].clone(),
    ];

    app.handle_player_event(PlayerEvent::QueueUpdated {
        items: updated_remote.clone(),
        cursor: 2,
        source: crate::config::QueueSource::Remote,
    });

    assert_eq!(
        app.remote_player_tab
            .as_ref()
            .unwrap()
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        updated_remote
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn remote_queue_update_after_move_keeps_cursor_on_moved_item() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let (mut app, _cmd_rx) =
        make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_up();

    app.handle_player_event(PlayerEvent::QueueUpdated {
        items: vec![
            remote_items[1].clone(),
            remote_items[0].clone(),
            remote_items[2].clone(),
        ],
        cursor: 1,
        source: crate::config::QueueSource::Remote,
    });

    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 0);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn remote_queue_update_after_move_tracks_duplicate_item_by_position() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let mut remote_items = make_items(3);
    remote_items[1].id = remote_items[0].id.clone();
    let (mut app, _cmd_rx) =
        make_remote_app_stub_with_cmd_rx(local_items.clone(), remote_items.clone());
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 1;

    app.move_queue_item_down();

    app.handle_player_event(PlayerEvent::QueueUpdated {
        items: vec![
            remote_items[0].clone(),
            remote_items[2].clone(),
            remote_items[1].clone(),
        ],
        cursor: 0,
        source: crate::config::QueueSource::Remote,
    });

    assert_eq!(app.remote_player_tab.as_ref().unwrap().queue_cursor, 2);
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        local_items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn moving_now_playing_item_keeps_cursor_on_it() {
    let _guard = crate::config::TestStateDirGuard::new();
    // `PlayerProxy::stub` (used by `make_app_stub`) has no live cmd channel to
    // assert against, so this only covers the app-side item/cursor bookkeeping;
    // `player::tests` covers the mpv-side PlaylistMove handling directly.
    let items = make_items(3);
    let mut app = make_app_stub();
    app.player_tab.items = items.clone();
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }

    app.move_queue_item_down();

    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|i| i.id.as_str())
            .collect::<Vec<_>>(),
        vec![
            items[0].id.as_str(),
            items[2].id.as_str(),
            items[1].id.as_str()
        ]
    );
    assert_eq!(app.player_tab.queue_cursor, 2);
}
