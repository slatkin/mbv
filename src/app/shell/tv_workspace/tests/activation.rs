use super::*;
use mbv_emby_model::test_support::make_item;

#[test]
fn activate_selected_series_resolves_mirrored_cursor_and_guards_series() {
    let mut model = mounted_tv_model();
    model.terminal_width = 80;
    model.terminal_height = 24;
    // The narrow TV layout needs the queue column: with the empty-queue
    // column hidden (change `hide-empty-queue-column`, D1) an empty
    // displayed queue would widen the library into the wide breakpoint.
    model
        .app
        .seed_local_queue_for_test(vec![make_item("Queue Item", "Movie")], 0);
    model.sync_tv_content();
    model.sync_active_destination();

    // Divergence: the panel-hosted TvContent owner selects index 0 while
    // App's mirrored BrowseLevel cursor is stale at index 1. Resolve narrow
    // extras using the component-owned cursor, as the production seam does.
    model.app.set_library_level_cursor_for_test(0, 0, 1);
    let component_cursor = model.test_tv_owner().browse_cursor();
    assert_eq!(component_cursor, 0);
    assert_eq!(model.app.emby_library_level_cursor(0, 0), Some(1));

    assert_eq!(
        model.test_tv_owner().selected_item_id(),
        Some("movie-focused".into()),
        "the shared TV owner resolves its own selected Series"
    );

    model
        .app
        .set_library_level_cursor_for_test(0, 0, component_cursor);

    // Wide TV layout => enter_series_selection targets the component's
    // Series (asserted by the resolved target, not merely the bool).
    let wide_target = model
        .app
        .selected_series_item(0, model.app.emby_library_level_cursor(0, 0).unwrap_or(0))
        .expect("series");
    assert_eq!(wide_target.id, "movie-focused");
    assert!(model.app.activate_selected_series(0));

    // Narrow layout => the shell routes the resolved Series through the
    // mounted owner's Library Hero overlay.
    model.terminal_width = 80;
    model.terminal_height = 24;
    model.app.set_library_level_cursor_for_test(0, 0, 0);
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    model.app.seed_series_detail_for_test(
        "movie-focused",
        SeriesDetail {
            seasons: vec![season],
            episodes: [("season-1".into(), vec![episode])].into_iter().collect(),
        },
    );
    model.push_tv_workspace_content();
    model.sync_library_panel();
    let item = model
        .test_tv_owner()
        .selected_item()
        .expect("component-selected Series");
    model.handle_tv_request(mbv_ui_msg::ShellRequest::TvActivate { item });
    let panel = model
        .application
        .get_component(&mbv_ui_msg::ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<mbv_components::library_panel::LibraryPanel>()
        .expect("Library panel");
    assert!(panel.test_hero_overlay_open());
    assert_eq!(
        model.test_tv_owner().selected_item_id(),
        Some("movie-focused".into())
    );
    assert_eq!(
        model
            .test_tv_owner()
            .selected_episode_item()
            .map(|item| item.id),
        Some("episode-1".into()),
        "the overlay Workspace keeps its stable child target"
    );

    // Guard 1: a non-tvshows collection_type rejects.
    model.app.set_library_collection_type_for_test(0, "movies");
    assert!(!model.app.activate_selected_series(0));

    // Guard 2: a selected item that is not a Series rejects.
    model.app.set_library_collection_type_for_test(0, "tvshows");
    model
        .app
        .set_library_row_item_type_for_test(0, 0, 0, "Movie");
    assert!(!model.app.activate_selected_series(0));
}

/// Wide TV layout: the generic browser's `EmbyLibraryActivate` for a Series
/// enters the persistent Workspace through the shared gate. Regression for the
/// row 2.7 review (commit 56878761d): routing that arm straight to
/// `select_item` drilled into a flat browse list instead, because a Series is a
/// folder.
#[test]
fn wide_emby_library_activate_enters_the_workspace_not_a_flat_drill_in() {
    let mut model = mounted_tv_model();
    model.sync_library_panel();
    assert!(
        model.app.tv_workspace_painted_wide_for_test(0),
        "the wide fixture publishes the Client's write-back fact"
    );
    let item = model
        .app
        .emby_library_level(0)
        .and_then(|level| level.items.first())
        .cloned()
        .expect("the TV library's first Series row");
    let depth_before = model.app.emby_library_level_count(0);

    model.handle_emby_library_request(mbv_ui_msg::ShellRequest::EmbyLibraryActivate { item });

    assert_eq!(
        model.app.emby_library_level_count(0),
        depth_before,
        "wide Series activation keeps the persistent Workspace, never a drill-in"
    );
}

/// A tree-selected show opens its Hero even when the old flat carrier still
/// points at another show.
#[test]
fn narrow_show_activation_gates_hero_on_tree_selection_not_flat_carrier() {
    let mut model = mounted_tv_model();
    model.terminal_width = 80;
    model.terminal_height = 24;
    // The narrow TV layout needs the queue column: with the empty-queue
    // column hidden (change `hide-empty-queue-column`, D1) an empty
    // displayed queue would widen the library into the wide breakpoint.
    model
        .app
        .seed_local_queue_for_test(vec![make_item("Queue Item", "Movie")], 0);
    model.push_tv_workspace_content();
    model.sync_library_panel();

    // Move only the tree selection. Until the shell handles the row-selection
    // request, the legacy flat carrier still points at the first show.
    let moved = model.test_tv_owner_mut().test_key(&KeyEvent {
        code: Key::Down,
        modifiers: KeyModifiers::NONE,
    });
    assert!(matches!(
       moved,
       Some(Msg::Shell(ref shell_boxed))
    if matches!(shell_boxed.as_ref(),
           mbv_ui_msg::ShellRequest::TvHitClick { .. }
       )));
    assert_eq!(
        model.test_tv_owner().selected_item().map(|item| item.id),
        Some("movie-focused".into()),
        "the flat carrier is intentionally stale"
    );
    let selected_show = model
        .test_tv_owner()
        .selected_tree_show()
        .expect("tree selects the second show");
    assert_eq!(selected_show.id, "movie-second");

    model.handle_tv_request(mbv_ui_msg::ShellRequest::TvActivate {
        item: selected_show,
    });

    let panel = model
        .application
        .get_component(&mbv_ui_msg::ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<mbv_components::library_panel::LibraryPanel>()
        .expect("Library panel");
    assert!(panel.test_hero_overlay_open());
}

/// Regression for the replace-wide-paint-inference review finding (#643):
/// `activate_selected_series_item` must gate wide/narrow on the *caller's*
/// `lib_idx`, not a hardcoded 0. Movies sits at library index 0 (never wide
/// TV) and the wide-eligible TV Shows library sits at index 1, mirroring a
/// common multi-library Emby account. Activating the Series selected in
/// library 1 must enter the wide persistent workspace, not fall back to the
/// Library Hero overlay.
#[test]
fn activate_selected_series_gates_on_the_caller_supplied_lib_idx_not_zero() {
    let mut app = make_movie_app();
    // library 0 stays "movies" (never wide TV eligible).
    let mut tv_app = make_movie_app();
    let mut tv_lib = tv_app
        .take_libraries_for_test()
        .into_iter()
        .next()
        .expect("the fixture's TV library");
    tv_lib.library.id = "lib-tvshows".into();
    tv_lib.library.collection_type = "tvshows".into();
    for item in &mut tv_lib.nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.push_library_for_test(tv_lib);
    app.seed_active_tab_for_test(TabSelection::EmbyLibrary(1));
    let mut model = Model::new(app);
    model.terminal_width = 160;
    model.terminal_height = 40;
    model.sync_tv_content();
    model.sync_active_destination();

    assert!(
        crate::app::shell::tv_wide::wide_tv_show_area(&model.app, 0, model.terminal_size())
            .is_none()
    );
    assert!(
        crate::app::shell::tv_wide::wide_tv_show_area(&model.app, 1, model.terminal_size())
            .is_some()
    );

    assert!(model.app.activate_selected_series(1));
    assert!(
        model.app.pending_overlay_request_for_test().is_none(),
        "library 1 is wide-eligible; activation must enter the persistent \
         workspace, not open a separate constituent picker"
    );
}
