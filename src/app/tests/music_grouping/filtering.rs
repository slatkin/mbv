use super::*;

#[rstest]
#[case::empty_folder((true, Some(0u32), false))]
#[case::unknown_count((true, None, true))]
#[case::non_folder((false, Some(0u32), true))]
fn grouped_music_filter_cases(#[case] fixture: (bool, Option<u32>, bool)) {
    let (is_folder, child_count, kept) = fixture;
    let mut item = make_item("candidate", "Folder");
    item.is_folder = is_folder;
    item.child_count = child_count;
    let mut items = vec![item];
    retain_grouped_music_items(&mut items, true);
    assert_eq!(items.is_empty(), !kept);
}

#[test]
fn non_grouped_music_filter_keeps_empty_folder() {
    let mut item = make_item("candidate", "Folder");
    item.is_folder = true;
    item.child_count = Some(0);
    let mut items = vec![item];
    retain_grouped_music_items(&mut items, false);
    assert_eq!(items.len(), 1);
}

#[test]
fn grouped_refresh_filters_at_event_boundary_and_keeps_server_row_count() {
    let mut app = make_music_app(Vec::new());
    let mut empty = make_group_item("empty", "Empty");
    empty.child_count = Some(0);
    let kept = make_group_item("kept", "Kept");

    app.handle_lib_event(LibEvent::Refreshed {
        lib_idx: 0,
        parent_id: "group-0".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![empty, kept],
        total_count: 2,
    });

    let level = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(
        level
            .items
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["kept"]
    );
    assert_eq!(level.fetched_rows, 2);
    assert_eq!(level.total_count, 2);
}

#[test]
fn grouped_refresh_clamps_cursor_after_empty_folder_filter() {
    let mut app = make_music_app(Vec::new());
    app.libs[0].nav_stack.last_mut().unwrap().resting = BrowseResting::new(1, 1);
    let mut empty = make_group_item("empty", "Empty");
    empty.child_count = Some(0);
    let kept = make_group_item("kept", "Kept");

    app.handle_lib_event(LibEvent::Refreshed {
        lib_idx: 0,
        parent_id: "group-0".into(),
        item_types: None,
        unplayed_only: false,
        items: vec![empty, kept],
        total_count: 2,
    });

    let level = app.libs[0].nav_stack.last().unwrap();
    assert_eq!(level.resting().cursor(), 0);
    let position = level.to_position_level();
    assert_eq!(position.cursor_index, 0);
    assert_eq!(position.focused_item_id.as_deref(), Some("kept"));
    assert_eq!(position.fetched_rows, Some(2));
}

#[test]
fn grouped_chain_landing_filters_every_level_and_preserves_server_rows() {
    let mut app = make_music_app(Vec::new());
    let mut root = make_group_level();
    root.items.clear();
    let mut empty_root = make_group_item("empty-root", "Empty root");
    empty_root.child_count = Some(0);
    let kept_root = make_group_item("kept-root", "Kept root");
    root.items.extend([empty_root, kept_root]);
    root.total_count = root.items.len();

    let mut child = make_music_album_level(Vec::new());
    let mut empty_child = make_group_item("empty-child", "Empty child");
    empty_child.child_count = Some(0);
    let kept_child = make_group_item("kept-child", "Kept child");
    child.items = vec![empty_child, kept_child];
    child.total_count = child.items.len();
    child.fetched_rows = child.items.len();

    app.handle_lib_event(LibEvent::NavigateTo {
        lib_idx: 0,
        landing: crate::app::state::types::events::NavigateLanding::Chain {
            nav_stack: vec![root, child],
        },
        switch_tab: false,
    });

    assert_eq!(app.libs[0].nav_stack.len(), 2);
    for (level, (expected_id, expected_fetched_rows)) in app.libs[0]
        .nav_stack
        .iter()
        .zip([("kept-root", 2), ("kept-child", 2)])
    {
        assert_eq!(
            level
                .items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            [expected_id]
        );
        assert_eq!(level.fetched_rows, expected_fetched_rows);
        assert_eq!(
            level.to_position_level().fetched_rows,
            Some(expected_fetched_rows)
        );
    }
}
#[test]
fn focused_artist_arms_next_page_regardless_of_child_proximity() {
    // The grouped album level paginates to completion unconditionally
    // (client-side artist grouping needs the whole folder, and tree scroll
    // position doesn't correspond to flat-array position once artists
    // collapse/expand), so even a target near the top of the loaded items
    // arms the next page.
    let mut app = make_music_app(make_items(30));
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    level.fetched_rows = 30;
    level.total_count = 100;

    app.maybe_fetch_next_page_for_music_artist(0, &["id4".into()]);
    assert!(app.libs[0].nav_stack.last().unwrap().loading);
}

#[test]
fn focused_artist_with_no_loaded_child_does_not_arm_a_page() {
    let mut app = make_music_app(make_items(30));
    let level = app.libs[0].nav_stack.last_mut().unwrap();
    level.fetched_rows = 30;
    level.total_count = 100;

    app.maybe_fetch_next_page_for_music_artist(0, &["not-loaded".into()]);
    assert!(!app.libs[0].nav_stack.last().unwrap().loading);
}
