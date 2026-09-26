use crate::app::state::types::browse::BrowseResting;

use crate::app::App;
use rstest::rstest;

// ── remote_seek_ticks: asymmetric clamp (rewind only) ───────────────────

#[rstest]
#[case::remote_seek_rewind_clamps_at_zero(3, -5.0, 0)]
fn remote_seek(#[case] position: i64, #[case] delta: f64, #[case] expected: i64) {
    assert_eq!(App::remote_seek_ticks(position, delta), expected);
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
    };
    let rx = app.player.spy_on_commands();

    // Double-click row B (index 1).
    app.handle_mouse_double_click_queue(Some(slot_b));

    assert!(
        matches!(rx.try_recv(), Ok(PlayerCommand::JumpTo { slot_id, .. }) if slot_id == slot_b),
        "double-click must jump to the clicked row B, not follow cursor 0"
    );
}

#[test]
fn enqueue_then_queue_play_cursor_syncs_and_jumps_to_new_item() {
    use crate::app::dispatch::action::Command;
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
    };

    let mut library = make_item("Movies", "CollectionFolder");
    library.id = "lib-movies".into();
    library.is_folder = true;
    library.collection_type = "movies".into();

    let mut queued = make_item("Queued Second", "Movie");
    queued.id = "movie-2".into();

    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            fetched_rows: 0,
            parent_id: "lib-movies".into(),
            title: "Movies".into(),
            items: vec![queued.clone()],
            total_count: 1,
            resting: BrowseResting::new(0, 0),
            item_types: None,
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

    assert_eq!(
        app.current_lib_item(0, 0).as_ref().map(|i| i.id.as_str()),
        Some("movie-2")
    );

    let rx = app.player.spy_on_commands();
    app.execute_context_action(Some(crate::app::ContextAction::Enqueue), None);

    assert_eq!(app.player_tab.emby_items().len(), 2);
    assert_eq!(app.player_tab.emby_items()[1].id, queued.id);
    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::QueueAppend { items }) if items.len() == 1 && items[0].item.id() == queued.id
    ));

    app.panel_focus = PanelFocus::Queue;
    app.player_tab.queue_cursor = 1;
    let want_slot = app.player_tab.queue.slots()[1].slot_id;

    app.dispatch(&Command::QueuePlayCursor(1));

    assert!(matches!(
        rx.try_recv(),
        Ok(PlayerCommand::JumpTo { slot_id, .. }) if slot_id == want_slot
    ));
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
