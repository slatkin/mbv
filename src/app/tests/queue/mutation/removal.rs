use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
fn context_menu_remove_targets_displayed_remote_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(3);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items.clone());
    app.panel_focus = PanelFocus::Queue;
    app.set_queue_scope(QueueScope::Remote);
    app.remote_player_tab.as_mut().unwrap().queue_cursor = 2;

    app.open_context_menu(false, None);

    let Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(menu)) =
        app.pending_overlay.as_ref()
    else {
        panic!("context menu");
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

    let Some(crate::app::state::types::overlay::OverlayRequest::ContextMenu(menu)) =
        app.pending_overlay.as_ref()
    else {
        panic!("context menu");
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
    };

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
