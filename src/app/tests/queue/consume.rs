use crate::app::tests::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn confirmed_delete_waits_for_owner_snapshot_after_stopped_removal_op() {
    // Confirm records the slot to remove after playback stops; neither the
    // confirmation nor its Stopped event optimistically edits the Client view.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.local_view
        .adopt_items(make_items(3), app.local_view.cursor());
    // TrackChanged(0) activates slot 0, mirroring real playback where the
    // model's active_slot_id becomes Some before the delete.
    app.handle_player_event(PlayerEvent::TrackChanged {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        transition: None,
    });
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    };
    app.ask_confirm(ConfirmModal {
        title: String::new(),
        message: String::new(),
        hint: String::new(),
        on_confirm: ConfirmAction::RemoveActiveQueueItem(0),
    });

    let mut model = Model::new(app);
    model.sync_modal_requests();
    model.handle_confirm_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let app = &mut model.app;

    assert_eq!(
        app.local_view.emby_items().len(),
        3,
        "the Client view waits for the owner's queue snapshot"
    );
    // Row 4.1 removed the in-process undo push; undo returns as an owner op in Unit 5.2.
    assert!(
        app.pending_delete_slot.is_some(),
        "the pending delete must be recorded so the eventual Stopped event recognizes it"
    );

    app.handle_player_event(PlayerEvent::Stopped {
        slot_id: app.playback_queue().slot_id_at(0),
        run_identity: 0,
        position_ticks: 0,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(
        app.local_view.emby_items().len(),
        3,
        "the Stopped event must not optimistically change the Client view"
    );
    assert_eq!(
        app.queue_undo_stack.len(),
        0,
        "the Stopped event must not push a second undo entry"
    );
    assert!(
        app.pending_delete_slot.is_none(),
        "the Stopped event must clear the pending-delete marker"
    );
}

#[test]
fn confirmed_delete_with_stale_position_does_not_mark_a_pending_delete() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.local_view
        .adopt_items(make_items(1), app.local_view.cursor());
    app.ask_confirm(ConfirmModal {
        title: String::new(),
        message: String::new(),
        hint: String::new(),
        on_confirm: ConfirmAction::RemoveActiveQueueItem(1),
    });

    let mut model = Model::new(app);
    model.sync_modal_requests();
    model.handle_confirm_key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE));
    let app = &mut model.app;

    assert_eq!(app.local_view.emby_items().len(), 1);
    assert!(app.pending_delete_slot.is_none());
    assert!(!app.queue_dirty);
}

#[test]
fn stopped_consume_keeps_the_client_view_until_the_owner_snapshot() {
    let _guard = crate::config::TestStateDirGuard::new();
    // When the last item in the queue finishes, the player thread sends a
    // Stopped event (not TrackCompleted/TrackChanged) since there's no next
    // track to advance to. consume_audio must still remove it, mirroring how
    // consume_videos already works for a video's Stopped-path removal.
    let items = make_audio_items(1);
    let mut app = make_app_stub();
    app.local_view.adopt_items(items, app.local_view.cursor());
    app.config.lock().unwrap().consume_audio = true;

    app.handle_player_event(PlayerEvent::Stopped {
        slot_id: app.playback_queue().slot_id_at(0),
        run_identity: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(app.local_view.emby_items().len(), 1);
}

#[test]
fn track_completed_for_removed_slot_does_not_mutate_queue() {
    let mut app = make_app_stub();
    app.local_view
        .adopt_items(make_items(2), app.local_view.cursor());
    let ids_before: Vec<_> = app.local_view.slots().iter().map(|s| s.slot_id).collect();

    // a slot id that was never allocated in this queue
    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: mbv_queue::QueueSlotId::from_raw(9999),
        run_identity: 0,
        position_ticks: 600_000_000,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    let ids_after: Vec<_> = app.local_view.slots().iter().map(|s| s.slot_id).collect();
    assert_eq!(ids_before, ids_after);
}

#[test]
fn track_changed_activates_the_current_slot() {
    let mut app = make_app_stub();
    app.local_view
        .adopt_items(make_items(3), app.local_view.cursor());
    app.handle_player_event(PlayerEvent::TrackChanged {
        slot_id: app.playback_queue().slot_id_at(1).unwrap(),
        transition: None,
    });

    assert_eq!(
        app.local_view.cursor(),
        1,
        "TrackChanged resolves the owner's stable slot to the selected view position"
    );
}

#[test]
fn track_completed_consume_keeps_the_client_view_until_owner_snapshot() {
    // The completion reaction must not consume the Client's read-only queue.
    let mut app = make_app_stub();
    app.local_view
        .adopt_items(make_items(3), app.local_view.cursor());
    let slot_b = app.local_view.slots()[1].slot_id;
    app.config.lock().unwrap().consume_videos = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        run_identity: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    assert_eq!(app.local_view.slots().len(), 3);

    app.handle_player_event(PlayerEvent::TrackChanged {
        slot_id: slot_b,
        transition: None,
    });

    assert_eq!(app.local_view.slots().len(), 3);
    assert_eq!(app.local_view.cursor(), 1);
}

#[test]
fn a_video_consume_flag_is_ignored_when_video_consumption_is_disabled() {
    // queue-owner-process design D9: client consume side effects follow the
    // owner-provided consume flag only when this media kind is enabled.
    let mut app = make_app_stub();
    app.local_view
        .adopt_items(make_items(2), app.local_view.cursor());
    app.config.lock().unwrap().consume_videos = false;
    app.config.lock().unwrap().save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        run_identity: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    assert!(
        !app.queue_dirty,
        "disabled video consumption has no consume reaction"
    );
}

#[test]
fn consuming_a_video_without_autosave_marks_queue_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.local_view.adopt_items(items, app.local_view.cursor());
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        });
    app.config.lock().unwrap().consume_videos = true;
    app.config.lock().unwrap().save_playlist_on_consume = false;

    // First item finishes playing and is consumed at completion.
    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        run_identity: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    assert_eq!(app.local_view.emby_items().len(), 2);
    assert!(
        app.queue_dirty,
        "consuming an item changes the saved playlist's contents; without \
             save_playlist_on_consume the queue must be marked dirty so the user is still \
             prompted to save before quitting/replacing the queue"
    );
}

#[test]
fn consuming_a_video_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.local_view.adopt_items(items, app.local_view.cursor());
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        });
    app.config.lock().unwrap().consume_videos = true;
    app.config.lock().unwrap().save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        run_identity: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    assert_eq!(app.local_view.emby_items().len(), 2);
    assert!(
        !app.queue_dirty,
        "with save_playlist_on_consume enabled, consuming from a saved playlist should \
             trigger an immediate re-save to Emby (mirroring the manual save-playlist flow), \
             so the queue is no longer considered dirty"
    );
}

#[test]
fn consuming_a_video_on_direct_remote_queue_does_not_touch_local_queue_or_dirty_flag() {
    let _guard = crate::config::TestStateDirGuard::new();
    let local_items = make_items(2);
    let remote_items = make_items(2);
    let mut app = make_remote_app_stub(local_items.clone(), remote_items);
    // The local queue happens to be a saved playlist with autosave-on-consume enabled —
    // the trap scenario: before the scope gate, consuming on the *remote* queue would
    // still fire save_playlist_to_emby() and push the unrelated, unmodified local
    // playlist to Emby.
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        });
    app.config.lock().unwrap().consume_videos = true;
    app.config.lock().unwrap().save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        run_identity: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    assert_eq!(
        app.remote_view.as_ref().unwrap().emby_items().len(),
        2,
        "the Client must not remove from the out-of-process owner queue"
    );
    assert_eq!(
        app.local_view.emby_items().len(),
        local_items.len(),
        "consume on a direct-remote queue must not touch the unrelated local playlist"
    );
    assert!(
        !app.queue_dirty,
        "consume on a direct-remote queue must not mark the local queue dirty or trigger \
             an autosave of the local playlist — the change happened on the remote queue"
    );
}

#[test]
fn clients_hold_no_editable_queue_track_completed_audio_consume_keeps_view_until_snapshot() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.local_view.adopt_items(items, app.local_view.cursor());
    let slot_id = app.local_view.slot_id_at(0).unwrap();
    let item_before = app
        .local_view
        .item_at(0)
        .unwrap()
        .as_emby()
        .unwrap()
        .clone();
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        });
    app.config.lock().unwrap().consume_audio = true;
    app.config.lock().unwrap().save_playlist_on_consume_audio = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id,
        run_identity: 0,
        position_ticks: 600_000_000,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    assert_eq!(app.local_view.emby_items().len(), 2);
    assert_eq!(
        app.local_view.item_at(0).unwrap().as_emby().unwrap(),
        &item_before
    );
    assert!(
        app.queue_dirty,
        "consuming an audio item changes the saved playlist's contents; without \
             save_playlist_on_consume_audio the queue must be marked dirty so the user is \
             still prompted to save before quitting/replacing the queue"
    );
}

#[test]
fn consuming_an_audio_item_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.local_view.adopt_items(items, app.local_view.cursor());
    app.local_view
        .adopt_source(mbv_queue::QueueSource::Playlist {
            id: Some("pl1".to_string()),
            name: "My Playlist".to_string(),
        });
    app.config.lock().unwrap().consume_audio = true;
    app.config.lock().unwrap().save_playlist_on_consume_audio = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        slot_id: app.playback_queue().slot_id_at(0).unwrap(),
        run_identity: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });

    assert_eq!(app.local_view.emby_items().len(), 2);
    assert!(
        !app.queue_dirty,
        "with save_playlist_on_consume_audio enabled, consuming from a saved playlist \
             should trigger an immediate re-save to Emby, so the queue is no longer dirty"
    );
}
