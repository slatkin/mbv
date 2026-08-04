use super::*;
use crate::app::tests::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn tracking_stub() -> mbv_core::remote_reconciliation::ReconciliationTracker {
    mbv_core::remote_reconciliation::ReconciliationTracker::new(
        "session",
        vec![
            mbv_core::remote_reconciliation::SubmittedOccurrence::new(1, "id0"),
            mbv_core::remote_reconciliation::SubmittedOccurrence::new(2, "id1"),
        ],
        0,
        0,
    )
    .unwrap()
}

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
fn enqueue_stops_tracking_and_applies_immediately() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.home.section = 0;
    app.home.continue_items = make_items(1);
    app.home.continue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    app.handle_key(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL));
    assert!(app.confirm_modal.is_none());
    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.items.len(), 1);
}

#[test]
fn tracked_playlist_deletes_apply_immediately() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(4);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-1".into()),
        name: "Saved".into(),
    };
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    app.remove_from_queue(1);

    assert!(app.remote_tracker.is_none());
    assert!(app.confirm_modal.is_none());
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id0", "id3"]
    );
    assert!(app.queue_dirty);
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
fn clearing_tracked_queue_applies_immediately_and_stops_tracking() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.remote_tracker = Some(tracking_stub());

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.remote_tracker.is_none());
    assert!(app.confirm_modal.is_none());
    assert!(app.player_tab.items.is_empty());

    app.connected_session_id = Some("session".into());
    let mut advanced_session = make_session("Client", "Emby");
    advanced_session.id = "session".into();
    advanced_session.now_playing_item_id = Some("id1".into());
    app.handle_session_event(SessionEvent::Loaded {
        sessions: vec![advanced_session],
        generation: 1,
    });

    assert!(app.remote_tracker.is_none());
    assert!(app.sessions_rx.try_recv().is_err());
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

    assert!(app.confirm_modal.is_none());
    assert!(app.pending_queue_action.is_none());
    assert!(app.remote_player_tab.as_ref().unwrap().items.is_empty());
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
fn boundary_queue_edit_does_not_retire_tracking() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.remote_tracker = Some(tracking_stub());
    app.player_tab.queue_cursor = 0;

    app.move_queue_item_up();

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn empty_clear_queue_does_not_retire_tracking() {
    let mut app = make_app_stub();
    app.remote_tracker = Some(tracking_stub());

    app.execute_pending_queue_action(PendingQueueAction::ClearQueue);

    assert!(app.remote_tracker.is_some());
}

// ── playlist identity + save coordination (isolate-remote-tracking-client-behavior) ──

fn saved_playlist_app() -> App {
    let mut app = make_app_stub();
    let mut items = make_items(2);
    items[0].playlist_item_id = "entry-0".into();
    items[1].playlist_item_id = "entry-1".into();
    app.player_tab.items = items;
    app.player_tab.sync_queue_model_from_items_if_needed();
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl-1".into()),
        name: "A".into(),
    };
    app
}

fn track_source(app: &mut App, playlist_id: &str) {
    let items = app.player_tab.items.clone();
    app.remote_tracker =
        App::build_remote_tracker_with_source("session", &items, 0, 5, Some(playlist_id.into()));
    assert!(app.remote_tracker.is_some(), "test tracker must build");
}

fn consume_occurrence(app: &mut App, slot_index: usize, media_id: &str, entry_id: &str) {
    let slot = app.player_tab.queue.slots()[slot_index].slot_id;
    app.remote_consume_operations
        .push(super::types_playback::RemoteConsumeOperation {
            operation_id: 1,
            mutation_id: 1,
            session_id: "session".into(),
            tracking_id: 0,
            epoch: 0,
            occurrence_id: 1,
            playlist_id: "pl-1".into(),
            entry_id: entry_id.into(),
            media_id: media_id.into(),
            queue_slot_id: Some(slot),
            queue_lineage: app.remote_queue_lineage,
        });
    app.handle_session_event(SessionEvent::ConsumeOutcome {
        mutation_id: 1,
        operation_id: 1,
        tracking_id: 0,
        session_id: "session".into(),
        epoch: 0,
        occurrence_id: 1,
        playlist_id: "pl-1".into(),
        entry_id: entry_id.into(),
        media_id: media_id.into(),
        result: Ok(()),
    });
}

#[test]
fn untracked_save_invalidates_and_persists_entry_identities() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    let lineage = app.remote_queue_lineage;

    app.save_playlist_to_emby();

    assert!(
        app.player_tab
            .items
            .iter()
            .all(|item| item.playlist_item_id.is_empty()),
        "an untracked save recreates server entry IDs and must still clear the local identities"
    );
    assert_eq!(app.remote_queue_lineage, lineage);
    let persisted = crate::config::load_queue_state().expect("cleared identity persisted");
    assert!(persisted
        .items
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
}

#[test]
fn save_of_tracked_playlist_retires_eligibility_but_preserves_lineage() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    let lineage = app.remote_queue_lineage;
    track_source(&mut app, "pl-1");

    app.save_playlist_to_emby();

    assert!(
        app.remote_tracker.is_none(),
        "a full save recreates entry IDs and must retire tracked consume eligibility"
    );
    assert_eq!(
        app.remote_queue_lineage, lineage,
        "a save does not change queue slots/content/source and must preserve request lineage"
    );
    assert!(app
        .player_tab
        .items
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
}

#[test]
fn overwrite_of_tracked_playlist_advances_lineage_and_clears_entry_ids() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    let lineage = app.remote_queue_lineage;
    track_source(&mut app, "pl-1");

    app.do_overwrite_playlist("pl-1", "A");

    assert!(app.remote_tracker.is_none());
    assert!(
        app.remote_queue_lineage > lineage,
        "a real content replacement advances visible-queue lineage"
    );
    assert!(app
        .player_tab
        .items
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
}

#[test]
fn overwriting_unrelated_playlist_leaves_tracked_source_identity_intact() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    let lineage = app.remote_queue_lineage;
    track_source(&mut app, "pl-1");

    app.do_overwrite_playlist("pl-2", "B");

    assert!(
        app.remote_tracker.is_some(),
        "an unrelated playlist overwrite must not retire the tracked source"
    );
    assert!(app.remote_tracking_source_is("pl-1"));
    assert_eq!(app.remote_queue_lineage, lineage);
    assert_eq!(
        app.player_tab.items[0].playlist_item_id, "entry-0",
        "the current source's identities are not recreated by an unrelated overwrite"
    );
}

#[test]
fn save_after_consumed_projection_snapshots_the_projected_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    consume_occurrence(&mut app, 0, "id0", "entry-0");
    assert_eq!(app.player_tab.items.len(), 1);

    app.save_playlist_to_emby();

    let item_ids = match app
        .playlist_mutations
        .get("pl-1")
        .and_then(|state| state.active.as_ref())
    {
        Some(super::types_playback::PlaylistMutation::Save {
            item_ids: Some(ids),
            ..
        }) => ids.clone(),
        other => panic!("expected active save, got {other:?}"),
    };
    assert_eq!(
        item_ids,
        vec!["id1"],
        "a save requested after emitted consume must snapshot the post-projection queue"
    );
}

#[test]
fn save_before_replace_executes_pending_action_after_tracked_save() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    app.queue_dirty = true;
    let lineage = app.remote_queue_lineage;
    track_source(&mut app, "pl-1");

    app.replace_queue_or_prompt(PendingQueueAction::ClearQueue);
    assert!(app.pending_queue_action.is_some());
    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE));
    assert!(
        app.pending_queue_action.is_some(),
        "the replacement stays queued until the save crosses its boundary"
    );

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.remote_queue_lineage, lineage);

    app.handle_session_event(SessionEvent::PlaylistMutationComplete {
        mutation_id: 1,
        playlist_id: "pl-1".into(),
        queue_lineage: lineage,
        source_playlist_id: "pl-1".into(),
        result: Ok(()),
    });

    assert!(
        app.pending_queue_action.is_none(),
        "a successful save on the original lineage must run the save-before-replace continuation"
    );
    assert!(!app.queue_dirty);
    assert!(app.player_tab.items.is_empty());
}

#[test]
fn manual_save_overwrite_and_save_on_quit_share_per_playlist_ordering() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();

    // Manual saves serialize under the playlist's own coordinator key.
    app.save_playlist_to_emby();
    app.save_playlist_to_emby();
    let state = app
        .playlist_mutations
        .get("pl-1")
        .expect("manual save keyed by playlist");
    assert!(matches!(
        state.active,
        Some(super::types_playback::PlaylistMutation::Save { mutation_id: 1, .. })
    ));
    assert_eq!(
        state.queued.len(),
        1,
        "same-playlist saves must share one ordered stream"
    );

    // Overwrite routes under the overwritten playlist's independent key.
    app.do_overwrite_playlist("pl-2", "B");
    assert!(matches!(
        app.playlist_mutations
            .get("pl-2")
            .and_then(|s| s.active.as_ref()),
        Some(super::types_playback::PlaylistMutation::Replace { .. })
    ));
    assert_eq!(
        app.playlist_mutations.get("pl-1").unwrap().queued.len(),
        1,
        "an unrelated overwrite must not disturb the pl-1 stream"
    );

    // Save-on-quit enters the same coordinator rather than silently discarding:
    // the save fails at HTTP level here, so the source/dirty state survive the
    // attempted save (on_queue_replace_silent would have cleared both).
    app.queue_dirty = true;
    app.client.lock().unwrap().config.save_playlist_on_quit = true;
    assert!(app.try_quit());
    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { id: Some(ref id), .. } if id == "pl-1"
    ));
    assert!(app.queue_dirty);
}

#[test]
fn save_as_success_clears_old_playlist_entry_ids() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();

    app.save_queue_as_playlist("B".into());
    let mutation_id = app.next_playlist_mutation - 1;
    let coordinator_key = format!("create:{mutation_id}");
    app.handle_session_event(SessionEvent::PlaylistCreateComplete {
        mutation_id,
        coordinator_key: coordinator_key.clone(),
        name: "B".into(),
        queue_lineage: app.remote_queue_lineage,
        source_playlist_id: Some("pl-1".into()),
        result: Ok("pl-2".into()),
    });

    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { id: Some(ref id), .. } if id == "pl-2"
    ));
    assert!(
        app.player_tab
            .items
            .iter()
            .all(|item| item.playlist_item_id.is_empty()),
        "the new source must never retain entry identities from the old playlist"
    );
    let persisted = crate::config::load_queue_state().expect("save-as persisted");
    assert!(persisted
        .items
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
    assert!(matches!(
        persisted.source,
        crate::config::QueueSource::Playlist { id: Some(ref id), .. } if id == "pl-2"
    ));
}

#[test]
fn replace_completion_persists_new_source_and_cleared_entry_ids() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    let lineage = app.remote_queue_lineage;

    app.do_overwrite_playlist("pl-2", "B");
    assert_eq!(
        app.player_tab.items[0].playlist_item_id, "entry-0",
        "boundary of an unrelated overwrite must not clear the current source's identities"
    );

    app.handle_session_event(SessionEvent::PlaylistReplacementComplete {
        mutation_id: 1,
        playlist_id: "pl-2".into(),
        queue_lineage: lineage,
        source_playlist_id: "pl-2".into(),
        name: "B".into(),
        result: Ok("pl-2".into()),
    });

    assert!(matches!(
        app.queue_source,
        crate::config::QueueSource::Playlist { id: Some(ref id), .. } if id == "pl-2"
    ));
    assert!(!app.queue_dirty);
    assert!(app
        .player_tab
        .items
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
    let persisted = crate::config::load_queue_state().expect("overwrite persisted");
    assert!(matches!(
        persisted.source,
        crate::config::QueueSource::Playlist { id: Some(ref id), .. } if id == "pl-2"
    ));
    assert!(persisted
        .items
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
}

#[test]
fn save_and_save_on_quit_cannot_resurrect_a_consumed_occurrence() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    consume_occurrence(&mut app, 0, "id0", "entry-0");
    assert_eq!(app.player_tab.items.len(), 1);

    // Manual save completes cleanly against the projected queue.
    app.queue_dirty = true;
    app.save_playlist_to_emby();
    app.handle_session_event(SessionEvent::PlaylistMutationComplete {
        mutation_id: 1,
        playlist_id: "pl-1".into(),
        queue_lineage: app.remote_queue_lineage,
        source_playlist_id: "pl-1".into(),
        result: Ok(()),
    });
    assert!(!app.queue_dirty);
    assert_eq!(app.player_tab.items.len(), 1);

    // Save-on-quit snapshots the same projected queue, so the consumed
    // occurrence is never re-added by the quit save.
    app.queue_dirty = true;
    app.client.lock().unwrap().config.save_playlist_on_quit = true;
    assert!(app.try_quit());
    let persisted = crate::config::load_queue_state().expect("queue persisted");
    assert_eq!(persisted.items.len(), 1);
    assert_eq!(persisted.items[0].id, "id1");
}

// ── tasks 1.4 / 6.2 / 6.3: queue edits retire tracking only on real mutation ──

#[test]
fn reorder_retires_tracking_after_successful_move() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.queue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    app.move_queue_item_down();

    assert!(app.remote_tracker.is_none());
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id1", "id0", "id2"]
    );
}

#[test]
fn undo_retires_tracking_after_restoring_the_edit() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    app.remote_tracker = Some(tracking_stub());
    app.undo_last_queue_edit(QueueScope::Local);

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.items.len(), 3);
}

#[test]
fn empty_undo_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.remote_tracker = Some(tracking_stub());

    app.undo_last_queue_edit(QueueScope::Local);

    assert!(app.remote_tracker.is_some());
}

#[test]
fn boundary_move_down_leaves_tracking_active() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.player_tab.queue_cursor = 1;
    app.remote_tracker = Some(tracking_stub());

    app.move_queue_item_down();

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn out_of_range_removal_leaves_tracking_active() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(5);

    assert!(app.remote_tracker.is_some());
    assert_eq!(app.player_tab.items.len(), 2);
}

#[test]
fn rejected_route_enqueue_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());
    app.active_route = Some("music".to_string());
    let mut movies_item = make_item("Movies", "CollectionFolder");
    movies_item.id = "lib-movies".to_string();
    app.libs.push(LibraryTab {
        library: movies_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app.library_tab = 1;
    app.remote_tracker = Some(tracking_stub());

    app.enqueue_selected();

    assert!(app.remote_tracker.is_some());
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

fn serve_one_response(listener: &std::net::TcpListener, body: &str) {
    let (stream, _) = listener.accept().unwrap();
    let mut writer = stream.try_clone().unwrap();
    use std::io::Write;
    let _ = writer.write_all(
        format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        )
        .as_bytes(),
    );
}

#[test]
fn failed_folder_enqueue_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let mut folder = make_item("Folder", "CollectionFolder");
    folder.is_folder = true;
    app.home.continue_items = vec![folder];
    app.home.continue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    // The stub client has no server URL configured, so the enqueue fetch
    // fails at the HTTP layer before anything can be appended.
    app.enqueue_selected();

    assert!(app.remote_tracker.is_some());
    assert!(app.player_tab.items.is_empty());
}

#[test]
fn empty_folder_enqueue_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let mut app = make_app_stub();
    app.client.lock().unwrap().config.server_url = url;
    let mut folder = make_item("Folder", "CollectionFolder");
    folder.is_folder = true;
    app.home.continue_items = vec![folder];
    app.home.continue_cursor = 0;
    app.remote_tracker = Some(tracking_stub());

    let handle = std::thread::spawn(move || serve_one_response(&listener, r#"{"Items":[]}"#));
    app.enqueue_selected();
    handle.join().unwrap();

    assert!(app.remote_tracker.is_some());
    assert!(app.player_tab.items.is_empty());
    assert!(app.status.contains("Nothing to enqueue"));
}

#[test]
fn canceled_active_item_removal_leaves_tracking_active() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    assert!(app.confirm_modal.is_some());

    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));

    assert!(app.confirm_modal.is_none());
    assert_eq!(app.player_tab.items.len(), 3);
    assert!(app.remote_tracker.is_some());
}

#[test]
fn confirmed_active_item_removal_retires_tracking() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.queue_cursor = 1;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(1);
    app.handle_key_confirm_modal(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));

    assert!(app.remote_tracker.is_none());
    assert_eq!(app.player_tab.items.len(), 2);
}

// ── task 6.5: two successive removals then a save containing both edits ───

#[test]
fn two_removals_apply_immediately_and_save_snapshots_both_edits() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(4);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl-1".into()),
        name: "Saved".into(),
    };
    app.remote_tracker = Some(tracking_stub());

    app.remove_from_queue(0);
    app.remove_from_queue(1);
    assert!(app.remote_tracker.is_none());
    assert_eq!(
        app.player_tab
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["id1", "id3"]
    );

    app.save_playlist_to_emby();
    let item_ids = match app
        .playlist_mutations
        .get("pl-1")
        .and_then(|state| state.active.as_ref())
    {
        Some(super::types_playback::PlaylistMutation::Save {
            item_ids: Some(ids),
            ..
        }) => ids.clone(),
        other => panic!("expected active save, got {other:?}"),
    };
    assert_eq!(
        item_ids,
        vec!["id1", "id3"],
        "the following playlist save must contain both removals"
    );
}
