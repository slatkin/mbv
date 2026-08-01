use super::*;
use crate::app::tests::*;
use mbv_core::playback_queue::{QueueRevision, QueueSlotId};

#[test]
fn local_daemon_bootstrap_adopts_saved_local_queue_and_source() {
    let items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Playlist {
                id: Some("pl1".into()),
                name: "Saved".into(),
            },
            items,
            cursor: 1,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
            slot_ids: None,
        revision: None,
        active_slot_id: None,
        next_slot_id: None,
    }),
    );

    assert_eq!(bootstrap.player_tab.items.len(), 2);
    assert_eq!(bootstrap.player_tab.queue_cursor, 1);
    assert!(matches!(
        bootstrap.queue_source,
        crate::config::QueueSource::Playlist { ref name, .. } if name == "Saved"
    ));
    assert!(matches!(
        bootstrap.adopt_queue,
        Some((_, 1, crate::config::QueueSource::Playlist { ref name, .. })) if name == "Saved"
    ));
}

#[test]
fn failed_local_daemon_adoption_routes_through_remote_disconnected() {
    // #119 task 5: a swallowed `adopt_queue()` send-failure must not
    // leave the app silently sitting on optimistic queue state the
    // daemon never received — it routes through the same handling a
    // live `PlayerEvent::RemoteDisconnected` uses.
    let mut app = make_local_daemon_app_stub(Vec::new());
    assert_eq!(app.queue_scope, QueueScope::Local);

    app.handle_failed_local_daemon_adoption();

    assert!(app.remote_player_tab.is_none());
    assert_eq!(app.queue_scope, QueueScope::Local);
    assert!(app.status.contains("daemon connection lost"));
}

#[test]
fn remote_app_starts_on_local_queue_when_remote_queue_is_empty() {
    let app = make_remote_app_stub(make_items(2), Vec::new());

    assert_eq!(app.queue_scope, QueueScope::Local);
    assert_eq!(app.visible_queue_scope(), QueueScope::Local);
}

#[test]
fn remote_app_starts_on_remote_queue_when_remote_queue_has_items() {
    let app = make_remote_app_stub(make_items(2), make_items(1));

    assert_eq!(app.queue_scope, QueueScope::Remote);
    assert_eq!(app.visible_queue_scope(), QueueScope::Remote);
}

#[test]
fn local_daemon_bootstrap_carries_saved_positions_for_enrichment() {
    let items = make_items(2);
    let mut positions = std::collections::HashMap::new();
    positions.insert(items[0].id.clone(), 999);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items,
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: positions.clone(),
        slot_ids: None,
        revision: None,
        active_slot_id: None,
        next_slot_id: None,
        }),
    );

    assert_eq!(bootstrap.positions, positions);
}

#[test]
fn local_daemon_bootstrap_has_no_positions_without_saved_state() {
    let bootstrap =
        bootstrap_local_daemon_queue(Vec::new(), 0, crate::config::QueueSource::Unknown, None);

    assert!(bootstrap.positions.is_empty());
}

#[test]
fn local_daemon_bootstrap_uses_restore_cursor_and_carries_last_played_state() {
    let items = make_items(3);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: items.clone(),
            cursor: 0,
            last_played_item_id: Some(items[1].id.clone()),
            last_played_completed: true,
            positions: Default::default(),
            slot_ids: None,
        revision: None,
        active_slot_id: None,
        next_slot_id: None,
    }),
    );

    assert_eq!(bootstrap.player_tab.queue_cursor, 2);
    assert_eq!(
        bootstrap.last_played_item_id.as_deref(),
        Some(items[1].id.as_str())
    );
    assert!(bootstrap.last_played_completed);
}

#[test]
fn local_daemon_bootstrap_prefers_existing_daemon_queue_state() {
    let remote_items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        remote_items.clone(),
        0,
        crate::config::QueueSource::Playlist {
            id: Some("daemon".into()),
            name: "Daemon Queue".into(),
        },
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Playlist {
                id: Some("local".into()),
                name: "Local Saved".into(),
            },
            items: make_items(1),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
            slot_ids: None,
        revision: None,
        active_slot_id: None,
        next_slot_id: None,
    }),
    );

    assert_eq!(bootstrap.player_tab.items.len(), 2);
    assert_eq!(bootstrap.player_tab.items[0].id, remote_items[0].id);
    assert!(matches!(
        bootstrap.queue_source,
        crate::config::QueueSource::Playlist { ref name, .. } if name == "Daemon Queue"
    ));
    assert!(bootstrap.adopt_queue.is_none());
}
// `item_text_and_style` and its dedicated tests above were deleted
// (#361): its only production caller was the deleted Standard
// `render/library/table/context.rs`.

#[test]
fn local_daemon_app_keeps_live_queue_over_stale_disk_snapshot() {
    // Once every attach goes through `bootstrap_local_daemon_queue`, this
    // is a realistic data-loss path: an already-adopted live daemon queue
    // must survive the startup disk restore, not be overwritten by
    // whatever `queue_state.json` happens to hold.
    let remote_items = make_items(2);
    let mut app = make_local_daemon_app_stub(remote_items.clone());
    assert_eq!(app.player_tab.items.len(), 2);

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: make_items(5),
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
        slot_ids: None,
        revision: None,
        active_slot_id: None,
        next_slot_id: None,
    });

    app.maybe_restore_queue_state();

    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(app.player_tab.items[0].id, remote_items[0].id);
}

#[test]
fn queue_restore_uses_saved_cursor_when_last_played_is_missing() {
    let items = make_items(3);
    let cursor = super::actions::queue_restore_cursor(&items, 2, None, false);
    assert_eq!(cursor, 2);
}

// ── slot-aware bootstrap tests (task 10.3) ──────────────────────────

#[test]
fn cold_adoption_learns_slot_ids_from_persisted_state() {
    let items = make_items(3);
    let slot_ids: Vec<u64> = vec![100, 200, 300];
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: items.clone(),
            cursor: 1,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
            slot_ids: Some(slot_ids),
            revision: Some(5),
            active_slot_id: Some(200),
            next_slot_id: Some(301),
        }),
    );

    // Items are restored in slot order.
    assert_eq!(bootstrap.player_tab.items.len(), 3);
    assert_eq!(bootstrap.player_tab.items[0].id, items[0].id);
    assert_eq!(bootstrap.player_tab.items[1].id, items[1].id);
    assert_eq!(bootstrap.player_tab.items[2].id, items[2].id);

    // The queue model preserves slot identities from persisted data.
    assert_eq!(bootstrap.player_tab.queue.slots().len(), 3);
    assert_eq!(
        bootstrap.player_tab.queue.slots()[0].slot_id,
        QueueSlotId(100)
    );
    assert_eq!(
        bootstrap.player_tab.queue.slots()[1].slot_id,
        QueueSlotId(200)
    );
    assert_eq!(
        bootstrap.player_tab.queue.slots()[2].slot_id,
        QueueSlotId(300)
    );
    assert_eq!(
        bootstrap.player_tab.queue.active_slot_id(),
        Some(QueueSlotId(200))
    );
    assert_eq!(bootstrap.player_tab.queue.revision(), QueueRevision(5));

    // Active slot is selected by default.
    assert_eq!(
        bootstrap.player_tab.selected_slot_id,
        Some(QueueSlotId(200))
    );

    // Cursor points to the active slot.
    assert_eq!(bootstrap.player_tab.queue_cursor, 1);

    // Adopt queue is populated for cold daemon flow.
    assert!(bootstrap.adopt_queue.is_some());
    let (adopt_items, adopt_cursor, _source) = bootstrap.adopt_queue.unwrap();
    assert_eq!(adopt_items.len(), 3);
    assert_eq!(adopt_cursor, 1);
}

#[test]
fn cold_adoption_allocates_fresh_slot_ids_when_persisted_has_no_slot_data() {
    let items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: items.clone(),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
            slot_ids: None,
            revision: None,
            active_slot_id: None,
            next_slot_id: None,
        }),
    );

    // Items are restored even without slot IDs.
    assert_eq!(bootstrap.player_tab.items.len(), 2);

    // Fresh slot IDs are allocated (starting from 1 by default).
    let slots = bootstrap.player_tab.queue.slots();
    assert_eq!(slots.len(), 2);
    // default next_slot_id starts at 1 so first allocated is id=1
    assert_eq!(slots[0].slot_id, QueueSlotId(1));
    assert_eq!(slots[1].slot_id, QueueSlotId(2));
    assert!(bootstrap.player_tab.queue.active_slot_id().is_none());
}

#[test]
fn warm_daemon_bootstrap_defaults_selection_to_first_slot() {
    let remote_items = make_items(3);
    let bootstrap = bootstrap_local_daemon_queue(
        remote_items.clone(),
        0,
        crate::config::QueueSource::Playlist {
            id: Some("pl".into()),
            name: "Warm".into(),
        },
        None,
    );

    // Warm daemon: remote_items are non-empty, so bootstrap uses them directly.
    assert_eq!(bootstrap.player_tab.items.len(), 3);
    assert_eq!(bootstrap.player_tab.queue_cursor, 0);

    // Adopt is None for warm daemon (queue already exists at daemon).
    assert!(bootstrap.adopt_queue.is_none());

    // Selection defaults to the first visible item.
    let first_slot = bootstrap.player_tab.queue.slots().first().unwrap().slot_id;
    assert_eq!(bootstrap.player_tab.selected_slot_id, Some(first_slot));
}

#[test]
fn reconnect_clears_adopt_queue() {
    // Warm daemon with non-empty items: adopt_queue is None
    let remote_items = make_items(2);
    let bootstrap = bootstrap_local_daemon_queue(
        remote_items,
        0,
        crate::config::QueueSource::Unknown,
        None,
    );
    // Reconnect (warm) should have no adoption.
    assert!(bootstrap.adopt_queue.is_none());
    assert_eq!(bootstrap.player_tab.items.len(), 2);
}

#[test]
fn from_slot_snapshot_preserves_daemon_assigned_slot_ids_and_active_selection() {
    let items = make_items(3);
    let slot_ids = vec![QueueSlotId(10), QueueSlotId(20), QueueSlotId(30)];
    let tab = PlayerTab::from_slot_snapshot(
        items.clone(),
        slot_ids.clone(),
        Some(QueueSlotId(20)), // active slot
        QueueRevision(3),
        1, // cursor at position 1 (the active slot)
    );

    assert_eq!(tab.items.len(), 3);
    assert_eq!(tab.queue.slots().len(), 3);
    assert_eq!(tab.queue.slots()[0].slot_id, QueueSlotId(10));
    assert_eq!(tab.queue.slots()[1].slot_id, QueueSlotId(20));
    assert_eq!(tab.queue.slots()[2].slot_id, QueueSlotId(30));
    assert_eq!(tab.queue.active_slot_id(), Some(QueueSlotId(20)));
    assert_eq!(tab.queue.revision(), QueueRevision(3));
    assert_eq!(tab.queue_cursor, 1);
    // Selection defaults to the active slot on initial load.
    assert_eq!(tab.selected_slot_id, Some(QueueSlotId(20)));
}

#[test]
fn from_slot_snapshot_without_active_falls_back_to_first_slot() {
    let items = make_items(2);
    let slot_ids = vec![QueueSlotId(5), QueueSlotId(6)];
    let tab = PlayerTab::from_slot_snapshot(
        items.clone(),
        slot_ids.clone(),
        None, // no active slot
        QueueRevision(0),
        0,
    );

    assert_eq!(tab.items.len(), 2);
    assert!(tab.queue.active_slot_id().is_none());
    // Falls back to first slot.
    assert_eq!(tab.selected_slot_id, Some(QueueSlotId(5)));
    assert_eq!(tab.queue_cursor, 0);
}

