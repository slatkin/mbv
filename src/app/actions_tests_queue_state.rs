#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::library_browse_actions::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, QueueScope,
};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerEvent;
use std::collections::HashMap;
use std::sync::mpsc;

fn folder(id: &str, name: &str) -> MediaItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

fn album(id: &str, name: &str) -> MediaItem {
    let mut item = make_item(name, "MusicAlbum");
    item.id = id.into();
    item.is_folder = true;
    item.media_type = "Audio".into();
    item
}

fn recursive_music_app() -> App {
    let mut app = make_app_stub();
    app.music_levels = vec!["group".into(), "artist".into(), "album".into()];
    let mut library = make_item("Music", "CollectionFolder");
    library.id = "music-lib".into();
    library.collection_type = "music".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    app
}
use crate::config::tests::SYS_ENV_LOCK as XDG_HOME_LOCK;

/// RAII guard that points `XDG_CONFIG_HOME` (subtitle-mode saves) and
/// test-only state-dir lookups (prefs/queue saves) at a fresh tempdir,
/// restoring and cleaning up on drop -- including on panic.
struct XdgHomeGuard {
    dir: std::path::PathBuf,
    _state_dir: crate::config::TestStateDirGuard,
}

impl XdgHomeGuard {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("mbv-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_CONFIG_HOME", &dir);
        std::env::remove_var("MBV_SYSTEM");
        let state_dir = crate::config::TestStateDirGuard::new_at(dir.join("mbv"));
        Self {
            dir,
            _state_dir: state_dir,
        }
    }
}

impl Drop for XdgHomeGuard {
    fn drop(&mut self) {
        std::env::remove_var("XDG_CONFIG_HOME");
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}
// ── queue_restore_cursor: last-played-id lookup + drift fallback ────────

#[test]
fn queue_restore_cursor_finds_last_played_by_id() {
    let items = crate::app::tests::make_items(3);
    let cursor = queue_restore_cursor(&items, 0, Some("id1"), false);
    assert_eq!(cursor, 1);
}

#[test]
fn queue_restore_cursor_advances_past_a_completed_last_played_item() {
    let items = crate::app::tests::make_items(3);
    let cursor = queue_restore_cursor(&items, 0, Some("id1"), true);
    assert_eq!(cursor, 2);
}

#[test]
fn queue_restore_cursor_falls_back_to_saved_cursor_when_last_played_id_missing() {
    let items = crate::app::tests::make_items(3);
    // "id5" isn't in the restored list (e.g. it was removed from the
    // queue before quitting) — must fall back to the saved cursor, not
    // silently snap back to the front of the queue.
    let cursor = queue_restore_cursor(&items, 2, Some("id5"), false);
    assert_eq!(cursor, 2);
}

#[test]
fn queue_restore_cursor_falls_back_to_saved_cursor_clamped_to_len() {
    let items = crate::app::tests::make_items(3);
    let cursor = queue_restore_cursor(&items, 99, Some("id5"), false);
    #[rustfmt::skip]
    assert_eq!(
        cursor, 2,
        "out-of-range saved cursor must clamp to the last valid index"
    );
}

#[test]
fn queue_restore_cursor_uses_saved_cursor_when_no_last_played_id() {
    let items = crate::app::tests::make_items(3);
    let cursor = queue_restore_cursor(&items, 1, None, false);
    assert_eq!(cursor, 1);
}

// ── queue_state persistence: restore + attached-session guards ──────────

#[test]
fn restore_queue_state_with_no_saved_file_does_nothing() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.restore_queue_state();

    assert!(app.player_tab.items.is_empty());
}

#[test]
fn restore_queue_state_with_no_items_does_nothing() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: vec![],
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    });

    let mut app = crate::app::tests::make_app_stub();
    app.restore_queue_state();

    assert!(app.player_tab.items.is_empty());
}

#[test]
fn restore_queue_state_populates_queue_synchronously_from_disk() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let items = crate::app::tests::make_items(3);
    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: items.clone(),
        cursor: 1,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    });

    let mut app = crate::app::tests::make_app_stub();
    app.restore_queue_state();

    // No network call is needed for the queue to already be correct —
    // this is a synchronous, local read, not a spawned background fetch.
    assert_eq!(app.player_tab.items.len(), 3);
    assert_eq!(app.player_tab.queue_cursor, 1);
}

#[test]
fn restore_queue_state_clears_a_stale_dirty_flag() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: crate::app::tests::make_items(1),
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    });

    let mut app = crate::app::tests::make_app_stub();
    app.queue_dirty = true;
    app.restore_queue_state();

    assert!(
        !app.queue_dirty,
        "restoring a queue from disk is not a local edit — it must not \
         leave a stale dirty flag that could trigger an unwanted \
         save_playlist_to_emby() push on the next consume"
    );
}

#[test]
fn quit_preserves_saved_playlist_source_for_restart_restore() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items = crate::app::tests::make_items(2);
    app.queue_source = crate::config::QueueSource::Playlist {
        id: Some("playlist-id".into()),
        name: "Saved Queue".into(),
    };
    app.queue_dirty = true;

    assert!(app.try_quit());
    app.save_queue_state_no_clear();

    let state = crate::config::load_queue_state().expect("queue state should be saved");
    assert_eq!(
        state.source,
        crate::config::QueueSource::Playlist {
            id: Some("playlist-id".into()),
            name: "Saved Queue".into(),
        },
        "shutdown persistence must keep the saved-playlist association so \
         a restart can still autosave/consume against the playlist"
    );
}

#[test]
fn handle_loaded_level_replaces_the_matching_loading_level() {
    let mut app = crate::app::tests::make_app_stub();
    let mut library = crate::app::tests::make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    app.libs.push(LibraryTab {
        library,
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
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
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
        }],
        search: None,
        feed_home_video: None,

        album_track_focus: None,
        artist_header_focus: None,
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
fn ensure_power_feed_library_preserves_saved_feed_position() {
    let mut app = crate::app::tests::make_app_stub();
    app.library_tab = 1;
    app.client.lock().unwrap().config.feed_view_libraries = vec!["youtube".into()];

    let mut library = crate::app::tests::make_item("YouTube", "CollectionFolder");
    library.id = "lib-feed".into();
    library.is_folder = true;
    library.collection_type = "homevideos".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 2,
            video_cursor: 3,
            video_scroll: 4,
            ..Default::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
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
fn ensure_power_podcast_library_preserves_saved_feed_position() {
    let mut app = crate::app::tests::make_app_stub();
    app.library_tab = 1;

    let mut library = crate::app::tests::make_item("Podcasts", "CollectionFolder");
    library.id = "lib-podcasts".into();
    library.is_folder = true;
    library.collection_type = "podcasts".into();
    app.libs.push(LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: Some(FeedHomeVideoState {
            selected_group: 1,
            video_cursor: 5,
            video_scroll: 6,
            ..Default::default()
        }),

        album_track_focus: None,
        artist_header_focus: None,
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
    app.player_tab.items = crate::app::tests::make_items(3); // id0, id1, id2
    app.player_tab.queue_cursor = 0;

    // The background fetch no longer returns id1 (e.g. deleted server-side).
    #[rustfmt::skip]
    let fresh = vec![app.player_tab.items[0].clone(), app.player_tab.items[2].clone()];
    app.handle_lib_event(LibEvent::QueueEnriched { items: fresh });

    let ids: Vec<&str> = app.player_tab.items.iter().map(|i| i.id.as_str()).collect();
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
    app.player_tab.items = crate::app::tests::make_items(3);
    let cmd_rx = app.player.spy_on_commands();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }

    let fresh = vec![
        app.player_tab.items[0].clone(),
        app.player_tab.items[2].clone(),
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
    app.player_tab.items = items;
    app.player_tab.items[0].playback_position_ticks = 3 * mbv_core::api::TICKS_PER_SECOND;
    app.player_tab.sync_queue_model_from_items_if_needed();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }

    // The fetch confirms id0 still exists, so slot 1's duplicate id0 would
    // also match by id alone if the skip weren't by-slot.
    let mut fresh = app.player_tab.items[0].clone();
    fresh.name = "Refreshed Name".to_string();
    app.handle_lib_event(LibEvent::QueueEnriched {
        items: vec![fresh.clone()],
    });

    assert_eq!(
        app.player_tab.items[0].playback_position_ticks,
        3 * mbv_core::api::TICKS_PER_SECOND,
        "the active slot must keep its authoritative local progress even though its id matched"
    );
    assert_eq!(
        app.player_tab.items[1].name, "Refreshed Name",
        "the non-active duplicate-id slot must still be enriched from the fresh fetch"
    );
}

#[test]
fn queue_enriched_skips_player_active_idx_not_queue_cursor() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items = crate::app::tests::make_items(2);
    app.player_tab.queue_cursor = 1;
    app.player_tab.items[0].playback_position_ticks = 3 * mbv_core::api::TICKS_PER_SECOND;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let mut stale = app.player_tab.items[0].clone();
    stale.playback_position_ticks = 46 * mbv_core::api::TICKS_PER_SECOND;

    app.handle_lib_event(LibEvent::QueueEnriched { items: vec![stale] });

    assert_eq!(
        app.player_tab.items[0].playback_position_ticks,
        3 * mbv_core::api::TICKS_PER_SECOND,
        "stale enrichment must not overwrite the actively playing slot"
    );
}

#[test]
fn queue_enriched_preserves_pending_sync_until_server_confirms_it() {
    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items = crate::app::tests::make_items(1);
    app.player_tab.sync_queue_model_from_items_if_needed();
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
    let mut stale = app.player_tab.items[0].clone();
    stale.playback_position_ticks = mbv_core::api::TICKS_PER_SECOND;

    app.handle_lib_event(LibEvent::QueueEnriched { items: vec![stale] });

    assert_eq!(
        app.player_tab.items[0].playback_position_ticks,
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
    app.player_tab.items = crate::app::tests::make_items(2);
    app.player_tab.sync_queue_model_from_items_if_needed();
    let active_slot = app.player_tab.queue.slots()[0].slot_id;
    let _ = app.player_tab.queue.apply_progress(
        active_slot,
        9 * mbv_core::api::TICKS_PER_SECOND,
        false,
    );
    app.player_tab.sync_items_from_queue_model();
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let mut stale_active = app.player_tab.items[0].clone();
    stale_active.playback_position_ticks = mbv_core::api::TICKS_PER_SECOND;
    let mut fresh_inactive = app.player_tab.items[1].clone();
    fresh_inactive.playback_position_ticks = 4 * mbv_core::api::TICKS_PER_SECOND;

    let _ = app.merge_refreshed_queue(QueueScope::Local, vec![stale_active, fresh_inactive]);

    assert_eq!(
        app.player_tab.items[0].playback_position_ticks,
        9 * mbv_core::api::TICKS_PER_SECOND
    );
    assert_eq!(
        app.player_tab.items[1].playback_position_ticks,
        4 * mbv_core::api::TICKS_PER_SECOND
    );
}

#[test]
fn save_queue_state_does_not_delete_file_while_attached_to_remote_session() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    // Seed an on-disk queue as if a previous local session left one behind.
    crate::config::save_queue_state(&crate::config::QueueState {
        source: crate::config::QueueSource::Unknown,
        items: crate::app::tests::make_items(2),
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    });

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items.clear();
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
        items: crate::app::tests::make_items(1),
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    });

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items.clear();
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
        items: crate::app::tests::make_items(1),
        cursor: 0,
        last_played_item_id: None,
        last_played_completed: false,
        positions: Default::default(),
    });

    let mut app = crate::app::tests::make_app_stub();
    app.player_tab.items.clear();
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
    app.player_tab.items = crate::app::tests::make_items(2);

    app.save_queue_state_no_clear();

    let state = crate::config::load_queue_state().expect("queue should be saved");
    assert_eq!(state.items.len(), 2);
}

#[test]
fn cycle_sub_local_idle_cycles_subtitle_mode_not_a_track() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    app.player.status.lock().unwrap().active = false;
    let before = app.client.lock().unwrap().config.subtitle_mode.clone();

    app.cycle_sub();

    let after = app.client.lock().unwrap().config.subtitle_mode.clone();
    assert_ne!(
        before, after,
        "idle z has no session equivalent, so it should still cycle the default subtitle mode"
    );
}

#[test]
fn cycle_sub_local_active_does_not_fall_back_to_subtitle_mode() {
    let _g = XDG_HOME_LOCK.lock().unwrap();
    let _xdg = XdgHomeGuard::new();

    let mut app = crate::app::tests::make_app_stub();
    {
        let mut status = app.player.status.lock().unwrap();
        status.active = true;
        status.sub_tracks = vec![(1, "English".to_string(), false)];
        status.sub_id = 0;
    }
    let before = app.client.lock().unwrap().config.subtitle_mode.clone();

    // #86: local `z` while active now cycles every track (like the
    // remote path) instead of the old on/off `toggle_sub()` -- assert at
    // minimum that it does *not* take the idle subtitle-mode fallback.
    app.cycle_sub();

    let after = app.client.lock().unwrap().config.subtitle_mode.clone();
    assert_eq!(
        before, after,
        "an active player has tracks to cycle and must not touch the idle subtitle-mode fallback"
    );
}
