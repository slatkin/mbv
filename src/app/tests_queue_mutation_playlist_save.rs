//! Playlist identity + save coordination tests, split out of
//! `tests_queue_mutation.rs` to keep that file within the repository's
//! file-size limit.

use crate::app::tests::*;
use crate::app::*;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

// ── playlist identity + save coordination (isolate-remote-tracking-client-behavior) ──

fn saved_playlist_app() -> App {
    let mut app = make_app_stub();
    let mut items = make_items(2);
    items[0].playlist_item_id = "entry-0".into();
    items[1].playlist_item_id = "entry-1".into();
    app.player_tab.set_items(items, app.player_tab.queue_cursor);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("pl-1".into()),
        name: "A".into(),
    };
    app
}

fn track_source(app: &mut App, playlist_id: &str) {
    let items = app.player_tab.emby_items();
    app.remote_tracker =
        App::build_remote_tracker_with_source("session", &items, 0, 5, Some(playlist_id.into()));
    assert!(app.remote_tracker.is_some(), "test tracker must build");
}

fn consume_occurrence(app: &mut App, slot_index: usize) {
    let slot = app.player_tab.queue.slots()[slot_index].slot_id;
    assert!(matches!(
        app.player_tab.queue.consume_slot(slot),
        mbv_core::playback_queue::QueueMutationResult::Applied(_)
    ));
}

#[test]
fn untracked_save_invalidates_and_persists_entry_identities() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    let lineage = app.remote_queue_lineage;

    app.save_playlist_to_emby();

    assert!(
        app.player_tab
            .emby_items()
            .iter()
            .all(|item| item.playlist_item_id.is_empty()),
        "an untracked save recreates server entry IDs and must still clear the local identities"
    );
    assert_eq!(app.remote_queue_lineage, lineage);
    let persisted = crate::config::load_queue_state().expect("cleared identity persisted");
    assert!(persisted
        .emby_items()
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
        .emby_items()
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
        .emby_items()
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
        app.player_tab.emby_items()[0].playlist_item_id,
        "entry-0",
        "the current source's identities are not recreated by an unrelated overwrite"
    );
}

#[test]
fn save_after_consumed_projection_snapshots_the_projected_queue() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    consume_occurrence(&mut app, 0);
    assert_eq!(app.player_tab.emby_items().len(), 1);

    app.save_playlist_to_emby();

    let item_ids = match app
        .playlist_mutations
        .get("pl-1")
        .and_then(|state| state.active.as_ref())
    {
        Some(crate::app::types_playback::PlaylistMutation::Save {
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
    assert!(app.player_tab.emby_items().is_empty());
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
        Some(crate::app::types_playback::PlaylistMutation::Save { mutation_id: 1, .. })
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
        Some(crate::app::types_playback::PlaylistMutation::Replace { .. })
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
            .emby_items()
            .iter()
            .all(|item| item.playlist_item_id.is_empty()),
        "the new source must never retain entry identities from the old playlist"
    );
    let persisted = crate::config::load_queue_state().expect("save-as persisted");
    assert!(persisted
        .emby_items()
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
        app.player_tab.emby_items()[0].playlist_item_id,
        "entry-0",
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
        .emby_items()
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
    let persisted = crate::config::load_queue_state().expect("overwrite persisted");
    assert!(matches!(
        persisted.source,
        crate::config::QueueSource::Playlist { id: Some(ref id), .. } if id == "pl-2"
    ));
    assert!(persisted
        .emby_items()
        .iter()
        .all(|item| item.playlist_item_id.is_empty()));
}

#[test]
fn save_and_save_on_quit_cannot_resurrect_a_consumed_occurrence() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    consume_occurrence(&mut app, 0);
    assert_eq!(app.player_tab.emby_items().len(), 1);

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
    assert_eq!(app.player_tab.emby_items().len(), 1);

    // Save-on-quit snapshots the same projected queue, so the consumed
    // occurrence is never re-added by the quit save.
    app.queue_dirty = true;
    app.client.lock().unwrap().config.save_playlist_on_quit = true;
    assert!(app.try_quit());
    let persisted = crate::config::load_queue_state().expect("queue persisted");
    assert_eq!(persisted.items.len(), 1);
    assert_eq!(persisted.items[0].id(), "id1");
}
