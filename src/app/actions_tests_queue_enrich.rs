use super::queue_state_tests::XdgHomeGuard;
use crate::app::{BrowseLevel, FeedHomeVideoState, LibEvent, LibraryTab, QueueScope, TabSelection};

use crate::config::tests::SYS_ENV_LOCK as XDG_HOME_LOCK;

#[test]
fn handle_loaded_level_replaces_the_matching_loading_level() {
    let mut app = crate::app::tests::make_app_stub();
    let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "parent".into(),
            title: "Loading".into(),
            items: vec![],
            total_count: 0,
            cursor: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: true,
            scroll: 0,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    let level = BrowseLevel {
        parent_id: "parent".into(),
        title: "Loaded".into(),
        items: crate::app::tests::make_items(2),
        total_count: 2,
        cursor: 1,
        item_types: None,
        unplayed_only: false,
        sort_by: "DateCreated".into(),
        sort_order: "Descending".into(),
        loading: false,
        scroll: 3,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    };

    app.handle_loaded_level(0, "parent".into(), level);

    let last = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(last.title, "Loaded");
    assert_eq!(last.items.len(), 2);
    assert_eq!(last.total_count, 2);
    assert_eq!(last.cursor, 1);
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
        library,
        search: None,
        nav_stack: vec![BrowseLevel {
            parent_id: "series".into(),
            title: "Season 1".into(),
            items: vec![second, first],
            total_count: 2,
            cursor: 0,
            item_types: Some("Episode".into()),
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
            scroll: 0,
            all_items: None,
            letter_filter: None,
            music_grouping: None,
        }],
        feed_home_video: None,

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.normalize_current_browse_level_items(0);

    let last = app.libs[0].nav_stack.last().unwrap();
    let names: Vec<&str> = last.items.iter().map(|item| item.name.as_str()).collect();
    assert_eq!(names, vec!["Episode 1", "Episode 2"]);
}

#[test]
fn ensure_feed_library_preserves_saved_feed_position() {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);
    app.config.lock().unwrap().feed_view_libraries = vec!["youtube".into()];

    let mut library = crate::app::tests::make_item("YouTube", "CollectionFolder");
    library.id = "lib-feed".into();
    library.is_folder = true;
    library.collection_type = "homevideos".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 2,
            video_cursor: 3,
            video_scroll: 4,
            ..Default::default()
        }),

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.ensure_lib_loaded_for(0);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.loading);
    assert_eq!(state.selected_group, 2);
    assert_eq!(state.video_cursor, 3);
    assert_eq!(state.video_scroll, 4);
}

#[test]
fn ensure_podcast_library_preserves_saved_feed_position() {
    let mut app = crate::app::tests::make_app_stub();
    app.tab = TabSelection::EmbyLibrary(0);

    let mut library = crate::app::tests::make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.is_folder = true;
    library.collection_type = "podcasts".into();
    app.libs.push(LibraryTab {
        library,
        search: None,
        nav_stack: Vec::new(),
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 1,
            video_cursor: 5,
            video_scroll: 6,
            ..Default::default()
        }),

        album_track_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });

    app.ensure_lib_loaded_for(0);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(state.loading);
    assert_eq!(state.selected_group, 1);
    assert_eq!(state.video_cursor, 5);
    assert_eq!(state.video_scroll, 6);
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
    }

    let fresh = vec![
        app.player_tab.emby_items()[0].clone(),
        app.player_tab.emby_items()[2].clone(),
    ];
    app.handle_lib_event(LibEvent::QueueEnriched { items: fresh });

    assert!(
        matches!(
            cmd_rx.try_recv(),
            Ok(crate::player::PlayerCommand::QueueRemove(1))
        ),
        "pruning a live playback queue slot must also remove it from the player's private queue copy"
    );
}

#[test]
fn queue_enriched_never_prunes_or_merges_the_active_slot_even_with_a_duplicate_id() {
    let mut app = crate::app::tests::make_app_stub();
    let mut items = crate::app::tests::make_items(2); // id0, id1
    items[1].id = "id0".to_string(); // duplicate of the active item's id
    app.player_tab.set_items(items, app.player_tab.queue_cursor);
    app.player_tab
        .set_slot_progress_at(0, 3 * mbv_core::api::TICKS_PER_SECOND);
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }

    // The fetch confirms id0 still exists, so slot 1's duplicate id0 would
    // also match by id alone if the skip weren't by-slot.
    let mut fresh = app.player_tab.emby_items()[0].clone();
    fresh.name = "Refreshed Name".to_string();
    app.handle_lib_event(LibEvent::QueueEnriched {
        items: vec![fresh.clone()],
    });

    assert_eq!(
        app.player_tab.emby_items()[0].playback_position_ticks,
        3 * mbv_core::api::TICKS_PER_SECOND,
        "the active slot must keep its authoritative local progress even though its id matched"
    );
    assert_eq!(
        app.player_tab.emby_items()[1].name,
        "Refreshed Name",
        "the non-active duplicate-id slot must still be enriched from the fresh fetch"
    );
}

#[test]
fn queue_enriched_skips_player_active_idx_not_queue_cursor() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(2),
        app.player_tab.queue_cursor,
    );
    app.player_tab.queue_cursor = 1;
    app.player_tab
        .set_slot_progress_at(0, 3 * mbv_core::api::TICKS_PER_SECOND);
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let mut stale = app.player_tab.emby_items()[0].clone();
    stale.playback_position_ticks = 46 * mbv_core::api::TICKS_PER_SECOND;

    app.handle_lib_event(LibEvent::QueueEnriched { items: vec![stale] });

    assert_eq!(
        app.player_tab.emby_items()[0].playback_position_ticks,
        3 * mbv_core::api::TICKS_PER_SECOND,
        "stale enrichment must not overwrite the actively playing slot"
    );
}

#[test]
fn queue_enriched_preserves_pending_sync_until_server_confirms_it() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(1),
        app.player_tab.queue_cursor,
    );
    app.handle_player_event(mbv_core::player::PlayerEvent::TrackChanged(0));
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    app.handle_player_event(mbv_core::player::PlayerEvent::Stopped {
        idx: 0,
        position_ticks: 6 * mbv_core::api::TICKS_PER_SECOND,
        played: false,
        consume: false,
        progress_report_accepted: true,
        error: None,
    });
    let mut stale = app.player_tab.emby_items()[0].clone();
    stale.playback_position_ticks = mbv_core::api::TICKS_PER_SECOND;

    app.handle_lib_event(LibEvent::QueueEnriched { items: vec![stale] });

    assert_eq!(
        app.player_tab.emby_items()[0].playback_position_ticks,
        6 * mbv_core::api::TICKS_PER_SECOND,
        "stale enrichment must not overwrite accepted local stopped progress while sync is pending"
    );
    assert!(app.player_tab.queue.slots()[0]
        .progress_state
        .pending_sync
        .is_some());
}

#[test]
fn manual_refresh_merge_uses_queue_model_active_slot_protection() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(2),
        app.player_tab.queue_cursor,
    );
    let active_slot = app.player_tab.queue.slots()[0].slot_id;
    let _ = app.player_tab.queue.apply_progress(
        active_slot,
        9 * mbv_core::api::TICKS_PER_SECOND,
        false,
    );
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let mut stale_active = app.player_tab.emby_items()[0].clone();
    stale_active.playback_position_ticks = mbv_core::api::TICKS_PER_SECOND;
    let mut fresh_inactive = app.player_tab.emby_items()[1].clone();
    fresh_inactive.playback_position_ticks = 4 * mbv_core::api::TICKS_PER_SECOND;

    let _ = app.merge_refreshed_queue(QueueScope::Local, vec![stale_active, fresh_inactive]);

    assert_eq!(
        app.player_tab.emby_items()[0].playback_position_ticks,
        9 * mbv_core::api::TICKS_PER_SECOND
    );
    assert_eq!(
        app.player_tab.emby_items()[1].playback_position_ticks,
        4 * mbv_core::api::TICKS_PER_SECOND
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
        positions: Default::default(),
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
        positions: Default::default(),
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

#[test]
fn save_queue_state_no_clear_still_saves_when_queue_has_items() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.set_items(
        crate::app::tests::make_items(2),
        app.player_tab.queue_cursor,
    );

    app.save_queue_state_no_clear();

    let state = crate::config::load_queue_state().expect("queue should be saved");
    assert_eq!(state.items.len(), 2);
}
