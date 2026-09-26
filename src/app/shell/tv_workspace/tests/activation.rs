use super::*;
use crate::app::tests::make_item;
use crate::app::SeriesDetail;

#[test]
fn activate_selected_series_resolves_mirrored_cursor_and_guards_series() {
    let mut model = mounted_tv_model();
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.sync_tv_content();
    model.sync_active_destination();

    // Divergence: the panel-hosted TvContent owner selects index 0 while
    // App's mirrored BrowseLevel cursor is stale at index 1. Resolve narrow
    // extras using the component-owned cursor, as the production seam does.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let component_cursor = model.test_tv_owner().browse_cursor();
    assert_eq!(component_cursor, 0);
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 1);

    assert_eq!(
        model.test_tv_owner().selected_item_id(),
        Some("movie-focused".into()),
        "the shared TV owner resolves its own selected Series"
    );

    model.app.libs[0].nav_stack[0].set_resting_cursor(component_cursor);

    // Wide TV layout => enter_series_selection targets the component's
    // Series (asserted by the resolved target, not merely the bool).
    let wide_target = model
        .app
        .selected_series_item(0, model.app.libs[0].nav_stack[0].resting().cursor())
        .expect("series");
    assert_eq!(wide_target.id, "movie-focused");
    assert!(model.app.activate_selected_series(0));

    // Narrow layout => the shell routes the resolved Series through the
    // mounted owner's Library Hero overlay.
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.app.libs[0].nav_stack[0].set_resting_cursor(0);
    let mut season = make_item("Season 1", "Season");
    season.id = "season-1".into();
    let mut episode = make_item("Episode 1", "Episode");
    episode.id = "episode-1".into();
    model.app.series_detail_cache.insert(
        "movie-focused".into(),
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
    model.handle_tv_request(crate::app::components::ShellRequest::TvActivate { item });
    let panel = model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
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
    model.app.libs[0].library.collection_type = "movies".into();
    assert!(!model.app.activate_selected_series(0));

    // Guard 2: a selected item that is not a Series rejects.
    model.app.libs[0].library.collection_type = "tvshows".into();
    model.app.libs[0].nav_stack[0].items[0].item_type = "Movie".into();
    assert!(!model.app.activate_selected_series(0));
}

/// A tree-selected show opens its Hero even when the old flat carrier still
/// points at another show.
#[test]
fn narrow_show_activation_gates_hero_on_tree_selection_not_flat_carrier() {
    let mut model = mounted_tv_model();
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
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
           crate::app::components::ShellRequest::TvHitClick { .. }
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

    model.handle_tv_request(crate::app::components::ShellRequest::TvActivate {
        item: selected_show,
    });

    let panel = model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
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
    let mut tv_lib = tv_app.libs.remove(0);
    tv_lib.library.id = "lib-tvshows".into();
    tv_lib.library.collection_type = "tvshows".into();
    for item in &mut tv_lib.nav_stack[0].items {
        item.item_type = "Series".into();
    }
    app.libs.push(tv_lib);
    app.tab = TabSelection::EmbyLibrary(1);
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
    model.sync_tv_content();
    model.sync_active_destination();

    assert!(model.app.wide_tv_library_area(0).is_none());
    assert!(model.app.wide_tv_library_area(1).is_some());

    assert!(model.app.activate_selected_series(1));
    assert!(
        model.app.pending_overlay.is_none(),
        "library 1 is wide-eligible; activation must enter the persistent \
         workspace, not open a separate constituent picker"
    );
}
