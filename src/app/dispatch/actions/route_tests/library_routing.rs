use super::*;

#[test]
fn enqueue_route_conflict_rejects_mismatched_route() {
    let mut app = make_app_stub();
    app.active_route = Some("music".to_string());
    assert!(app.enqueue_route_conflict(Some(&"movies".to_string())));
    assert!(app.status.contains("Can't mix libraries in a routed queue"));
}

#[test]
fn enqueue_route_conflict_allows_enqueue_while_attached_to_a_session() {
    // A Sessions-panel attached session (`connected_session_id`) has
    // its own, separate queue-scope rules -- the library-routing
    // invariant must not fire a "Can't mix libraries" toast for a
    // reason unrelated to library routing.
    let mut app = make_app_stub();
    app.connected_session_id = Some("sess-1".to_string());
    assert!(!app.enqueue_route_conflict(Some(&"music".to_string())));
}

#[test]
fn enqueue_route_conflict_allows_enqueue_while_on_a_non_route_direct_remote() {
    let mut app = make_app_stub();
    let (remote, remote_rx) = mbv_core::remote_player::RemotePlayer::stub(make_items(1), 0);
    app.player = mbv_core::player::PlayerProxy::remote(remote, false);
    app.player_rx = remote_rx;
    // active_route stays None: this is a Sessions-panel direct-remote
    // connection, not a library route.
    assert!(!app.enqueue_route_conflict(Some(&"music".to_string())));
}

#[test]
fn play_item_skips_library_routing_when_attached_to_a_session() {
    let mut app = make_app_stub();
    app.library_routes
        .insert("music".to_string(), "living-room-pc".to_string());
    app.connected_session_id = Some("sess-1".to_string());
    let mut lib_item = make_item("Music", "CollectionFolder");
    lib_item.id = "lib-music".to_string();
    app.libs.push(LibraryTab::new(lib_item));
    let mut item = make_item("Song", "Audio");
    item.id = "song-1".to_string();

    // No DAEMON_ROUTE_CONNECT_OVERRIDE set -- if library routing
    // engaged here it would attempt a real connection and this test
    // would hang/fail rather than reach the assertion below.
    app.play_item(item);

    assert!(app.active_route.is_none());
}

#[test]
fn play_item_submits_selected_item_to_direct_remote_owner() {
    let mut app = make_app_stub();
    let stale_item = make_item("Stale", "Movie");
    let (remote, remote_rx, command_rx) =
        mbv_core::remote_player::RemotePlayer::stub_with_command_rx(vec![stale_item], 0);
    let sess = crate::app::tests::make_session("remote-mbv", "mbv");
    app.switch_to_direct_remote(
        &sess,
        remote,
        remote_rx,
        &mbv_core::remote_player::DaemonEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
    );

    let mut selected = make_item("Selected", "Movie");
    selected.id = "selected-id".into();
    app.play_item(selected);

    let mut replacement = None;
    for command in command_rx.try_iter() {
        if let mbv_core::ctrl::CtrlCmd::UnifiedQueueReplace { slots, .. } = command {
            replacement = Some(slots);
            break;
        }
    }
    let slots = replacement.expect("play should submit a queue");
    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].item.id(), "selected-id");
}

/// Row 3.6: library autoplay (`select_item` with autoload) replaces a
/// populated queue without the replacement confirmation -- a browse
/// activation is not a user-initiated queue replacement.
#[test]
fn library_autoplay_on_a_populated_queue_does_not_raise_the_replace_modal() {
    let _guard = crate::config::TestStateDirGuard::new();
    let mut app = make_app_stub();
    let http = MockHttp::new();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    install_test_emby(&mut app, config);
    app.config.lock().unwrap().autoload = true;
    let client = app
        .emby_runtime
        .client
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .clone()
        .with_test_agent(http.agent());
    app.emby_runtime = mbv_core::service_runtime::EmbyRuntime::ready(std::sync::Arc::new(
        std::sync::Mutex::new(client),
    ));

    // Populated target queue + a browse level whose parent holds the item.
    let mut existing = make_item("Existing", "Movie");
    existing.id = "existing".into();
    app.player_tab.set_items(vec![existing], 0);
    let mut anchor = make_item("Anchor", "Movie");
    anchor.id = "anchor-1".into();
    let mut sibling = make_item("Sibling", "Movie");
    sibling.id = "sibling-1".into();
    app.libs.push(LibraryTab {
        nav_stack: vec![BrowseLevel {
            parent_id: "parent-1".into(),
            title: "Movies".into(),
            items: vec![anchor.clone()],
            fetched_rows: 1,
            total_count: 1,
            resting: crate::app::state::types::browse::BrowseResting::new(0, 0),
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
        ..LibraryTab::new(anchor.clone())
    });

    // Request 1: `get_items_by_ids` resolves the activated row; request 2:
    // `get_direct_playable` resolves the autoload siblings.
    http.respond(
        200,
        r#"{"Items":[{"Id":"anchor-1","Name":"Anchor","Type":"Movie","MediaType":"Video"}]}"#,
    );
    http.respond(
        200,
        r#"{"Items":[
            {"Id":"anchor-1","Name":"Anchor","Type":"Movie","MediaType":"Video"},
            {"Id":"sibling-1","Name":"Sibling","Type":"Movie","MediaType":"Video"}
        ]}"#,
    );

    app.select_item(0, anchor.clone());

    assert!(
        !matches!(
            app.pending_overlay,
            Some(crate::app::state::types::overlay::OverlayRequest::Confirm(
                _
            ))
        ),
        "library autoplay is never gated"
    );
    assert!(app.pending_queue_replacement.is_none());
    assert_eq!(
        app.playback_queue()
            .emby_items()
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        ["anchor-1", "sibling-1"]
    );
}
