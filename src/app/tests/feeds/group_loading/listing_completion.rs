//! The pill row's completion over a refreshed root listing: a partial root
//! refresh, the full refresh that invalidates the folder counts, the count
//! fan-out the listing completion commits, and the UI-state reset's root
//! restoration. Split from `group_loading` to keep its contract tests beside
//! this file's own.

use super::make_home_video_app;
use super::make_root_folder;
use super::seed_folder_counts;
use super::seed_home_video_root_loaded;
use super::youtube_root_level;
use crate::app::tests::*;
use mbv_ui_model::browse::ServerRows;
use rstest::rstest;

/// Regression (review P2, b603f329b..9e454b62d): a refresh that lands only
/// the first page of a multi-page root listing must not clamp or re-resolve
/// the pill index against the partial listing while `selection` keeps a
/// later-page folder. The final page append completes the refresh: the pill
/// row re-derives from the full listing and the selection stays the folder
/// the user picked. (Count preservation across the partial refresh has its
/// own owner in
/// `a_partial_refresh_commits_a_count_fanout_without_shrinking_the_counts`.)
#[rstest]
fn partial_root_refresh_keeps_counts_and_the_later_page_pill(make_home_video_app: App) {
    let mut app = make_home_video_app;
    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::Loaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            level: Box::new(youtube_root_level(
                ServerRows::loaded(3, 3),
                vec![
                    make_root_folder("folder-a", "Channel A"),
                    make_root_folder("folder-b", "Channel B"),
                    make_root_folder("folder-c", "Channel C"),
                ],
                false,
            )),
        }),
        None,
    );
    for id in ["folder-a", "folder-b", "folder-c"] {
        seed_folder_counts(&mut app, id, 1);
    }
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.selection = FeedHomeVideoSelection::Folder("folder-c".into());
    }

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::Refreshed {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            items: vec![make_root_folder("folder-a", "Channel A")],
            total_count: 3,
        }),
        None,
    );

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder("folder-c".into()),
        "the selection outlives the partial listing"
    );

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::PageAppended {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            items: vec![
                make_root_folder("folder-b", "Channel B"),
                make_root_folder("folder-c", "Channel C"),
            ],
            total_count: 3,
        }),
        None,
    );

    assert_eq!(
        app.feed_home_video_counted_folders(0)
            .iter()
            .map(|folder| folder.id.as_str())
            .collect::<Vec<_>>(),
        vec!["folder-a", "folder-b", "folder-c"],
        "the repaginated listing restores the pill row"
    );
    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        app.feed_home_video_selected_group_index(0),
        3,
        "the repaginated row highlights the selected folder again"
    );
    assert_eq!(
        state.selection,
        FeedHomeVideoSelection::Folder("folder-c".into())
    );
}

/// Contract (design D2 refresh; replaces the deleted `pending_count_refresh`
/// marker, review P2 9e454b62d..8e5ce710a): a fully paginated refresh
/// invalidates the per-folder counts, so the next listing completion's
/// non-forced fan-out fails its dedupe and re-queries every folder — a
/// refresh must not leave the pre-refresh counts serving the pill row.
#[rstest]
fn a_full_refresh_invalidates_the_folder_counts_so_the_fan_out_requeries(make_home_video_app: App) {
    let mut app = make_home_video_app;
    let active = seed_home_video_root_loaded(&mut app);
    seed_folder_counts(&mut app, &active.id, 2);

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::Refreshed {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            items: vec![active],
            total_count: 1,
        }),
        None,
    );

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(
        state.folder_counts.is_empty(),
        "the refresh cleared the counts so the fan-out re-queries"
    );
}

/// Regression (review P2 on 96abfa239..ab4b558bb): `spawn_refresh` fetches
/// only `loaded_count.max(PAGE_SIZE)` rows, so a refresh of a root listing
/// that has not fully paginated lands partial — and that refresh must still
/// re-query the counts (design D2), or a folder crossing 0<->positive
/// videos never gains or loses its pill for the rest of the session. The
/// partial refresh commits the fan-out synchronously (the quiescence
/// accounting mark) while leaving the existing counts in place: page-1
/// folders must not shrink the row (review P2, b603f329b..9e454b62d).
#[rstest]
fn a_partial_refresh_commits_a_count_fanout_without_shrinking_the_counts(make_home_video_app: App) {
    let mut app = make_home_video_app;
    // The stub client's count queries fail into the unprocessed event
    // channel, so the synchronous accounting mark stays observable.
    install_test_emby(&mut app, crate::app::config::Config::default());
    app.libs[0].nav_stack[0] = youtube_root_level(
        ServerRows::loaded(3, 3),
        vec![
            make_root_folder("folder-a", "Channel A"),
            make_root_folder("folder-b", "Channel B"),
            make_root_folder("folder-c", "Channel C"),
        ],
        false,
    );
    seed_folder_counts(&mut app, "folder-a", 1);
    seed_folder_counts(&mut app, "folder-b", 1);
    seed_folder_counts(&mut app, "folder-c", 1);

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::Refreshed {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            items: vec![make_root_folder("folder-a", "Channel A")],
            total_count: 3,
        }),
        None,
    );

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.folder_counts.len(),
        3,
        "page-1 folders did not shrink the count map"
    );
    assert_eq!(
        state.outstanding_counts, 1,
        "the partial refresh committed a fan-out for the listing's folders"
    );
}

/// Regression (round-2 review on 96abfa239..30a12b0e8): a partial refresh
/// whose folder set is UNCHANGED leaves the beyond-page-1 counts stale unless
/// the listing completion that ends the pagination re-queries them — the map
/// already covers every folder, so a coverage-only spawn condition
/// short-circuits and a folder beyond page 1 that crossed 0<->positive
/// videos keeps its stale pill for the session (design D2). The
/// listing-completion transition therefore spawns the full fan-out even
/// when the counts already cover the root. (Count preservation across the
/// partial refresh has its own owner in
/// `a_partial_refresh_commits_a_count_fanout_without_shrinking_the_counts`.)
#[rstest]
fn the_listing_completion_after_a_partial_refresh_requeries_every_folder(make_home_video_app: App) {
    let mut app = make_home_video_app;
    // The stub client's count queries fail into the unprocessed event
    // channel, so the synchronous accounting mark stays observable.
    install_test_emby(&mut app, crate::app::config::Config::default());
    app.libs[0].nav_stack[0] = youtube_root_level(
        ServerRows::loaded(3, 3),
        vec![
            make_root_folder("folder-a", "Channel A"),
            make_root_folder("folder-b", "Channel B"),
            make_root_folder("folder-c", "Channel C"),
        ],
        false,
    );
    seed_folder_counts(&mut app, "folder-a", 1);
    seed_folder_counts(&mut app, "folder-b", 1);
    seed_folder_counts(&mut app, "folder-c", 1);

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::Refreshed {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            items: vec![make_root_folder("folder-a", "Channel A")],
            total_count: 3,
        }),
        None,
    );

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::PageAppended {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            items: vec![
                make_root_folder("folder-b", "Channel B"),
                make_root_folder("folder-c", "Channel C"),
            ],
            total_count: 3,
        }),
        None,
    );

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert_eq!(
        state.outstanding_counts, 4,
        "the completion re-queried every folder despite full coverage \
         (1 refresh fan-out + 3 completion fan-out)"
    );
}

/// Regression (round-3 review on b603f329b..HEAD): the listing completion
/// that ends a partial refresh's pagination must not clear the selection's
/// `loading` flag while that selection's fetch is still in flight — the
/// shown pass spawns the fetch between the refresh and the final page
/// append, and the flag belongs to that fetch until its result lands (spec
/// `feed-home-video-group-view`, "An unloaded selection SHALL show its
/// loading state until its fetch returns"). Clearing it early hid the
/// spinner mid-fetch and let the shown pass double-spawn the fetch.
#[rstest]
fn listing_completion_keeps_loading_while_the_selection_fetch_is_in_flight(
    make_home_video_app: App,
) {
    let mut app = make_home_video_app;
    app.libs[0].nav_stack[0] = youtube_root_level(
        ServerRows::loaded(3, 3),
        vec![make_root_folder("folder-a", "Channel A")],
        false,
    );
    if let Some(state) = app.libs[0].feed_home_video.as_mut() {
        state.loading = true;
        state.fetching = Some(FeedHomeVideoSelection::All);
    }

    app.maybe_complete_feed_home_video_listing(0);

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(
        state.loading,
        "the in-flight selection fetch owns the loading flag"
    );

    app.handle_lib_event(
        LibEvent::Browse(BrowseEvent::FeedHomeVideoSelectionLoaded {
            lib_idx: 0,
            parent_id: "lib-youtube".into(),
            selection: FeedHomeVideoSelection::All,
            items: vec![make_item("Episode 1", "Movie")],
        }),
        None,
    );

    let state = app.libs[0].feed_home_video.as_ref().unwrap();
    assert!(!state.loading, "the fetch's own result clears the flag");
}

/// Contract (review P2 on task 5.1, commit 46b0ac739): the UI-state reset
/// restores a feed-view library's root to a plain folder listing — no
/// `item_types`, no unplayed-only flag — so a later refresh builds its fetch
/// from folder-listing fields instead of fetching videos onto the root and
/// destroying the pill source.
#[rstest]
fn ui_state_reset_restores_the_feed_view_root_to_a_folder_listing(make_home_video_app: App) {
    let mut app = make_home_video_app;
    seed_home_video_root_loaded(&mut app);
    if let Some(root) = app.libs[0].nav_stack.first_mut() {
        root.item_types = Some("Video".into());
        root.unplayed_only = true;
        root.sort_by = "DateCreated".into();
        root.sort_order = "Ascending".into();
    }

    app.reset_browse_root_defaults();

    let root = &app.libs[0].nav_stack[0];
    assert_eq!(root.item_types, None, "the reset drops the Video filter");
    assert!(
        !root.unplayed_only,
        "the reset drops the unplayed-only flag"
    );
    assert_eq!(root.sort_by, "SortName");
    assert_eq!(root.sort_order, "Ascending");
    assert_eq!(root.items.len(), 2, "the root keeps its folder items");
}
