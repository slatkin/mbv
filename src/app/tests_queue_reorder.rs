use super::*;
use crate::app::tests::*;

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
fn active_index_prediction_survives_same_length_move_until_player_ack() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(5);
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 2;
        status.queue_len = 5;
    }

    // Move the item immediately before the active one past it. The queue
    // length is unchanged, but the active row's predicted index is now 1.
    assert!(app.apply_queue_move(QueueScope::Local, 1, 2));
    assert_eq!(app.effective_playback_state().active_idx, 1);
    assert_eq!(app.pending_active_idx, Some(1));

    // Once the player status catches up, retain the same displayed index and
    // consume the prediction.
    app.player.status.lock().unwrap().current_idx = 1;
    assert_eq!(app.effective_playback_state().active_idx, 1);
    assert_eq!(app.pending_active_idx, None);
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
        feed_items: Vec::new(),
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
        feed_items: Vec::new(),
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
        feed_items: Vec::new(),
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
