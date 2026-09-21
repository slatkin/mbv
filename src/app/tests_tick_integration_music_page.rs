//! Grouped Music source pagination from the painted viewport edge (design
//! D3): scrolling the tree arms the next artist page without a selection
//! move, and the near-edge margin floors at the painted viewport height so
//! the loaded edge arms before it can scroll into view.

use super::*;

use super::landing::{
    draw_music_frame, mounted_music_app_at, music_panel_mut,
};

fn paginated_album_app(loaded: usize, total: usize) -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    let level = app.libs[0].nav_stack.last_mut().expect("album level");
    level.items = (1..=loaded)
        .map(|number| {
            let mut album = crate::app::tests::make_item(&format!("Album {number}"), "MusicAlbum");
            album.id = format!("album-{number}");
            album.artist = "Alpha".into();
            album.production_year = 2001;
            album
        })
        .collect();
    level.fetched_rows = loaded;
    level.total_count = total;
    app
}

#[test]
fn painted_edge_scrolling_ships_the_pagination_hint_without_a_selection_move() {
    let (mut harness, _id) =
        mounted_music_app_at(paginated_album_app(200, 400), 100, 24);
    // The production loop drains the panel's post-paint message before the
    // next frame; clear the adopt-frame payload so this test observes its own.
    let (mut music_resize, mut tv_resize) = (false, false);
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    assert!(
        !harness.model().app.libs[0].nav_stack.last().unwrap().loading,
        "the top-of-list frame arms no page: the loaded edge is far away"
    );

    {
        let owner = harness.model_mut().test_music_owner_mut();
        owner.browser.expand_all_roots();
    }
    // Draw once so the expansion change rebuilds the projection (the crate
    // re-anchors that rebuild to the selection), then scroll the viewport to
    // the bottom through the crate's offset seam (the wheel's local path),
    // leaving the selection on the first album.
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    let (mut music_resize, mut tv_resize) = (false, false);
    harness
        .model_mut()
        .drain_deferred_library_message(&mut music_resize, &mut tv_resize);
    {
        let owner = harness.model_mut().test_music_owner_mut();
        owner.browser.scroll_to(usize::MAX);
    }
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);

    let page = match music_panel_mut(&mut harness).take_deferred_msg() {
        Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch {
            page: Some(page),
            ..
        })) => page,
        other => panic!("expected the painted-edge page hint, got {other:?}"),
    };
    assert_eq!(page.0, "album-200", "the hint is the painted viewport edge");
    assert!(page.1 > 0, "the hint carries the painted viewport height");
    assert_eq!(
        harness.model().test_music_owner().browser.selected_album_target(),
        Some("album-1"),
        "scrolling never moved the selection"
    );

    // The same typed hint arms the next artist page through the shell arm.
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::MusicNeighbourPrefetch {
            targets: Vec::new(),
            page: Some(page),
        }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(
        harness.model().app.libs[0].nav_stack.last().unwrap().loading,
        "the painted-edge hint armed the next artist page"
    );
}

#[test]
fn painted_edge_margin_floors_at_the_viewport_height() {
    let mut app = paginated_album_app(30, 100);

    // Selection proximity alone (`PREFETCH_AHEAD` = 25) does not arm from
    // row 5 with 30 rows loaded (5 + 25 < 30); the painted 20-row viewport
    // floors the margin at 45, so the loaded edge arms before it can ever
    // scroll into view.
    app.maybe_fetch_next_page_for_music_tree(0, &("album-5".to_string(), 20));
    assert!(
        app.libs[0].nav_stack.last().unwrap().loading,
        "the viewport-floored margin armed the next page from the painted edge"
    );
}
