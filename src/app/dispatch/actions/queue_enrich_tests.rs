use super::queue_state_tests::XdgHomeGuard;
use crate::app::state::types::browse::BrowseResting;
use crate::app::{BrowseLevel, LibEvent, LibraryTab};

use crate::config::tests::SYS_ENV_LOCK as XDG_HOME_LOCK;

#[test]
fn handle_loaded_level_replaces_the_matching_loading_level() {
    let mut app = crate::app::tests::make_app_stub();
    let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "parent".into(),
            title: "Loading".into(),
            items: vec![],
            total_count: 0,
            resting: BrowseResting::new(0, 0),
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    let level = BrowseLevel {
        fetched_rows: 0,
        parent_id: "parent".into(),
        title: "Loaded".into(),
        items: crate::app::tests::make_items(2),
        total_count: 2,
        resting: BrowseResting::new(1, 3),
        item_types: None,
        unplayed_only: false,
        sort_by: "DateCreated".into(),
        sort_order: "Descending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        tv_content_mode: None,
        music_grouping: None,
    };

    app.handle_loaded_level(0, "parent", level);

    let last = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(last.title, "Loaded");
    assert_eq!(last.items.len(), 2);
    assert_eq!(last.total_count, 2);
    assert_eq!(last.resting().cursor(), 1);
    assert_eq!(last.sort_by, "DateCreated");
    assert_eq!(last.sort_order, "Descending");
    assert!(!last.loading);
}

#[test]
fn normalize_current_browse_level_items_sorts_episode_lists() {
    let mut app = crate::app::tests::make_app_stub();
    let mut second = crate::app::tests::make_item("Episode 2", "Episode");
    second.index_number = 2;
    let mut first = crate::app::tests::make_item("Episode 1", "Episode");
    first.index_number = 1;
    let mut library = crate::app::tests::make_item("TV", "CollectionFolder");
    library.id = "lib-tv".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "series".into(),
            title: "Season 1".into(),
            items: vec![second, first],
            total_count: 2,
            resting: BrowseResting::new(0, 0),
            item_types: Some("Episode".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            all_items: None,
            letter_filter: None,
            tv_content_mode: None,
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    app.normalize_current_browse_level_items(0);

    let last = app.libs[0].nav_stack.last().unwrap();
    let names: Vec<&str> = last.items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["Episode 1", "Episode 2"]);
}

#[test]
fn queue_enriched_prunes_items_the_server_no_longer_returns() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(3),
        app.player_tab.queue_cursor,
    ); // id0, id1, id2
    app.player_tab.queue_cursor = 0;

    // The background fetch no longer returns id1 (e.g. deleted server-side).
    #[rustfmt::skip]
    let fresh = vec![app.player_tab.emby_items()[0].clone(), app.player_tab.emby_items()[2].clone()];
    app.handle_lib_event(LibEvent::QueueEnriched { items: fresh });

    let current_items = app.player_tab.emby_items();
    let ids: Vec<&str> = current_items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["id0", "id2"],
        "an item missing from the fresh fetch must be pruned from the \
         restored queue, not left stale forever"
    );
    assert_eq!(
        app.player_tab.queue_cursor, 0,
        "removing an item after the cursor must not shift the cursor"
    );
}

#[test]
fn queue_enriched_prunes_live_playback_slots_and_resyncs_player_queue() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(3),
        app.player_tab.queue_cursor,
    );
    let cmd_rx = app.player.spy_on_commands();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    };

    let fresh = vec![
        app.player_tab.emby_items()[0].clone(),
        app.player_tab.emby_items()[2].clone(),
    ];
    app.handle_lib_event(LibEvent::QueueEnriched { items: fresh });

    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(crate::player::PlayerCommand::QueueRemove(_))
        ),
        "pruning a live playback queue slot must also remove it from the player's private queue copy"
    );
}

#[test]
fn save_queue_state_does_not_delete_file_while_attached_to_remote_session() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    // Seed an on-disk queue as if a previous local session left one behind.
    crate::config::save_queue_state(&crate::app::tests::make_queue_state(
        crate::app::tests::make_items(1),
    ))
    .expect("save queue state");

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.clear();
    app.connected_session_id = Some("session-1".into());

    app.save_queue_state();

    assert!(
        crate::config::load_queue_state().is_some(),
        "an empty local tab while attached to a remote session must not delete the \
         saved queue — that emptiness reflects remote-control UI state, not the user \
         clearing their queue"
    );
}

#[test]
fn save_queue_state_still_clears_file_when_locally_empty_and_not_attached() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: crate::app::tests::make_items(1)
            .into_iter()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .expect("save queue state");

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.clear();
    app.connected_session_id = None;

    app.save_queue_state();

    assert!(
        crate::config::load_queue_state().is_none(),
        "a genuinely empty local queue with no remote session attached should still clear"
    );
}

#[test]
fn save_queue_state_no_clear_preserves_file_when_locally_empty_and_not_attached() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    // Seed an on-disk queue as if a previous session left one behind — this
    // session never touched the local queue tab (e.g. only browsed Home).
    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: crate::app::tests::make_items(1)
            .into_iter()
            .map(|item| mbv_core::playback_queue::QueueItem::Emby(Box::new(item)))
            .collect(),
        cursor: 0,
        last_played_content_id: None,
        last_played_item_id: None,
        last_played_completed: false,
        positions: std::collections::HashMap::default(),
    })
    .expect("save queue state");

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.clear();
    app.connected_session_id = None;

    app.save_queue_state_no_clear();

    assert!(
        crate::config::load_queue_state().is_some(),
        "quitting with a transiently-empty in-memory queue must not delete an \
         existing on-disk snapshot — only an explicit user-initiated clear should"
    );
}
