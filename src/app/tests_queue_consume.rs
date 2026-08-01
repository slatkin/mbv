use super::*;
use crate::app::tests::*;

#[test]
fn stopped_progress_updates_the_queue_model_not_just_the_shadow() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_id = app.player_tab.queue.slots()[0].slot_id;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    let slot = app.player_tab.queue.slot(slot_id).unwrap();
    assert_eq!(
        slot.item.playback_position_ticks, 600_000_000,
        "progress must be applied to the queue model, not only the display shadow"
    );
}

#[test]
fn stopped_with_accepted_report_marks_pending_sync_and_clears_active_slot() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(1);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_id = app.player_tab.queue.slots()[0].slot_id;
    app.handle_player_event(PlayerEvent::TrackChanged(0));
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.current_idx = 0;
    }

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: true,
        error: None,
    });

    let slot = app.player_tab.queue.slot(slot_id).unwrap();
    assert_eq!(
        slot.progress_state
            .pending_sync
            .as_ref()
            .map(|progress| progress.position_ticks),
        Some(600_000_000)
    );
    assert_eq!(app.player_tab.queue.active_slot_id(), None);
}

#[test]
fn stopped_consume_removes_the_right_slot_occurrence() {
    // Duplicate item ids: two occurrences of the same underlying item.
    // Stopping+consuming the second occurrence must remove that slot
    // specifically — never the first, which happens to share an id.
    let mut app = make_app_stub();
    let mut items = make_items(3);
    items[0].id = "dup".into();
    items[2].id = "dup".into();
    app.player_tab.items = items;
    app.player_tab.sync_queue_model_from_items_if_needed();
    let first_dup = app.player_tab.queue.slots()[0].slot_id;
    let second_dup = app.player_tab.queue.slots()[2].slot_id;
    app.client.lock().unwrap().config.consume_videos = true;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 2,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert!(app.player_tab.queue.slot(first_dup).is_some());
    assert!(app.player_tab.queue.slot(second_dup).is_none());
}

#[test]
fn stopped_delete_removes_the_active_now_playing_slot() {
    // The confirmed "remove now-playing item and stop playback" flow:
    // pending_delete_slot marks the active slot for removal, then a Stopped
    // event drives it. Now that TrackChanged populates the model's
    // active_slot_id in real playback, the gated remove_slot path would
    // refuse the active slot — the confirmed delete must bypass that gate.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    // TrackChanged(0) activates slot 0, mirroring real playback where the
    // model's active_slot_id becomes Some before the delete.
    app.handle_player_event(PlayerEvent::TrackChanged(0));
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let slot_0 = app.player_tab.queue.slots()[0].slot_id;
    app.pending_delete_slot = Some(slot_0);

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "the confirmed delete must remove the active now-playing slot"
    );
    assert_eq!(
        app.queue_undo_stack.len(),
        1,
        "delete must push an undo entry"
    );
}

#[test]
fn stopped_path_consumes_the_last_audio_item_in_the_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    // When the last item in the queue finishes, the player thread sends a
    // Stopped event (not TrackCompleted/TrackChanged) since there's no next
    // track to advance to. consume_audio must still remove it, mirroring how
    // consume_videos already works for a video's Stopped-path removal.
    let items = make_audio_items(1);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_audio = true;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert!(
        app.player_tab.items.is_empty(),
        "the last audio item should be consumed via the Stopped-path when consume_audio is on"
    );
}

#[test]
fn stopped_path_does_not_consume_audio_when_consume_audio_is_off() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(1);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_audio = false;

    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consume_audio is off, so the item must stay in the queue"
    );
}

#[test]
fn track_completed_progress_follows_slot_after_earlier_removal() {
    // queue: [a, b, c]; a is removed (indices shift: b now at 0, c at 1),
    // then a completion event for the player's post-removal index of b
    // (0) arrives. Progress must land on slot b regardless of the churn.
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;
    let slot_a = app.player_tab.queue.slots()[0].slot_id;
    assert!(matches!(
        app.player_tab.queue.remove_slot(slot_a),
        RemoveSlotResult::Removed(_)
    ));
    app.player_tab.sync_items_from_queue_model();

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: false,
    });

    let slot = app.player_tab.queue.slot(slot_b).unwrap();
    assert_eq!(slot.item.playback_position_ticks, 600_000_000);
}

#[test]
fn track_completed_for_removed_slot_does_not_mutate_queue() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(2);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let ids_before: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|s| s.slot_id)
        .collect();

    // index 5 does not exist
    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 5,
        position_ticks: 600_000_000,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });

    let ids_after: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|s| s.slot_id)
        .collect();
    assert_eq!(ids_before, ids_after);
    assert!(app.pending_queue_removal.is_none());
}

#[test]
fn track_changed_activates_the_current_slot() {
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;

    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.queue.active_slot_id(),
        Some(slot_b),
        "TrackChanged must set the model's active slot by identity, not just move the raw cursor"
    );
}

#[test]
fn track_changed_activates_slot_and_consumes_deferred_slot() {
    // [a, b, c]; complete+consume a (deferred), then TrackChanged to b.
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;
    app.client.lock().unwrap().config.consume_videos = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    assert!(app.pending_queue_removal.is_some());

    app.handle_player_event(PlayerEvent::TrackChanged(1)); // player reports b at old idx 1

    // a was consumed; queue is [b, c]; b is active.
    assert_eq!(app.player_tab.queue.slots().len(), 2);
    assert_eq!(app.player_tab.queue.active_slot_id(), Some(slot_b));
    assert!(app.pending_queue_removal.is_none());
}

#[test]
fn consuming_a_video_without_autosave_marks_queue_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.save_playlist_on_consume = false;

    // First item finishes playing and is consumed while advancing to the next track.
    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed item should be removed from the local queue"
    );
    assert!(
        app.queue_dirty,
        "consuming an item changes the saved playlist's contents; without \
             save_playlist_on_consume the queue must be marked dirty so the user is still \
             prompted to save before quitting/replacing the queue"
    );
}

#[test]
fn consuming_a_video_resyncs_the_players_own_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    // The player thread (QueueSession) keeps its own separate copy of the
    // items list, independent of `player_tab.items`. If consume only shrinks
    // the app-side queue and never tells the player, the player's internal
    // index space permanently diverges from the displayed queue after the
    // first consume — any later index-based command (Enter on a queue row,
    // JumpTo, next natural advance) then lands on the wrong item.
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_videos = true;
    let cmd_rx = app.player.spy_on_commands();

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(crate::player::PlayerCommand::QueueRemove(0))
        ),
        "consuming idx=0 must tell the player to remove idx=0 from its own \
             internal queue, keeping it in sync with the app-side queue"
    );
}

#[test]
fn consuming_a_video_with_autosave_pushes_playlist_to_emby_and_clears_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed item should be removed from the local queue"
    );
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
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.save_playlist_on_consume = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.remote_player_tab.as_ref().unwrap().items.len(),
        1,
        "consumed item should still be removed from the remote queue"
    );
    assert_eq!(
        app.player_tab.items.len(),
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
fn consuming_an_audio_item_without_autosave_marks_queue_dirty() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_audio = true;
    app.client
        .lock()
        .unwrap()
        .config
        .save_playlist_on_consume_audio = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed audio item should be removed from the local queue"
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
    app.player_tab.items = items;
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl1".to_string()),
        name: "My Playlist".to_string(),
    };
    app.client.lock().unwrap().config.consume_audio = true;
    app.client
        .lock()
        .unwrap()
        .config
        .save_playlist_on_consume_audio = true;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        1,
        "consumed audio item should be removed from the local queue"
    );
    assert!(
        !app.queue_dirty,
        "with save_playlist_on_consume_audio enabled, consuming from a saved playlist \
             should trigger an immediate re-save to Emby, so the queue is no longer dirty"
    );
}

#[test]
fn consume_videos_flag_does_not_consume_audio_items() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_audio_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_videos = true;
    app.client.lock().unwrap().config.consume_audio = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "consume_videos must not remove an audio item; consume_audio is off"
    );
}

#[test]
fn consume_audio_flag_does_not_consume_video_items() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let mut app = make_app_stub();
    app.player_tab.items = items;
    app.client.lock().unwrap().config.consume_audio = true;
    app.client.lock().unwrap().config.consume_videos = false;

    app.handle_player_event(PlayerEvent::TrackCompleted {
        idx: 0,
        position_ticks: 0,
        played: true,
        consume: true,
        progress_report_accepted: false,
    });
    app.handle_player_event(PlayerEvent::TrackChanged(1));

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "consume_audio must not remove a video item; consume_videos is off"
    );
}

// ── P1 fix: active-delete race ───────────────────────────────────────────

#[test]
fn pending_delete_slot_matches_by_identity_even_after_index_shift() {
    // P3 scenario: queue [A, B(active), C, D], confirm delete of B,
    // then remove A (non-active) before Stopped arrives.  B's index
    // shifts from 1 → 0 in both queues, but the slot identity is stable.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(4);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_b = app.player_tab.queue.slots()[1].slot_id;
    app.handle_player_event(PlayerEvent::TrackChanged(1));
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 1;
    }
    // User confirms delete of B at index 1
    app.pending_delete_slot = Some(slot_b);

    // Before Stopped arrives, remove A (index 0) — B shifts to index 0
    let removed = app.player_tab.remove_slot_at(0);
    assert!(removed.is_some(), "A should be removed");

    // Player stops with B at its new index 0
    app.handle_player_event(PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 0,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    assert_eq!(
        app.player_tab.items.len(),
        2,
        "B should have been deleted (slot matched by identity despite index shift)"
    );
    // Verify C and D remain
    assert_eq!(app.player_tab.items[0].id, "id2");
    assert_eq!(app.player_tab.items[1].id, "id3");
}

#[test]
fn stale_pending_delete_slot_is_discarded_when_slot_does_not_match() {
    // If a pending delete slot doesn't resolve to the stopped idx
    // (e.g., the item was already consumed or the queue diverged),
    // the stale pending is discarded and normal stop handling applies.
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    app.player_tab.items = make_items(3);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let slot_0 = app.player_tab.queue.slots()[0].slot_id;
    // Pending targets slot 0
    app.pending_delete_slot = Some(slot_0);

    // Stopped reports idx 2 — a completely different slot
    app.handle_player_event(PlayerEvent::Stopped {
        idx: 2,
        position_ticks: 600_000_000,
        played: false,
        consume: false,
        progress_report_accepted: false,
        error: None,
    });

    // Slot 0 was not deleted; normal stop path applied progress to slot 2
    assert_eq!(
        app.player_tab.items.len(),
        3,
        "stale pending delete should be discarded, not applied to the wrong slot"
    );
    assert!(
        app.pending_delete_slot.is_none(),
        "stale pending should have been consumed"
    );
}
