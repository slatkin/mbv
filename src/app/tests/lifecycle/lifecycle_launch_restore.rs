use super::*;
use crate::app::components::LibraryKey;

#[test]
fn pending_launch_tab_resolves_after_catalog_arrival_and_restores_existing_tab() {
    let mut app = crate::app::render::make_movie_app();
    app.tab = TabSelection::Home;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });
    app.resolve_library_tab_pending();
    assert_eq!(
        app.tab,
        TabSelection::Home,
        "catalog identity is not ready yet"
    );
    assert!(!app.pending_launch_tab_resolved);

    app.emby_catalog_ready = true;
    app.resolve_library_tab_pending();
    assert_eq!(app.tab, TabSelection::EmbyLibrary(0));
    assert!(app.pending_launch_tab_resolved);
    assert!(
        app.pending_launch_state.is_some(),
        "destination state remains for 3.2"
    );
}

/// A local-daemon/remote launch attaches a live Emby client at construction
/// and never spawns the Emby startup worker, so `apply_emby_bootstrap` never
/// runs. Its live catalog arrives through `fetch_home`'s view rebuild, which
/// must mark the catalog ready or the saved launch tab never resolves. The
/// resolved tab must also load its library's content, not just select the
/// tab, or the panel's owner is empty and the tab paints blank.
#[test]
fn restored_launch_tab_loads_its_library_content_not_just_the_tab() {
    let mut app = crate::app::render::make_movie_app();
    app.emby_catalog_ready = false;

    let mut second_library = make_item("Shows", "CollectionFolder");
    second_library.id = "lib-shows".into();
    second_library.is_folder = true;
    second_library.collection_type = "tvshows".into();
    app.libs.push(crate::app::LibraryTab::new(second_library));

    let views: Vec<mbv_core::api::EmbyItem> =
        app.libs.iter().map(|lib| lib.library.clone()).collect();
    app.rebuild_library_tabs_from_views(&views);
    assert!(
        app.emby_catalog_ready,
        "rebuilding tabs from live views is the Emby catalog boundary"
    );

    app.tab = TabSelection::Home;
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-shows".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });

    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::EmbyLibrary(1));
    assert!(app.pending_launch_tab_resolved);
    assert_eq!(
        app.libs[1].nav_stack.len(),
        1,
        "the restored tab must load its root level"
    );
    assert!(app.libs[1].nav_stack[0].loading);
    assert!(
        app.pending_launch_state.is_some(),
        "destination state remains for the pill/item re-anchor"
    );
}

#[test]
fn explicit_tab_movement_consumes_pending_launch_tab_before_refresh() {
    let mut app = crate::app::render::make_movie_app();
    app.pending_launch_state = Some(mbv_core::config::TuiLaunchState {
        version: mbv_core::config::TUI_LAUNCH_STATE_VERSION,
        tab: mbv_core::config::TabIdentity::ServiceLibrary {
            kind: ServiceKind::Emby,
            library_id: "lib-movies".into(),
        },
        panel_focus: mbv_core::config::LaunchPanelFocus::Library,
        selector: None,
        item: None,
    });
    app.set_library_tab(0);
    app.emby_catalog_ready = true;
    app.resolve_library_tab_pending();

    assert_eq!(app.tab, TabSelection::Home);
    assert!(!app.pending_launch_tab_resolved);
    assert!(app.pending_launch_state.is_none());
}

#[test]
fn orderly_teardown_writes_only_the_selected_destination_launch_snapshot() {
    let mut model = Model::new(make_app_stub());
    model.app.panel_focus = PanelFocus::Queue;
    model.app.player_tab.queue_cursor = 7;

    let mut selected = make_item("Selected home item", "Movie");
    selected.id = "selected-home-item".into();
    model.update_library_owner(
        &LibraryKey::Home,
        || Box::new(HomeContent::new()),
        |owner| {
            owner.set_content(
                vec![mbv_core::playback_queue::QueueItem::Emby(Box::new(
                    selected,
                ))],
                false,
            );
        },
    );

    let unselected_feed_item = FeedEntry {
        guid: "unselected-feed-item".into(),
        title: "Unselected feed item".into(),
        enclosure_url: Some("https://example.test/unselected-feed-item.mp3".into()),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("unselected-feed-selector".into()),
        position_ticks: 0,
        played: false,
    };
    model.update_library_owner(
        &LibraryKey::Feeds,
        || Box::new(FeedsContent::new()),
        |owner| {
            owner.set_content(FeedsOwnerPush {
                subscriptions: vec![FeedSubscription {
                    name: "Unselected feed".into(),
                    url: "https://example.test/unselected-feed-selector".into(),
                    kind: FeedKind::Audio,
                }],
                entries: vec![vec![unselected_feed_item.clone()]],
                all_entries: vec![unselected_feed_item],
                loading: false,
            });
            owner.cycle_group(1);
        },
    );
    model.sync_library_panel();

    model.teardown(Duration::from_secs(1));

    let state = mbv_core::config::load_tui_launch_state().expect("launch snapshot after teardown");
    assert_eq!(state.tab, mbv_core::config::TabIdentity::Home);
    assert_eq!(state.panel_focus, mbv_core::config::LaunchPanelFocus::Queue);
    assert_eq!(
        state.selector,
        Some(mbv_core::config::SelectorIdentity::Home {
            key: mbv_core::config::HomeSelectorKey::Continue,
        })
    );
    assert_eq!(
        state.item,
        Some(mbv_core::config::LibraryItemIdentity::Home {
            id: "selected-home-item".into(),
        })
    );

    let serialized = std::fs::read_to_string(mbv_core::config::tui_launch_state_path())
        .expect("serialized launch snapshot");
    assert!(!serialized.contains("queue_cursor"));
    assert!(!serialized.contains("queue_slot"));
    assert!(!serialized.contains("unselected-feed-item"));
    assert!(!serialized.contains("unselected-feed-selector"));
}
