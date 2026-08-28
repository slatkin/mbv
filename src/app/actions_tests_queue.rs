#![allow(dead_code, unused_imports)]

use super::*;
use crate::app::library_browse_actions::{
    build_album_index_with, full_library_fetch_limit, recursive_album_search_eligible,
};
use crate::app::tests::{make_app_stub, make_item, make_items};
use crate::app::{
    AlbumIndexState, AlbumPathPart, AlbumSearchEntry, BrowseLevel, ContextAction,
    FeedHomeVideoState, LibEvent, LibraryTab, QueueScope, TabSelection,
};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::player::PlayerEvent;
use std::collections::HashMap;
use std::sync::mpsc;

fn folder(id: &str, name: &str) -> EmbyItem {
    let mut item = make_item(name, "Folder");
    item.id = id.into();
    item.is_folder = true;
    item
}

fn album(id: &str, name: &str) -> EmbyItem {
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
    app.libs.push(LibraryTab::new(library));
    app
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

    app.execute_context_action(Some(ContextAction::Play), None);

    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::SeekAbsolute(pos)) if pos == 0.0
    ));
}

#[test]
fn queue_menu_play_carries_clicked_index_not_follow_cursor() {
    use crate::app::action::Command;
    use crate::app::tests::make_item;
    use crate::app::types_overlay::OverlayRequest;
    use crate::player::PlayerCommand;

    // The queue menu's Play action must retain the index resolved when the
    // menu opened (the right-clicked slot) rather than re-reading
    // `queue_cursor` at execution (split-queue-cursor-ownership D2): a
    // follow update while the menu is open must not redirect playback to
    // another row.
    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.player_tab
        .set_items(vec![make_item("A", "Movie"), make_item("B", "Movie")], 0);
    let slot_b = app.player_tab.queue.slots()[1].slot_id;

    // Right-click row B (index 1): the click resolves slot -> index 1 and
    // opens the menu; the menu's Play action closes over 1.
    app.handle_mouse_right_click_queue(Some(slot_b), 0, 0, false);
    let menu = match app.pending_overlay.as_ref() {
        Some(OverlayRequest::ContextMenu(menu)) => menu,
        _ => panic!("queue context menu must open on right-click"),
    };
    let play_index = menu
        .entries
        .iter()
        .find_map(|entry| match entry.action.as_ref() {
            Some(crate::app::ContextAction::PlayQueue(pos)) => Some(*pos),
            _ => None,
        })
        .expect("queue menu must carry a PlayQueue action");
    assert_eq!(
        play_index, 1,
        "menu Play must close over the clicked index 1, got {play_index}"
    );

    // Follow update while the menu is open: playback advances the follow
    // cursor to 0 (row A). Executing the retained PlayQueue(1) must still
    // play row B.
    app.player_tab.queue_cursor = 0;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
        st.queue_len = 2;
    }
    let rx = app.player.spy_on_commands();
    app.execute_context_action(Some(crate::app::ContextAction::PlayQueue(play_index)), None);

    assert!(
        matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))),
        "menu Play must jump to the retained clicked index 1, not follow cursor 0"
    );
}

#[test]
fn queue_double_click_plays_clicked_index_not_follow_cursor() {
    use crate::app::tests::make_item;
    use crate::player::PlayerCommand;

    // The queue-row double-click must play the index resolved from the
    // clicked slot, passed straight through (D2), not recovered from
    // `queue_cursor`: a follow cursor pointing elsewhere must not redirect.
    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = crate::app::PanelFocus::Queue;
    app.player_tab
        .set_items(vec![make_item("A", "Movie"), make_item("B", "Movie")], 0);
    // Follow cursor points at row A (index 0).
    app.player_tab.queue_cursor = 0;
    let slot_b = app.player_tab.queue.slots()[1].slot_id;
    {
        let mut st = app.player.status.lock().unwrap();
        st.active = true;
        st.current_idx = 0;
        st.queue_len = 2;
    }
    let rx = app.player.spy_on_commands();

    // Double-click row B (index 1).
    app.handle_mouse_double_click_queue(Some(slot_b));

    assert!(
        matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo(1))),
        "double-click must jump to the clicked index 1, not follow cursor 0"
    );
}

#[test]
fn enqueue_then_queue_play_cursor_syncs_and_jumps_to_new_item() {
    use crate::app::action::Command;
    use crate::app::tests::make_item;
    use crate::app::{BrowseLevel, LibraryTab, PanelFocus, TabSelection};
    use crate::player::PlayerCommand;

    let mut app = crate::app::tests::make_app_stub();
    app.panel_focus = PanelFocus::Library;
    app.tab = TabSelection::EmbyLibrary(0);
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
            music_grouping: None,
        }],
        ..LibraryTab::new(library)
    });

    assert_eq!(
        app.current_lib_item(0).as_ref().map(|i| i.id.as_str()),
        Some("movie-2")
    );

    let rx = app.player.spy_on_commands();
    app.execute_context_action(Some(crate::app::ContextAction::Enqueue), None);

    assert_eq!(app.player_tab.emby_items().len(), 2);
    assert_eq!(app.player_tab.emby_items()[1].id, queued.id);
    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::QueueAppend { items }) if items.len() == 1 && items[0].id() == queued.id
    ));

    app.panel_focus = PanelFocus::Queue;
    app.player_tab.queue_cursor = 1;

    app.dispatch(Command::QueuePlayCursor(1));

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
