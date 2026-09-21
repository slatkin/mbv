//! Grouped Music source pagination from the painted viewport edge (design
//! D3): the post-paint hint is the deepest painted album target — deeper
//! than the selection — and the near-edge margin floors at the painted
//! viewport height, so the loaded edge arms before it can scroll into view.

use super::*;

use super::landing::{draw_music_frame, mounted_music_app_at, music_panel_mut};

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
fn painted_edge_hint_is_deeper_than_the_selection_and_arms_the_page() {
    let (mut harness, _id) = mounted_music_app_at(paginated_album_app(200, 400), 100, 24);
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

    // The user has scrolled near the loaded edge: the selection rests on a
    // deep album and the viewport's painted rows reach past it.
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .expand_all_roots();
    assert!(
        harness
            .model_mut()
            .test_music_owner_mut()
            .browser
            .select_album_target("album-190"),
        "the fixture interns album-190"
    );
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);

    let page = match music_panel_mut(&mut harness).take_deferred_msg() {
        Some(Msg::Shell(ShellRequest::MusicNeighbourPrefetch {
            page: Some(page),
            ..
        })) => page,
        other => panic!("expected the painted-edge page hint, got {other:?}"),
    };
    assert!(
        page.0.starts_with("album-"),
        "the hint is a painted album target, got {:?}",
        page.0
    );
    assert!(page.1 > 0, "the hint carries the painted viewport height");
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_album_target(),
        Some("album-190"),
        "the hint never moves the selection"
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
