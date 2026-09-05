use super::*;

#[test]
fn wide_tv_handoff_does_not_fetch_empty_series_id() {
    let mut app = make_movie_app();
    app.libs[0].library.collection_type = "tvshows".into();
    app.libs[0].nav_stack[0].items[0].item_type = "Series".into();
    app.libs[0].nav_stack[0].items[0].id.clear();
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    let mut model = Model::new(app);
    let mut client = mbv_core::api::EmbyClient::new(crate::config::Config::default());
    client.apply_credential_exchange(&mbv_core::api::EmbyCredentialExchange {
        server_url: "http://127.0.0.1:1".into(),
        user_id: "user-id".into(),
        token: "token".into(),
    });
    model.app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    // This is the first wide-TV handoff, so the mounted component cannot have
    // already captured a valid ID before the guard is exercised.
    model.sync_tv_workspace();
    model.sync_active_destination();

    assert!(model.app.series_detail_loading.is_empty());
    assert!(model.app.series_detail_cache.is_empty());
}

#[test]
fn activate_selected_series_resolves_mirrored_cursor_and_guards_series() {
    let mut model = mounted_tv_model();
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.sync_tv_workspace();
    model.sync_active_destination();
    model.sync_emby_browser();

    // Divergence: the mounted BrowserComponent selects index 0 while App's
    // mirrored BrowseLevel cursor is stale at index 1. Resolve narrow extras
    // using the component-owned cursor, as the production seam does.
    model.app.libs[0].nav_stack[0].set_resting_cursor(1);
    let component_cursor = model
        .application
        .get_component(&model.emby_browser_id.clone().expect("browser mounted"))
        .expect("browser mounted")
        .as_any()
        .downcast_ref::<crate::app::components::BrowserComponent>()
        .expect("browser component")
        .cursor();
    assert_eq!(component_cursor, 0);
    assert_eq!(model.app.libs[0].nav_stack[0].resting().cursor(), 1);

    let component_extras = model.app.narrow_browse_extras(0, component_cursor);
    match component_extras.inline_hero {
        Some(NarrowInlineHero::Series { item, .. }) => {
            assert_eq!(item.id, "movie-focused");
        }
        _ => panic!("component cursor must resolve the focused Series"),
    }
    let stale_extras = model
        .app
        .narrow_browse_extras(0, model.app.libs[0].nav_stack[0].resting().cursor());
    match stale_extras.inline_hero {
        Some(NarrowInlineHero::Series { item, .. }) => {
            assert_eq!(item.id, "movie-second");
        }
        _ => panic!("stale App cursor must resolve the other Series"),
    }

    model.app.libs[0].nav_stack[0].set_resting_cursor(component_cursor);

    // Wide TV layout => enter_series_selection targets the component's
    // Series (asserted by the resolved target, not merely the bool).
    let wide_target = model
        .app
        .selected_series_item(0, model.app.libs[0].nav_stack[0].resting().cursor())
        .expect("series");
    assert_eq!(wide_target.id, "movie-focused");
    assert!(model.app.activate_selected_series(0));

    // Narrow layout => open_series_selection_modal targets the same
    // Series, proven by the modal's Series source id.
    model.app.terminal_width = 80;
    model.app.terminal_height = 24;
    model.app.libs[0].nav_stack[0].set_resting_cursor(0);
    model.app.activate_selected_series(0);
    match model.app.pending_overlay.as_ref() {
        Some(crate::app::types_overlay::OverlayRequest::SelectionModal(modal)) => {
            if let crate::app::types_selection_modal::SelectionModalSource::Series { series_id } =
                &modal.source
            {
                assert_eq!(
                    series_id.as_str(),
                    "movie-focused",
                    "narrow activation must target the component's selected Series"
                );
            } else {
                panic!("narrow activation must open a Series selection modal");
            }
        }
        _ => panic!("narrow layout must open the series selection modal"),
    }
    model.app.pending_overlay = None;

    // Guard 1: a non-tvshows collection_type rejects.
    model.app.libs[0].library.collection_type = "movies".into();
    assert!(!model.app.activate_selected_series(0));

    // Guard 2: a selected item that is not a Series rejects.
    model.app.libs[0].library.collection_type = "tvshows".into();
    model.app.libs[0].nav_stack[0].items[0].item_type = "Movie".into();
    assert!(!model.app.activate_selected_series(0));
}

/// Regression for the replace-wide-paint-inference review finding (#643):
/// `activate_selected_series_item` must gate wide/narrow on the *caller's*
/// `lib_idx`, not a hardcoded 0. Movies sits at library index 0 (never wide
/// TV) and the wide-eligible TV Shows library sits at index 1, mirroring a
/// common multi-library Emby account. Activating the Series selected in
/// library 1 must enter the wide persistent workspace, not fall back to the
/// narrow selection modal.
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
    app.layout.main.tv_wide_right_area = Rect::new(40, 0, 60, 20);
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut model = Model::new(app);
    model.sync_tv_workspace();
    model.sync_active_destination();

    assert!(model.app.wide_tv_library_area(0).is_none());
    assert!(model.app.wide_tv_library_area(1).is_some());

    assert!(model.app.activate_selected_series(1));
    assert!(
        model.app.pending_overlay.is_none(),
        "library 1 is wide-eligible; activation must enter the persistent \
         workspace, not open the narrow series selection modal"
    );
}

/// replace-wide-paint-inference completion gate (6.3): `activate_selected_series`
/// gates on `App::wide_tv_library_area`, a paint-free predicate driven solely
/// by terminal size. Resizing narrow -> wide through the real
/// `Msg::TerminalEvent(Resize)` path must flip the activation branch on that
/// same tick, before any repaint refreshes `tv_wide_left_area`/
/// `tv_wide_right_area`.
#[test]
fn tv_series_activation_branch_flips_on_resize_tick_before_repaint() {
    let mut model = mounted_tv_model();
    model.app.terminal_width = 60;
    model.app.terminal_height = 24;
    model.sync_tv_workspace();
    model.sync_active_destination();
    assert!(model.app.wide_tv_library_area(0).is_none());

    // Narrow: activation opens the Series selection modal, never the
    // persistent workspace fetch.
    assert!(model.app.activate_selected_series(0));
    assert!(
        matches!(
            model.app.pending_overlay,
            Some(crate::app::types_overlay::OverlayRequest::SelectionModal(_))
        ),
        "narrow activation must open the series selection modal"
    );
    model.app.pending_overlay = None;

    let mut music_resize = false;
    let mut tv_resize = false;
    model.handle_terminal_message(
        Msg::TerminalEvent(TerminalObserverEvent::Resize {
            width: 160,
            height: 40,
        }),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(model.app.terminal_width, 160);
    assert!(model.app.wide_tv_library_area(0).is_some());

    // Wide, on this same tick: activation must enter the persistent
    // workspace, not the narrow modal -- with no intervening repaint.
    assert!(model.app.activate_selected_series(0));
    assert!(
        model.app.pending_overlay.is_none(),
        "wide activation right after the resize tick must not open the series modal"
    );
}
