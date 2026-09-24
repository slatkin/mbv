//! Playlist identity + save coordination tests, split out of
//! `tests_queue_mutation.rs` to keep that file within the repository's
//! file-size limit.

use crate::app::tests::*;
use crate::app::*;


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

fn consume_occurrence(app: &mut App, slot_index: usize) {
    let slot = app.player_tab.queue.slots()[slot_index].slot_id;
    assert!(matches!(
        app.player_tab.queue.consume_slot(slot),
        mbv_core::playback_queue::QueueMutationResult::Applied(_)
    ));
}

#[test]
fn stale_playlist_mutation_completion_is_rejected_after_queue_change() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = saved_playlist_app();
    app.save_playlist_to_emby();
    let stale_lineage = app.remote_queue_lineage;
    app.remove_from_queue(0);
    assert!(app.remote_queue_lineage > stale_lineage);
    app.queue_dirty = true;

    app.handle_session_event(SessionEvent::PlaylistMutationComplete {
        mutation_id: 1,
        playlist_id: "pl-1".into(),
        queue_lineage: stale_lineage,
        source_playlist_id: "pl-1".into(),
        result: Ok(()),
    });

    assert!(app.queue_dirty, "stale completion must not clear current queue edits");
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
    app.config.lock().unwrap().save_playlist_on_quit = true;
    app.config.lock().unwrap().quit_timeout_secs = 0;
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
        owner_queue_lineage: None,
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
fn stay_alive_save_as_sends_source_update_with_observed_owner_lineage() {
    let _guard = crate::config::TestStateDirGuard::new();
    let items = make_items(2);
    let config = crate::config::Config {
        stay_alive: true,
        ..Default::default()
    };
    let (remote, player_rx, commands) =
        mbv_core::remote_player::RemotePlayer::stub_owner_queue_load_with_command_rx(
            items.clone(),
            0,
        );
    let mut app = App::new_remote_with_config(
        mbv_core::api::EmbyClient::new(config.clone()),
        remote,
        player_rx,
        mbv_core::remote_player::DaemonEndpoint::Local,
        config,
    );
    while commands.try_recv().is_ok() {}
    let old_source = crate::config::QueueSource::Playlist {
        id: Some("pl-1".into()),
        name: "A".into(),
    };
    let mut snapshot = emby_unified_state(&items, 0);
    snapshot.status.active = true;
    snapshot.source = old_source.clone();
    snapshot.lineage = mbv_core::ctrl::QueueLineage(11);
    app.player
        .as_remote()
        .unwrap()
        .unified_queue
        .lock()
        .unwrap()
        .replace(snapshot.clone());
    app.handle_player_event(mbv_core::player::PlayerEvent::UnifiedQueueUpdated(Box::new(
        snapshot,
    )));
    let slots_before: Vec<_> = app
        .player_tab
        .queue
        .slots()
        .iter()
        .map(|slot| (slot.slot_id, slot.item.id().to_string()))
        .collect();
    let status_before = {
        let status = app.player.status.lock().unwrap();
        (status.active, status.sequence_generation, status.current_idx, status.queue_len)
    };

    app.save_queue_as_playlist("B".into());
    let mutation_id = app.next_playlist_mutation - 1;
    let coordinator_key = format!("create:{mutation_id}");
    let owner_lineage = match app.playlist_mutations[&coordinator_key].active.as_ref().unwrap() {
        crate::app::types_playback::PlaylistMutation::CreateAs {
            owner_queue_lineage, ..
        } => *owner_queue_lineage,
        _ => panic!("expected Save As mutation"),
    };
    assert_eq!(owner_lineage, Some(mbv_core::ctrl::QueueLineage(11)));

    app.handle_session_event(SessionEvent::PlaylistCreateComplete {
        mutation_id,
        coordinator_key,
        name: "B".into(),
        queue_lineage: app.remote_queue_lineage,
        source_playlist_id: Some("pl-1".into()),
        owner_queue_lineage: owner_lineage,
        result: Ok("pl-2".into()),
    });

    assert!(matches!(commands.try_recv(), Ok(mbv_core::ctrl::CtrlCmd::UnifiedQueueSourceUpdate {
        source: crate::config::QueueSource::Playlist { id: Some(ref id), .. },
        lineage: mbv_core::ctrl::QueueLineage(11),
    }) if id == "pl-2"));
    assert_eq!(app.queue_source, old_source, "the Client waits for the owner's snapshot");
    assert_eq!(
        app.player_tab.queue.slots().iter().map(|slot| (slot.slot_id, slot.item.id().to_string())).collect::<Vec<_>>(),
        slots_before,
    );
    {
        let status = app.player.status.lock().unwrap();
        assert_eq!(
            (status.active, status.sequence_generation, status.current_idx, status.queue_len),
            status_before,
        );
    }

    app.handle_player_event(mbv_core::player::PlayerEvent::CommandRejected(
        "queue source update rejected: owner queue lineage changed".into(),
    ));
    assert_eq!(app.queue_source, old_source, "owner rejection must not rename locally");
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
    app.config.lock().unwrap().save_playlist_on_quit = true;
    app.config.lock().unwrap().quit_timeout_secs = 0;
    assert!(app.try_quit());
    let persisted = crate::config::load_queue_state().expect("queue persisted");
    assert_eq!(persisted.items.len(), 1);
    assert_eq!(persisted.items[0].id(), "id1");
}
