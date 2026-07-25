use super::*;
use crate::app::library_browse_actions::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction, LibraryTab,
};
use mbv_core::player::PlayerEvent;
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

#[test]
fn album_index_eligibility_requires_grouped_music_ending_in_album() {
    assert!(recursive_album_search_eligible(
        "music",
        &["group".into(), "album".into()]
    ));
    assert!(recursive_album_search_eligible(
        "music",
        &["group".into(), "artist".into(), "album".into()]
    ));
    assert!(!recursive_album_search_eligible("music", &[]));
    assert!(!recursive_album_search_eligible("music", &["album".into()]));
    assert!(!recursive_album_search_eligible(
        "music",
        &["group".into(), "artist".into()]
    ));
    assert!(!recursive_album_search_eligible(
        "movies",
        &["group".into(), "album".into()]
    ));
}

#[test]
fn album_index_traverses_deep_branches_pages_and_ignores_non_albums() {
    let mut tree = HashMap::new();
    tree.insert(
        "music-lib".to_string(),
        vec![folder("group-a", "A"), folder("group-b", "B")],
    );
    tree.insert(
        "group-a".to_string(),
        vec![
            folder("artist-empty", "Empty"),
            folder("artist-a", "Artist A"),
        ],
    );
    tree.insert("artist-empty".to_string(), Vec::new());
    let mut many_albums: Vec<MediaItem> = (0..201)
        .map(|index| album(&format!("album-a-{index}"), &format!("Record {index}")))
        .collect();
    many_albums.push(make_item("Not an album", "Audio"));
    tree.insert("artist-a".to_string(), many_albums);
    tree.insert("group-b".to_string(), vec![folder("artist-b", "Artist B")]);
    tree.insert("artist-b".to_string(), vec![album("album-b", "Record 0")]);
    let mut calls = Vec::new();
    let mut fetch = |parent: &str, start: usize, limit: usize| {
        calls.push((parent.to_string(), start));
        let all = tree.get(parent).cloned().unwrap_or_default();
        let page = all.iter().skip(start).take(limit).cloned().collect();
        Ok((page, all.len()))
    };

    let entries = build_album_index_with(
        "music-lib",
        &["group".into(), "artist".into(), "album".into()],
        &mut fetch,
    )
    .unwrap();

    assert_eq!(entries.len(), 202);
    assert_eq!(
        entries.last().unwrap().display_label,
        "B / Artist B / Record 0"
    );
    assert_eq!(
        entries.last().unwrap().ancestors,
        vec![
            AlbumPathPart {
                id: "group-b".into(),
                name: "B".into()
            },
            AlbumPathPart {
                id: "artist-b".into(),
                name: "Artist B".into()
            }
        ]
    );
    assert!(calls.contains(&("artist-a".into(), 200)));
    assert!(entries
        .iter()
        .all(|entry| entry.album.item_type == "MusicAlbum"));
}

#[test]
fn recursive_album_search_matches_ancestor_labels() {
    let mut app = recursive_music_app();
    let target = album("album-1", "Needle Record");
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Ready(vec![AlbumSearchEntry {
            album: target,
            ancestors: vec![AlbumPathPart {
                id: "group-a".into(),
                name: "Deep Group".into(),
            }],
            display_label: "Deep Group / Needle Record".into(),
            search_text: "Deep Group / Needle Record".into(),
        }]),
    );

    assert!(app.open_recursive_album_search(0));
    app.libs[0].search.as_mut().unwrap().query = "deep grp".into();
    app.update_lib_search(0);

    assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
    assert_eq!(
        app.recursive_album_display_item(0, 0, album("album-1", "Needle Record"))
            .name,
        "Deep Group / Needle Record"
    );

    app.libs[0].search.as_mut().unwrap().query = "needle rec".into();
    app.update_lib_search(0);
    assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
}

#[test]
fn album_only_music_keeps_visible_list_search() {
    let mut app = recursive_music_app();
    app.music_levels = vec!["album".into()];
    app.libs[0].search = Some(super::super::LibSearch {
        query: "visible rec".into(),
        items: vec![album("album-1", "Visible Record")],
        results: Vec::new(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });

    assert!(!app.open_recursive_album_search(0));
    app.update_lib_search(0);

    assert_eq!(app.libs[0].search.as_ref().unwrap().results, vec![0]);
}

#[test]
fn album_index_completion_updates_the_open_current_query() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );
    assert!(app.open_recursive_album_search(0));
    app.libs[0].search.as_mut().unwrap().query = "remote group".into();

    app.handle_lib_event(LibEvent::AlbumIndexBuilt {
        library_id: "music-lib".into(),
        result: Ok(vec![AlbumSearchEntry {
            album: album("album-1", "Record"),
            ancestors: vec![AlbumPathPart {
                id: "group-a".into(),
                name: "Remote Group".into(),
            }],
            display_label: "Remote Group / Record".into(),
            search_text: "Remote Group / Record".into(),
        }]),
    });

    let search = app.libs[0].search.as_ref().unwrap();
    assert!(!search.loading);
    assert_eq!(search.query, "remote group");
    assert_eq!(search.results, vec![0]);
}

#[test]
fn failed_album_index_becomes_unavailable_and_clears_search_loading() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );
    assert!(app.open_recursive_album_search(0));

    app.handle_lib_event(LibEvent::AlbumIndexBuilt {
        library_id: "music-lib".into(),
        result: Err("index failed".into()),
    });

    assert!(matches!(
        app.album_indexes.get("music-lib"),
        Some(AlbumIndexState::Unavailable)
    ));
    assert!(!app.libs[0].search.as_ref().unwrap().loading);
    assert!(app.status.contains("index failed"));
}

#[test]
fn refresh_while_album_index_loads_coalesces_one_replacement() {
    let mut app = recursive_music_app();
    app.album_indexes.insert(
        "music-lib".into(),
        AlbumIndexState::Loading {
            rebuild_pending: false,
        },
    );

    app.start_album_index(0, true);
    app.start_album_index(0, true);

    assert!(matches!(
        app.album_indexes.get("music-lib"),
        Some(AlbumIndexState::Loading {
            rebuild_pending: true
        })
    ));
}

#[test]
fn power_recursive_activation_keeps_power_view_and_enters_inline_tracks() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = recursive_music_app();
    app.library_tab = 1;
    app.panel_focus = PanelFocus::Library;
    app.libs[0].nav_stack.push(BrowseLevel {
        parent_id: "group-a".into(),
        title: "Group A".into(),
        items: vec![folder("artist-a", "Artist A")],
        total_count: 1,
        cursor: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        scroll: 0,
        all_items: None,
        letter_filter: None,
    });
    let default_position = app.libs[0].library_position_snapshot();
    app.library_position_state
        .libraries
        .insert("music-lib".into(), default_position.clone());
    app.libs[0].search = Some(super::super::LibSearch {
        query: String::new(),
        items: Vec::new(),
        results: Vec::new(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });
    let level = BrowseLevel {
        parent_id: "artist-c".into(),
        title: "Artist C".into(),
        items: vec![album("album-1", "Record")],
        total_count: 1,
        cursor: 0,
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        scroll: 0,
        all_items: None,
        letter_filter: None,
    };

    app.handle_lib_event(LibEvent::RecursiveAlbumActivated {
        library_id: "music-lib".into(),
        nav_stack: vec![level],
    });

    assert!(app.libs[0].search.is_none());
    assert_eq!(app.libs[0].album_track_focus, Some(0));
    assert_eq!(app.libs[0].nav_stack.last().unwrap().parent_id, "artist-c");
    let position = app
        .library_position_state
        .libraries
        .get("music-lib")
        .unwrap();
    assert_eq!(
        position.levels.last().map(|level| level.parent_id.as_str()),
        Some("artist-c")
    );
}

// ── remote_seek_ticks: asymmetric clamp (rewind only) ───────────────────

#[test]
fn remote_seek_rewind_clamps_at_zero() {
    // 3s in, rewind 5s: would go negative, must clamp to 0.
    assert_eq!(App::remote_seek_ticks(3, -5.0), 0);
}

#[test]
fn remote_seek_rewind_does_not_clamp_when_unnecessary() {
    assert_eq!(App::remote_seek_ticks(20, -5.0), 15 * TICKS_PER_SECOND);
}

#[test]
fn remote_seek_forward_has_no_clamp() {
    // Fast-forward has no lower-bound clamp in the original code; a small
    // pos_s plus a large forward delta simply goes wherever the math
    // says, same as rewind's clamp being absent here.
    assert_eq!(App::remote_seek_ticks(3, 5.0), 8 * TICKS_PER_SECOND);
}

// ── execute_context_action(Play) on the queue tab (issue #134 follow-up) ─
// This used to be a third, independent copy of queue-cursor activation
// that had drifted from the keyboard `Enter`/mouse double-click paths
// (no seek-to-start for an already-playing audio item); it now shares
// `Command::QueuePlayCursor` with both of them.

#[test]
fn context_menu_play_on_queue_tab_seeks_to_start_for_current_playing_audio_item() {
    use crate::app::tests::make_item;

    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.player_tab
        .set_items(vec![make_item("Track One", "Audio")], 0);
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
    }
    let rx = app.player.spy_on_commands();

    app.execute_context_action(Some(ContextAction::Play));

    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::SeekAbsolute(pos)) if pos == 0.0
    ));
}

#[test]
fn power_view_enqueue_then_queue_play_cursor_syncs_and_jumps_to_new_item() {
    use crate::app::action::Command;
    use crate::app::tests::make_item;
    use crate::app::{BrowseLevel, LibraryTab, PanelFocus};
    use crate::player::PlayerCommand;

    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.library_tab = 1;
    app.player_tab
        .set_items(vec![make_item("Queued First", "Movie")], 0);
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
        st.queue_len = 1;
    }

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut queued = make_item("Queued Second", "Movie");
    queued.id = "movie-2".into();

    app.libs.push(LibraryTab {
        library,
        nav_stack: vec![BrowseLevel {
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![queued.clone()],
            total_count: 1,
            cursor: 0,
            scroll: 0,
            item_types: None,
            unplayed_only: false,
            sort_by: "SortName".into(),
            sort_order: "Ascending".into(),
            loading: false,
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

    assert_eq!(
        app.current_lib_item().as_ref().map(|i| i.id.as_str()),
        Some("movie-2")
    );

    let rx = app.player.spy_on_commands();
    app.execute_context_action(Some(crate::app::ContextAction::Enqueue));

    assert_eq!(app.player_tab.items.len(), 2);
    assert_eq!(app.player_tab.items[1].id, queued.id);
    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::QueueAppend { items }) if items.len() == 1 && items[0].id == queued.id
    ));

    app.panel_focus = PanelFocus::Queue;
    app.player_tab.queue_cursor = 1;

    app.dispatch(Command::QueuePlayCursor);

    assert!(matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))));
}

// ── next_subtitle_entry: shared cycling math (remote/local parity, #86) ─

#[test]
fn next_subtitle_entry_advances_from_off() {
    assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 0), 5);
}

#[test]
fn next_subtitle_entry_wraps_from_last_back_to_off() {
    assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 7), 0);
}

#[test]
fn next_subtitle_entry_unknown_current_restarts_at_first() {
    // A stale/unrecognized current selection (e.g. a track that
    // disappeared) is treated as if it were at position 0, matching the
    // pre-existing `.unwrap_or(0)` fallback in both the remote and local
    // branches -- so the *next* entry advances to position 1.
    assert_eq!(App::next_subtitle_entry(&[0, 5, 7], 99), 5);
}

#[test]
fn next_subtitle_entry_empty_returns_current_unchanged() {
    assert_eq!(App::next_subtitle_entry(&[], 3), 3);
}

#[test]
fn next_subtitle_entry_matches_remote_sentinel_convention() {
    // Remote sessions use -1 as the "off" sentinel (vs. 0 for local
    // playback) -- same wraparound math, different sentinel value.
    assert_eq!(App::next_subtitle_entry(&[-1, 2, 4], -1), 2);
    assert_eq!(App::next_subtitle_entry(&[-1, 2, 4], 4), -1);
}

// ── cycle_sub: local branch (#86 unification + idle fallback) ───────────

// `XDG_CONFIG_HOME`/`MBV_SYSTEM` are process-global env vars, so tests
// that touch them must not run concurrently with each other -- or with
// any other test in the crate that touches env vars.
// Reuse config.rs's `SYS_ENV_LOCK` rather than a second, independent
// mutex: two separate locks over the same global state don't exclude
// each other and previously caused flaky cross-test env-var races.
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

// ── is_audio_item / toggle_mute: remote-session awareness (#88) ─────────

fn make_remote_session(audio_only: bool) -> mbv_core::api::SessionInfo {
    mbv_core::api::SessionInfo {
        media_info: mbv_core::api::SessionMediaInfo {
            audio_only,
            ..Default::default()
        },
        ..crate::app::tests::make_session("device", "Emby")
    }
}

#[test]
fn is_audio_item_reads_remote_session_audio_only_flag_when_true() {
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_remote_session(true));

    assert!(
        app.is_audio_item(),
        "a connected session's audio_only flag should decide is_audio_item(), \
         not local playlist/cursor state"
    );
}

#[test]
fn is_audio_item_reads_remote_session_audio_only_flag_when_false() {
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_remote_session(false));

    assert!(!app.is_audio_item());
}

#[test]
fn is_audio_item_falls_back_to_local_state_when_no_session() {
    let mut app = crate::app::tests::make_app_stub();
    assert!(app.connected_session_id.is_none());
    app.player_tab.items = vec![crate::app::tests::make_item("song", "Audio")];
    app.player_tab.queue_cursor = 0;

    assert!(app.is_audio_item());
}

#[test]
fn toggle_mute_falls_back_to_cycle_audio_when_remote_session_connected() {
    // No session-level mute primitive exists (#88), so toggle_mute()
    // must hand off to cycle_audio()'s session-aware branch instead of
    // touching local ui_volume/pre_mute_volume state, which wouldn't
    // reflect a remote session's audio-only playback anyway.
    let mut app = crate::app::tests::make_app_stub();
    app.connected_session_id = Some("sess-1".into());
    app.connected_session_state = Some(make_remote_session(true));
    let ui_volume_before = app.ui_volume;

    app.toggle_mute();

    assert_eq!(
        app.ui_volume, ui_volume_before,
        "remote toggle_mute() must not touch local ui_volume state"
    );
    assert_eq!(
        app.connected_session_state.as_ref().unwrap().audio_index,
        2,
        "toggle_mute() should have delegated to cycle_audio()'s remote branch, \
         which advances the session's audio_index"
    );
}

// ── album_tracks_cache / LibEvent::AlbumTracksFetched (#145) ────────────
// Proactive track-list fetch/cache for the Power View inline album
// detail pane, mirroring the existing `album_artist_cache` pattern.

#[test]
fn album_tracks_fetched_event_populates_cache_and_clears_loading() {
    use crate::app::tests::make_item;

    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_loading.insert("album-1".into());

    let mut track = make_item("Opening Track", "Audio");
    track.id = "track-1".into();
    app.handle_lib_event(LibEvent::AlbumTracksFetched {
        album_id: "album-1".into(),
        tracks: vec![track],
    });

    assert!(
        !app.album_tracks_loading.contains("album-1"),
        "the loading marker must be cleared once the fetch resolves"
    );
    let cached = app
        .album_tracks_cache
        .get("album-1")
        .expect("fetched tracks must be cached under the album id");
    assert_eq!(cached.len(), 1);
    assert_eq!(cached[0].id, "track-1");
}

#[test]
fn fetch_album_tracks_is_a_no_op_when_already_cached() {
    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_cache.insert("album-1".into(), Vec::new());

    app.fetch_album_tracks("album-1".into());

    assert!(
        !app.album_tracks_loading.contains("album-1"),
        "a cache hit must return before marking the album as loading \
         (and before spawning a redundant network fetch)"
    );
}

#[test]
fn fetch_album_tracks_is_a_no_op_when_already_loading() {
    let mut app = crate::app::tests::make_app_stub();
    app.album_tracks_loading.insert("album-1".into());

    app.fetch_album_tracks("album-1".into());

    assert!(
        !app.album_tracks_cache.contains_key("album-1"),
        "a duplicate call while a fetch is already in flight must not \
         spawn a second fetch or fabricate a cache entry"
    );
}

// #286: this used to redirect the process-wide STDERR_FILENO fd to
// capture the bell byte, which raced against any other test ringing the
// bell concurrently on a different thread (flash_status/flash_status_high
// also ring it) and produced flaky doubled "\x07\x07" captures. Reading
// `TEST_BELL_LOG` (thread-local, cleared per test thread) instead avoids
// touching real stderr at all, so there's nothing left to race against.
#[test]
fn notify_with_actions_rings_terminal_bell_even_without_system_notifications() {
    TEST_BELL_LOG.with(|log| log.borrow_mut().clear());

    let app = crate::app::tests::make_app_stub();
    app.notify_with_actions("mbv", "Next up?", &[("next_up:play", "Play Now")]);

    let rung = TEST_BELL_LOG.with(|log| log.borrow().clone());
    assert_eq!(rung, b"\x07");
}

#[test]
fn enqueue_selected_rejects_item_from_a_different_route_than_active_queue() {
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

    app.enqueue_selected();

    // `PlayerTab`/`PlaybackQueue`/`MediaItem` implement neither
    // `PartialEq` nor `Debug` in this codebase (confirmed: `MediaItem`
    // derives only `Debug, Clone, Serialize, Deserialize`, and
    // `PlayerTab` derives only `Clone, Default`), so a whole-struct
    // `assert_eq!` against a captured "before" clone will not compile.
    // The established idiom elsewhere in this test module (e.g. the
    // rollback-path tests) is to assert on `.items` directly instead
    // -- here that's simplest as "still empty", since `make_app_stub`
    // starts with an empty queue and a rejected enqueue must leave it
    // that way.
    assert!(app
        .queue_for_scope(app.visible_queue_scope())
        .items
        .is_empty());
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

#[test]
fn enqueue_route_conflict_allows_matching_route() {
    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    assert!(!app.enqueue_route_conflict(Some("music".to_string())));
}

#[test]
fn enqueue_route_conflict_allows_local_queue_local_item() {
    let mut app = make_app_stub();
    assert!(!app.enqueue_route_conflict(None));
}

#[test]
fn enqueue_route_conflict_rejects_mismatched_route() {
    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    assert!(app.enqueue_route_conflict(Some("movies".to_string())));
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

#[test]
fn enqueue_route_conflict_allows_enqueue_while_attached_to_a_session() {
    // A Sessions-panel attached session (`connected_session_id`) has
    // its own, separate queue-scope rules -- the library-routing
    // invariant must not fire a "Can't mix libraries" toast for a
    // reason unrelated to library routing.
    let mut app = make_app_stub();
    app.connected_session_id = Some("sess-1".to_string());
    assert!(!app.enqueue_route_conflict(Some("music".to_string())));
}

#[test]
fn enqueue_route_conflict_allows_enqueue_while_on_a_non_route_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.player = mbv_core::player::PlayerProxy::remote(remote, false);
    app.player_rx = remote_rx;
    // active_route stays None: this is a Sessions-panel direct-remote
    // connection, not a library route.
    assert!(!app.enqueue_route_conflict(Some("music".to_string())));
}

#[test]
fn play_item_swaps_to_library_route_before_replacing_queue() {
    // #256: library-route resolution is now a pure config read -- no
    // live session lookup, no SESSIONS_LOAD_OVERRIDE seam needed here.
    // DAEMON_ROUTE_CONNECT_OVERRIDE is still needed: apply_route_for_playback
    // still performs a real connect to the resolved endpoint.
    let _guard = crate::config::TestStateDirGuard::new();
    let _connect_guard = crate::app::DAEMON_ROUTE_CONNECT_TEST_LOCK.lock().unwrap();
    fn route_connect_success(
        _endpoint: &mbv_core::remote_player::DaemonEndpoint,
        _auth_token: &str,
    ) -> Result<
        (
            mbv_core::remote_player::RemotePlayer,
            mpsc::Receiver<PlayerEvent>,
        ),
        String,
    > {
        Ok(mbv_core::remote_player::RemotePlayer::stub(
            make_items(1),
            0,
        ))
    }
    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = Some(route_connect_success);

    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "tcp://127.0.0.1:9000".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();
    app.library_tab = 1;

    app.play_item(item);

    *crate::app::DAEMON_ROUTE_CONNECT_OVERRIDE.lock().unwrap() = None;
    assert_eq!(app.active_route.as_deref(), Some("music"));
}

#[test]
fn play_item_skips_library_routing_when_attached_to_a_session() {
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());
    app.connected_session_id = Some("sess-1".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();

    // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
    // engaged here it would attempt a real connection and this test
    // would hang/fail rather than reach the assertion below.
    app.play_item(item);

    assert!(app.active_route.is_none());
}

#[test]
fn play_item_skips_library_routing_when_already_direct_remote_via_sessions_panel() {
    // Regression guard for the gap `connected_session_id.is_none()`
    // alone misses: a Sessions-panel "Direct Remote" ctrl-socket
    // upgrade leaves `connected_session_id` as `None` but
    // `self.player.is_remote()` `true` and `active_route` `None`.
    // Library routing must not engage here either -- it would swap
    // `self.player` out from under the active direct-remote
    // connection without ever clearing `direct_remote_label`.
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    let sess = crate::app::tests::make_session("other-mbv", "mbv");
    app.switch_to_direct_remote(&sess, remote, remote_rx);
    assert!(app.player.is_remote());
    assert!(app.active_route.is_none());

    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab {
        library: lib_item,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    });
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();

    // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
    // engaged here it would attempt a real connection and this test
    // would hang/fail rather than reach the assertion below.
    app.play_item(item);

    assert!(app.active_route.is_none());
}

fn lib_tab(collection_type: &str) -> LibraryTab {
    let mut library = make_item("Lib", "CollectionFolder");
    library.id = "lib-1".into();
    library.collection_type = collection_type.into();
    LibraryTab {
        library,
        nav_stack: Vec::new(),
        search: None,
        feed_home_video: None,
        album_track_focus: None,
        artist_header_focus: None,
        series_selection: None,
        series_season_cursor: 0,
        library_total: None,
    }
}

#[test]
fn active_lib_is_tvshows_true_only_on_a_tvshows_library_tab() {
    // `shuffle_folder` (issue: TV libraries should shuffle from a
    // video-only fetch, everything else from the broader playable-items
    // fetch) branches on this. Covers both view modes indirectly, since
    // `lib_tab_offset()` is view-mode-independent -- the tab_idx math is
    // identical whether the active library tab was reached through the
    // standard tab bar or Power View's left panel.
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));
    app.libs.push(lib_tab("music"));

    app.library_tab = 1;
    assert!(
        app.active_lib_is_tvshows(),
        "library_tab on the tvshows library tab"
    );

    app.library_tab = 2;
    assert!(
        !app.active_lib_is_tvshows(),
        "library_tab on the music library tab"
    );
}

#[test]
fn active_lib_is_tvshows_false_outside_any_library_tab() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("tvshows"));

    app.library_tab = 0; // Home
    assert!(!app.active_lib_is_tvshows());

    app.panel_focus = PanelFocus::Queue;
    assert!(!app.active_lib_is_tvshows());
}

/// Pushes a top-level, non-loading, non-searching `BrowseLevel` onto
/// `lib`'s nav_stack -- the minimum state `should_show_letter_pills`
/// needs to consider the library "at its top browse level".
fn push_top_level(lib: &mut LibraryTab, item_count: usize) {
    lib.nav_stack.push(BrowseLevel {
        parent_id: lib.library.id.clone(),
        title: lib.library.name.clone(),
        items: make_items(item_count),
        total_count: item_count,
        cursor: 0,
        scroll: 0,
        item_types: Some("Movie".into()),
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
    });
}

#[test]
fn should_show_letter_pills_requires_library_total_over_threshold() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);

    // No captured library_total yet -> hidden even if the fetched-so-far
    // count is small.
    assert!(!app.should_show_letter_pills(0));

    app.libs[0].library_total = Some(300);
    assert!(
        !app.should_show_letter_pills(0),
        "300 is the threshold, not over it"
    );

    app.libs[0].library_total = Some(301);
    assert!(app.should_show_letter_pills(0));
}

#[test]
fn should_show_letter_pills_excludes_music_search_and_drilldowns() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("music"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    assert!(
        !app.should_show_letter_pills(0),
        "music libraries use group pills instead"
    );

    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[1], 10);
    app.libs[1].library_total = Some(1000);
    assert!(app.should_show_letter_pills(1));

    app.libs[1].search = Some(crate::app::LibSearch {
        query: String::new(),
        items: Vec::new(),
        results: Vec::new(),
        cursor: 0,
        scroll: 0,
        loading: false,
    });
    assert!(!app.should_show_letter_pills(1), "hidden while searching");
    app.libs[1].search = None;

    // A second nav level (drilled into a folder) is no longer the "top"
    // browse level.
    push_top_level(&mut app.libs[1], 5);
    assert!(
        !app.should_show_letter_pills(1),
        "hidden below the top browse level"
    );
}

#[test]
fn select_letter_pill_scopes_the_level_and_resets_cursor() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);
    app.libs[0].nav_stack[0].cursor = 4;
    app.libs[0].nav_stack[0].scroll = 2;

    app.select_letter_pill(0, 4); // "M–O"

    let lvl = app.libs[0].nav_stack.last().unwrap();
    let filter = lvl.letter_filter.as_ref().expect("pill should be set");
    assert_eq!(filter.index, 4);
    assert_eq!(filter.label, "M\u{2013}O");
    assert_eq!(filter.name_ge, Some("M"));
    assert_eq!(filter.name_lt, Some("P"));
    assert_eq!(lvl.cursor, 0);
    assert_eq!(lvl.scroll, 0);
    assert!(lvl.loading, "a scoped refresh should be in flight");
}

#[test]
fn select_letter_pill_is_a_noop_outside_letter_pill_eligibility() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    // library_total never captured -> should_show_letter_pills is false.

    app.select_letter_pill(0, 0);

    assert!(app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .is_none());
}

#[test]
fn cycle_letter_pill_wraps_around() {
    let mut app = make_app_stub();
    app.libs.push(lib_tab("movies"));
    push_top_level(&mut app.libs[0], 10);
    app.libs[0].library_total = Some(1000);

    // Default (no pill selected yet) is treated as index 0; cycling back
    // wraps to the last bucket ("#").
    app.cycle_letter_pill(0, -1);
    let filter = app.libs[0]
        .nav_stack
        .last()
        .unwrap()
        .letter_filter
        .as_ref()
        .unwrap();
    assert_eq!(filter.label, "#");
}

// Regression coverage for the bug found in review of the letter-pills
// PR: `spawn_all_items_prefetch`/`spawn_search_items_load` used to cap
// their unfiltered fetch's `limit` at `lvl.total_count`, which is the
// FILTERED range's count whenever a letter pill is active (e.g. ~40 for
// an `M–O` pill out of a 3,000-movie library) -- so `all_items` (the set
// `/`-search runs over) silently shrank to just the active range, and
// whole-library search missed everything outside it.
#[test]
fn full_library_fetch_limit_uses_true_total_not_the_filtered_range_count() {
    let mut lib = lib_tab("movies");
    push_top_level(&mut lib, 40); // the "M–O" slice: 40 items
    lib.library_total = Some(3000); // the library's true size
    {
        let lvl = lib.nav_stack.last_mut().unwrap();
        lvl.total_count = 40; // what get_items_sorted_ranged reported for M–O
        lvl.letter_filter = crate::app::render::LetterFilter::for_index(4);
    }
    let lvl = lib.nav_stack.last().unwrap();

    assert_eq!(
        full_library_fetch_limit(&lib, lvl),
        3000,
        "must fetch the whole library, not just the active M–O range"
    );
}

#[test]
fn full_library_fetch_limit_falls_back_to_total_count_before_library_total_is_known() {
    let mut lib = lib_tab("movies");
    push_top_level(&mut lib, 10);
    // library_total not yet captured (e.g. first-ever load in flight).
    let lvl = lib.nav_stack.last().unwrap();
    assert_eq!(full_library_fetch_limit(&lib, lvl), 10);
}
