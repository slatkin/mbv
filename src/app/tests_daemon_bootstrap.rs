use super::*;
use crate::app::tests::*;

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
            items: items
                .into_iter()
                .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor: 1,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.emby_items().len(), 2);
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
    let _connect_guard = DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
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
            items: items
                .into_iter()
                .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: positions.clone(),
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
    let last_played_id = items[1].id.clone();
    let bootstrap = bootstrap_local_daemon_queue(
        Vec::new(),
        0,
        crate::config::QueueSource::Unknown,
        Some(crate::config::QueueState {
            source: crate::config::QueueSource::Album,
            items: items
                .into_iter()
                .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor: 0,
            last_played_item_id: Some(last_played_id.clone()),
            last_played_completed: true,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.queue_cursor, 2);
    assert_eq!(
        bootstrap.last_played_item_id.as_deref(),
        Some(last_played_id.as_str())
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
            items: make_items(1)
                .into_iter()
                .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
                .collect(),
            cursor: 0,
            last_played_item_id: None,
            last_played_completed: false,
            positions: Default::default(),
        }),
    );

    assert_eq!(bootstrap.player_tab.emby_items().len(), 2);
    assert_eq!(bootstrap.player_tab.emby_items()[0].id, remote_items[0].id);
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
    assert_eq!(app.player_tab.emby_items().len(), 2);

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: make_items(5)
            .into_iter()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    })
    .expect("save queue state");

    app.maybe_restore_queue_state();

    assert_eq!(app.player_tab.emby_items().len(), 2);
    assert_eq!(app.player_tab.emby_items()[0].id, remote_items[0].id);
}

#[test]
fn queue_restore_uses_saved_cursor_when_last_played_is_missing() {
    let items: Vec<mbv_core::playback_queue::QueueItem> = make_items(3)
        .into_iter()
        .map(|i| mbv_core::playback_queue::QueueItem::Emby(Box::new(i)))
        .collect();
    let cursor = super::actions::queue_restore_cursor(&items, 2, None, false);
    assert_eq!(cursor, 2);
}
