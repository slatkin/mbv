use super::*;

fn landed_album_level(
    parent_id: &str,
    title: &str,
    items: Vec<mbv_core::api::EmbyItem>,
) -> crate::app::BrowseLevel {
    crate::app::BrowseLevel {
        fetched_rows: 0,
        parent_id: parent_id.into(),
        title: title.into(),
        total_count: items.len(),
        items,
        resting: crate::app::types_browse::BrowseResting::new(0, 0),
        item_types: None,
        unplayed_only: false,
        sort_by: "SortName".into(),
        sort_order: "Ascending".into(),
        loading: false,
        all_items: None,
        letter_filter: None,
        music_grouping: None,
    }
}

/// The landed nav stack the recursive album activation builds for a grouped
/// library: root artist level + the artist's album level, the navigated album
/// at the album level's resting cursor. The sibling album is present (the
/// activation fetches the whole parent listing), so preserving the owner's
/// prior target would miss the navigated album without the re-anchor.
fn landed_grouped_album_stack(album_id: &str) -> Vec<crate::app::BrowseLevel> {
    let mut artist = crate::app::tests::make_item("Alpha", "MusicArtist");
    artist.id = "group-0".into();
    artist.is_folder = true;
    let mut album = crate::app::tests::make_item("First Album", "MusicAlbum");
    album.id = album_id.into();
    album.artist = "Alpha".into();
    let mut sibling = crate::app::tests::make_item("Second Album", "MusicAlbum");
    sibling.id = "album-2".into();
    sibling.artist = "Alpha".into();
    vec![
        landed_album_level("lib-music", "Music", vec![artist]),
        landed_album_level("group-0", "Alpha", vec![album, sibling]),
    ]
}

/// Task 3.2 (grouped shape): a navigated album lands through the same
/// `RecursiveAlbumActivated` shell arm Inline Search activation uses, so the
/// retained Music owner's workspace re-anchors onto the navigated album and
/// shows its track list -- even after the user moved the owner's own cursor
/// elsewhere.
#[test]
fn navigated_album_reanchors_the_grouped_owner_workspace() {
    let (mut harness, id) = wide_music_harness();
    // A sibling album so the owner's own cursor can move off the navigated
    // one before the landing.
    {
        let app = &mut harness.model_mut().app;
        let mut second = crate::app::tests::make_item("Second Album", "MusicAlbum");
        second.id = "album-2".into();
        second.artist = "Alpha".into();
        let level = app.libs[0].nav_stack.last_mut().expect("album level");
        level.items.push(second);
        level.total_count = 2;
        app.album_tracks_cache.insert(
            "album-2".into(),
            vec![crate::app::tests::make_item("Other Track", "Audio")],
        );
    }
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-2"),
        "the owner's local cursor moved off the navigated album"
    );

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the owner workspace re-anchors onto the navigated album"
    );
    assert_eq!(
        music_track_focus_row(&harness, &id),
        Some(0),
        "track-selection mode is entered for the navigated album's track list"
    );
}

/// Task 6.2 (design D6): "Go to Library" on a queued track selects the track
/// in the workspace track list once the activated album's track rows arrive —
/// including when the fetch lands after the landing.
#[test]
fn navigated_track_is_selected_in_the_workspace_track_list() {
    let (mut harness, id) = wide_music_harness();
    // The queued track: the navigation carries it as deep selection bound to
    // the activated album.
    harness.model_mut().app.pending_track_selection = Some((0, "track-2".into()));

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the album landing stands"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the chosen track is selected in the track list"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the deep selection is fully consumed"
    );
    assert_ne!(
        harness.model().app.status_severity,
        crate::app::notify_actions::ToastSeverity::Error,
        "a successful deep selection does not flash: {}",
        harness.model().app.status
    );
}

/// Task 6.2: the track is absent from the fetched track list — the album
/// landing stands with the default selection and no error.
#[test]
fn absent_track_keeps_the_landing_with_default_selection() {
    let (mut harness, id) = wide_music_harness();
    harness.model_mut().app.pending_track_selection = Some((0, "track-gone".into()));

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the album landing stands"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(0),
        "default selection: first track"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the absent-track pending is cleared"
    );
    assert_ne!(
        harness.model().app.status_severity,
        crate::app::notify_actions::ToastSeverity::Error,
        "absence is not failure: {}",
        harness.model().app.status
    );
}

/// Task 6.2: the album's tracks have not arrived when the activation drains —
/// the pending selection stays armed and applies on the tracks re-push.
#[test]
fn navigated_track_selection_waits_for_the_album_tracks() {
    let mut app = crate::app::render::make_music_group_app();
    app.terminal_width = 160;
    app.terminal_height = 40;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _id = ComponentId::Library;

    harness.model_mut().app.pending_track_selection = Some((0, "track-2".into()));
    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("album-1"));
    harness.step();
    assert!(
        harness.model().pending_music_track_selection.is_some(),
        "the pending selection stays armed while the track fetch is in flight"
    );

    // The tracks arrive (the `AlbumTracksFetched` drain re-pushes).
    let mut first = crate::app::tests::make_item("Track One", "Audio");
    first.id = "track-1".into();
    let mut second = crate::app::tests::make_item("Track Two", "Audio");
    second.id = "track-2".into();
    harness
        .model_mut()
        .app
        .album_tracks_cache
        .insert("album-1".into(), vec![first, second]);
    harness.model_mut().sync_mounted_surfaces();

    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the chosen track is selected once its rows arrive"
    );
    assert!(harness.model().pending_music_track_selection.is_none());
}

// ── Task 2.3: the tree is the one Grouped Music browser owner and painter ──

pub(super) fn music_panel(
    harness: &TickHarness,
) -> &crate::app::components::library_panel::LibraryPanel {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("LibraryPanel")
}

/// Draw one frame at the model's own terminal size (the live paint path).
pub(super) fn draw_music_frame(harness: &mut TickHarness) {
    let width = harness.model().app.terminal_width;
    let height = harness.model().app.terminal_height;
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("test terminal");
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .expect("music frame");
}

/// The mounted Music app at one Panel-mode fixture, drawn once.
fn mounted_music_at(width: u16, height: u16) -> (TickHarness, ComponentId) {
    mounted_music_app_at(crate::app::render::make_music_group_app(), width, height)
}

/// The mounted Music app for a supplied fixture at one Panel-mode geometry,
/// drawn once.
pub(super) fn mounted_music_app_at(
    mut app: crate::app::App,
    width: u16,
    height: u16,
) -> (TickHarness, ComponentId) {
    app.terminal_width = width;
    app.terminal_height = height;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    // A single-panel fixture keeps the Library panel's own area the test input
    // at every breakpoint instead of a split whose pane size is an arrangement
    // fact.
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    (harness, ComponentId::Library)
}

pub(super) fn music_panel_mut(
    harness: &mut TickHarness,
) -> &mut crate::app::components::library_panel::LibraryPanel {
    harness
        .model_mut()
        .application
        .get_component_mut(&ComponentId::Library)
        .expect("library panel mounted")
        .as_any_mut()
        .downcast_mut::<crate::app::components::library_panel::LibraryPanel>()
        .expect("LibraryPanel")
}

/// The tree's summary, context origin, and capability gate all cross the
/// mounted composition path: modified clicks mutate the owner, the status
/// projection observes only its count, and folder albums cannot manufacture
/// context actions merely because they were selected.
#[test]
fn grouped_music_tree_selection_projects_status_and_context_origin() {
    let mut app = crate::app::render::make_music_group_app();
    let mut second = crate::app::tests::make_item("Second Album", "MusicAlbum");
    second.id = "album-2".into();
    second.artist = "Alpha".into();
    second.is_folder = true;
    app.libs[0].nav_stack[1].items[0].is_folder = true;
    app.libs[0].nav_stack[1].items.push(second);
    app.terminal_width = 100;
    app.terminal_height = 30;
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let album_points = {
        let music = harness.model().test_music_owner();
        ["album-1", "album-2"].map(|target| {
            let node = music
                .browser
                .visible_targets()
                .into_iter()
                .find(|candidate| candidate.album_leaf_target() == Some(target))
                .expect("painted album node");
            let point = (0..256u16)
                .find_map(|y| {
                    (0..256u16).find_map(|x| {
                        (music
                            .browser
                            .resolve_current_point(ratatui::layout::Position::new(x, y))
                            == Some(&node))
                        .then_some((x, y))
                    })
                })
                .expect("painted album row");
            // Click inside the row's text area: a selected row's retained
            // rect starts at the panel's claim edge, two columns left of the
            // content rect the panel delivers within.
            (point.0 + 2, point.1)
        })
    };
    let click = |column, row, modifiers| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers,
        })
    };
    for (column, row) in album_points {
        harness.inject(click(column, row, KeyModifiers::CONTROL));
        let outcome = harness.step();
        let (mut music_resize, mut tv_resize) = (false, false);
        for message in outcome.messages {
            harness
                .model_mut()
                .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
        }
        harness.model_mut().sync_mounted_surfaces();
        // Each modified click's mutation invalidates the tree's completed
        // frame; re-paint so the next click resolves the latest geometry.
        draw_music_frame(&mut harness);
        harness.model_mut().sync_mounted_surfaces();
    }

    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .selected_album_targets_in_display_order(),
        vec!["album-1".to_string(), "album-2".to_string()]
    );
    assert_eq!(
        harness.model().visual_selection,
        Some((PanelFocus::Library, 2)),
        "status projection carries the tree count, not tree membership"
    );

    let right = Event::Mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Right),
        column: album_points[0].0,
        row: album_points[0].1,
        modifiers: KeyModifiers::NONE,
    });
    harness.inject(right);
    let outcome = harness.step();
    let context_items = outcome.messages.iter().find_map(|message| match message {
        Msg::Shell(ShellRequest::MusicRowContextMenu(
            crate::app::types_context_menu::ContextMenuTargets::Emby(items),
            _,
        )) => Some(items),
        _ => None,
    });
    assert_eq!(
        context_items
            .expect("tree selection reaches the existing bulk path")
            .iter()
            .map(|item| item.id.as_str())
            .collect::<Vec<_>>(),
        vec!["album-1", "album-2"]
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    assert_eq!(
        harness.model().context_menu_origin,
        Some(
            crate::app::components::media_list::SelectionOrigin::Library(
                crate::app::components::media_list::LibrarySelectionOrigin::Service(
                    crate::app::components::library_panel::owner::LibraryKey::Service {
                        service: mbv_core::config::ServiceKind::Emby,
                        library_id: "lib-music".into(),
                        kind: crate::app::components::library_panel::owner::LibraryKind::Music,
                    },
                ),
            )
        ),
        "bulk context captures the originating library identity"
    );
    assert_eq!(
        harness.model().visual_selection,
        Some((PanelFocus::Library, 2)),
        "folder-gated albums do not bypass capability intersection"
    );

    // The status projection has a clickable clear affordance; exercise its
    // captured-origin contract through the same shell request the mounted
    // panel emits (the status component's pointer-delivery coverage is shared
    // with the other library destinations).
    draw_music_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness
        .model()
        .application
        .get_component(&ComponentId::StatusBarPanel)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::StatusBarPanel>()
        })
        .and_then(|panel| panel.regions().visual_clear)
        .is_some());
    let origin = crate::app::components::media_list::SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Service(
            crate::app::components::library_panel::owner::LibraryKey::Service {
                service: mbv_core::config::ServiceKind::Emby,
                library_id: "lib-music".into(),
                kind: crate::app::components::library_panel::owner::LibraryKind::Music,
            },
        ),
    );
    let wrong_origin = crate::app::components::media_list::SelectionOrigin::Library(
        crate::app::components::media_list::LibrarySelectionOrigin::Home,
    );
    let (mut music_resize, mut tv_resize) = (false, false);
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ClearMultiSelection(wrong_origin)),
        &mut music_resize,
        &mut tv_resize,
    );
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .selected_album_targets()
            .len(),
        2,
        "a clear for another Library origin cannot clear the tree"
    );
    harness.model_mut().handle_terminal_message(
        Msg::Shell(ShellRequest::ClearMultiSelection(origin)),
        &mut music_resize,
        &mut tv_resize,
    );
    assert!(harness
        .model()
        .test_music_owner()
        .selected_album_targets()
        .is_empty());
}

/// Inject one key through the real router, dispatch every surviving message
/// through the shell, and re-run the production sync pass.
pub(super) fn tick_key(harness: &mut TickHarness, code: Key) {
    harness.inject(key(code));
    let outcome = harness.step();
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

/// Task 2.3: at every Panel mode the Grouped Music browser's one owner and
/// painter is the destination-local tree. The Library panel's browser slot
/// drives it through the object-safe `PanelList` surface, so the panel's
/// retained browser geometry *is* the tree's own latest-render geometry and the
/// tree's own hit map claims the row it painted. The removed parallel flat album
/// carrier has no field, painter, or point-resolution path left to run here.
#[test]
fn grouped_music_browser_has_one_tree_owner_and_painter_in_every_panel_mode() {
    for (width, height) in [(160, 40), (81, 30), (60, 30)] {
        let (harness, id) = mounted_music_at(width, height);
        let owner = music_workspace(&harness, &id);

        // The panel drove exactly one skeleton this frame (Wide xor Narrow),
        // and its browser slot's selected row is the tree's own retained row.
        let panel = music_panel(&harness);
        let (wide, narrow) = (panel.test_wide_geometry(), panel.test_narrow_geometry());
        assert!(
            wide.is_some() ^ narrow.is_some(),
            "{width}x{height}: exactly one Library skeleton painted a browser slot"
        );
        let browser = wide.or(narrow).expect("a browser slot painted");
        let tree_selected = owner.browser.selected_row_rect();
        assert!(
            tree_selected.is_some(),
            "{width}x{height}: the tree retained its selected row"
        );
        assert_eq!(
            browser.selected, tree_selected,
            "{width}x{height}: the panel's browser geometry is the tree's latest-render row"
        );

        // The tree's own hit map — not a second carrier — claims the row it
        // painted, and the panel's browser list rect contains that row.
        let row = tree_selected.expect("tree selected row");
        let position = ratatui::layout::Position { x: row.x, y: row.y };
        assert!(
            owner.browser.claims_current_point(position),
            "{width}x{height}: the tree claims the row it painted"
        );
        assert!(
            browser.list_area.contains(position),
            "{width}x{height}: the painted row sits in the browser slot"
        );

        // The projected browser flow is the tree's artist-root/album-leaf
        // model: an artist root row and at least one album leaf row. The
        // removed flat album carrier projected no focusable artist root.
        let targets = owner.album_flow_targets();
        assert!(
            targets.iter().any(Option::is_none),
            "{width}x{height}: the tree projects an artist root row"
        );
        assert!(
            targets.iter().any(Option::is_some),
            "{width}x{height}: the tree projects album leaf rows"
        );
    }
}

/// Task 2.3 / design D3: a responsive Panel-mode change reuses the *same* tree
/// owner instead of copying its selection into another control. The selected
/// album and the projected row flow survive Wide -> Mini -> Wide unchanged.
#[test]
fn grouped_music_browser_reuses_one_tree_owner_across_panel_modes() {
    let (mut harness, id) = mounted_music_at(160, 40);
    let selected = music_workspace(&harness, &id)
        .browser
        .selected_target()
        .and_then(|target| target.album_leaf_target())
        .map(str::to_owned);
    assert!(
        selected.is_some(),
        "the tree adopted the shell's projected album"
    );
    let wide_flow = music_workspace(&harness, &id).album_flow_targets();

    // Shrink to Mini (single panel, narrow skeleton): the same owner, no
    // re-adoption, same visible projection.
    harness.model_mut().app.terminal_width = 60;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_narrow_geometry().is_some(),
        "Mini painted the narrow skeleton"
    );
    let mini = music_workspace(&harness, &id);
    assert_eq!(
        mini.selected_album_target().as_deref().map(str::to_owned),
        selected,
        "the responsive change keeps the tree's selected album"
    );
    assert_eq!(
        mini.album_flow_targets(),
        wide_flow,
        "the responsive change keeps the tree's visible projection"
    );

    // Grow back to Wide: still the same owner and selection.
    harness.model_mut().app.terminal_width = 160;
    harness.model_mut().sync_mounted_surfaces();
    draw_music_frame(&mut harness);
    assert!(
        music_panel(&harness).test_wide_geometry().is_some(),
        "Wide painted the wide skeleton"
    );
    let wide = music_workspace(&harness, &id);
    assert_eq!(
        wide.selected_album_target().as_deref().map(str::to_owned),
        selected,
        "the tree's selection survives the full round trip"
    );
    assert_eq!(
        wide.album_flow_targets(),
        wide_flow,
        "the tree's visible projection survives the full round trip"
    );
}

// ── Task 6.1/6.2: Go to Library lands the configured Grouped Music album ──

/// A Grouped Music app with a scripted in-memory Emby transport installed,
/// the same boundary `spawn_navigate_to_item` resolves through.
fn grouped_music_app_with_mock_emby(http: &mbv_core::mock_http::MockHttp) -> crate::app::App {
    let mut app = crate::app::render::make_music_group_app();
    let mut config = app.config.lock().unwrap().clone();
    config.server_url = "http://127.0.0.1:1".into();
    crate::app::tests::install_test_emby(&mut app, config);
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
    app
}

/// Queue the scripted Emby responses for a queued-track "Go to Library"
/// landing on `alb1` in the `lib-music` grouped fixture (`music.levels`
/// `["group", "album"]`): the track + album fetches, the configured
/// album-index walk (levels 0 and 1), then the activation worker's level
/// fetches. Response #3 is also the body the pre-fix `get_ancestors` round
/// trip would have read (a listing object, silently an empty ancestor chain),
/// which is exactly the depth mismatch D7 diagnoses.
fn script_grouped_album_landing(http: &mbv_core::mock_http::MockHttp) {
    http.respond(200, r#"{"Items":[{"Id":"trk1","Name":"Song","Type":"Audio","AlbumId":"alb1"}],"TotalRecordCount":1}"#);
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"group-0","Name":"Alpha","Type":"MusicArtist"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"group-0","Name":"Alpha","Type":"MusicArtist"}],"TotalRecordCount":1}"#,
    );
    http.respond(
        200,
        r#"{"Items":[{"Id":"alb1","Name":"The Album","Type":"MusicAlbum"}],"TotalRecordCount":1}"#,
    );
}

/// Feed the queued-track navigation through the production shell drains: the
/// resolve worker's `NavigateTo`, then the recursive-activation drain.
fn drive_grouped_album_navigation(harness: &mut TickHarness, item_id: &str, item_type: &str) {
    harness.model_mut().app.spawn_navigate_to_item(
        item_id.into(),
        item_type.into(),
        vec![(0, "lib-music".into(), "music".into())],
    );
    let ev = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("navigate event");
    assert!(
        matches!(ev, crate::app::LibEvent::NavigateTo { .. }),
        "expected a NavigateTo landing"
    );
    harness.model_mut().handle_inline_search_lib_event(ev);

    let ev = harness
        .model()
        .app
        .lib_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("album activated");
    let crate::app::LibEvent::RecursiveAlbumActivated {
        library_id,
        nav_stack,
    } = ev
    else {
        panic!("expected RecursiveAlbumActivated");
    };
    harness
        .model_mut()
        .on_recursive_album_activated(library_id, nav_stack);
}

/// A structural snapshot of the shell's nav stack (`BrowseLevel` is neither
/// Clone nor PartialEq): parent id, item ids, and resting cursor per level.
fn nav_stack_snapshot(harness: &TickHarness) -> Vec<(String, Vec<String>, usize)> {
    harness.model().app.libs[0]
        .nav_stack
        .iter()
        .map(|level| {
            (
                level.parent_id.clone(),
                level.items.iter().map(|item| item.id.clone()).collect(),
                level.resting().cursor(),
            )
        })
        .collect()
}

/// Task 6.1 (design D7): the reported silent no-op in the user's shape — a
/// Grouped Music destination already mounted with a queued track. The
/// scripted ancestors response is a listing object, which the pre-fix
/// `get_ancestors` round trip reads as an EMPTY ancestor chain; the landing
/// then installs a one-level stack for which no Music owner is eligible, so
/// the tab moves but the tree never re-anchors (no fallback painter, no
/// error). After the fix the worker resolves the album through the configured
/// `music.levels` album shape and the tree lands on `alb1`.
#[test]
fn go_to_library_on_a_queued_track_lands_the_configured_grouped_album() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = mbv_core::mock_http::MockHttp::new();
    let mut app = grouped_music_app_with_mock_emby(&http);
    // The queued track: the "Go to Library" target.
    let mut track = crate::app::tests::make_item("Song", "Audio");
    track.id = "trk1".into();
    track.album_id = "alb1".into();
    app.replace_playback_queue(vec![track.clone()], 0);
    // Pre-seed the album-track cache so the sync pass never spawns a track
    // fetch that would race the navigation events on `lib_rx`.
    let mut fixture_track = crate::app::tests::make_item("Fixtures", "Audio");
    fixture_track.id = "fixture-track".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![fixture_track]);
    app.album_tracks_cache.insert("alb1".into(), vec![track]);
    script_grouped_album_landing(&http);

    // The destination is already mounted/retained before the navigation.
    let mut harness = TickHarness::new(app);
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    harness.model_mut().sync_mounted_surfaces();
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    let id = ComponentId::Library;
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-1"),
        "the retained tree starts on the fixture album"
    );

    drive_grouped_album_navigation(&mut harness, "trk1", "Audio");
    harness.step();

    assert!(
        harness.model().app.is_viewing_album_folders(0),
        "the landing installs the configured album level (silent no-op otherwise)"
    );
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("alb1"),
        "the tree re-anchors to the resolved album"
    );
}

/// Task 6.3: navigation into a previously mounted tree re-anchors to the
/// resolved album and selects the navigated track in the Workspace, even
/// though the retained owner had moved to a sibling album first.
#[test]
fn go_to_library_into_a_retained_tree_reanchors_and_selects_the_navigated_track() {
    let _guard = crate::config::TestStateDirGuard::new();
    let http = mbv_core::mock_http::MockHttp::new();
    let mut app = grouped_music_app_with_mock_emby(&http);
    app.terminal_width = 160;
    app.terminal_height = 40;
    // A sibling album so the retained tree can be moved off the navigated one.
    let mut sibling = crate::app::tests::make_item("Second Album", "MusicAlbum");
    sibling.id = "album-2".into();
    sibling.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(sibling);
    app.libs[0].nav_stack[1].total_count = 2;

    // The queued track lands on `alb1`, whose navigated track sits second in
    // the fetched track list, so a selected row of 1 proves the deep
    // selection rather than the default first row.
    let mut queued = crate::app::tests::make_item("Song", "Audio");
    queued.id = "trk1".into();
    queued.album_id = "alb1".into();
    app.replace_playback_queue(vec![queued.clone()], 0);
    let mut other = crate::app::tests::make_item("Other Song", "Audio");
    other.id = "trk0".into();
    app.album_tracks_cache
        .insert("alb1".into(), vec![other, queued]);
    let mut fixture = crate::app::tests::make_item("Fixture", "Audio");
    fixture.id = "fixture-track".into();
    app.album_tracks_cache
        .insert("album-1".into(), vec![fixture.clone()]);
    app.album_tracks_cache
        .insert("album-2".into(), vec![fixture]);
    script_grouped_album_landing(&http);

    let mut harness = TickHarness::new(app);
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    harness.model_mut().sync_mounted_surfaces();
    while harness.model().app.lib_rx.try_recv().is_ok() {}
    let id = ComponentId::Library;

    // The retained tree moves off the navigated album.
    harness.inject(key(Key::Down));
    harness.step();
    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("album-2"),
        "the retained tree had moved to the sibling album"
    );

    drive_grouped_album_navigation(&mut harness, "trk1", "Audio");
    harness.step();

    assert_eq!(
        music_selected_album_id(&harness, &id).as_deref(),
        Some("alb1"),
        "the tree re-anchors to the resolved album"
    );
    assert_eq!(
        harness.model().test_music_owner().track_selected_row(),
        Some(1),
        "the navigated track is selected in the Workspace track list"
    );
    assert!(
        harness.model().pending_music_track_selection.is_none(),
        "the deep selection is fully consumed"
    );
}

/// Task 6.4 (design D7): an apply-stage failure after album resolution — a
/// prepared grouped-catalog stack whose chain is broken — reports through the
/// existing library-error feedback and commits NOTHING: the active tab, nav
/// stack, saved Library position, and retained component selection are all
/// structurally unchanged.
#[test]
fn rejected_grouped_album_apply_flashes_and_leaves_every_committed_value_unchanged() {
    let (mut harness, id) = wide_music_harness();
    let before_tab = harness.model().app.tab;
    let before_stack = nav_stack_snapshot(&harness);
    let before_saved = harness.model().app.saved_library_position(0);
    let before_album = music_selected_album_id(&harness, &id);
    assert_eq!(
        before_album.as_deref(),
        Some("album-1"),
        "the retained tree starts on the fixture album"
    );

    // Album resolution already succeeded; the prepared stack fails to
    // reproduce the configured path (its album level hangs off a
    // nonexistent artist).
    let mut invalid = landed_grouped_album_stack("alb1");
    invalid[1].parent_id = "missing-artist".into();
    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), invalid);
    harness.step();

    assert!(
        harness.model().app.status.contains("Library error"),
        "the existing library-error feedback fires: {}",
        harness.model().app.status
    );
    assert_eq!(
        harness.model().app.status_severity,
        crate::app::notify_actions::ToastSeverity::Error
    );
    assert_eq!(
        harness.model().app.tab,
        before_tab,
        "the active tab is unchanged"
    );
    assert_eq!(
        nav_stack_snapshot(&harness),
        before_stack,
        "the nav stack is unchanged"
    );
    assert_eq!(
        harness.model().app.saved_library_position(0),
        before_saved,
        "the saved Library position is unchanged"
    );
    assert_eq!(
        music_selected_album_id(&harness, &id),
        before_album,
        "the retained component selection is unchanged"
    );
}

/// A configured album-terminating shape whose first level is not the grouped
/// Music level cannot provide a Music owner. Landing validation must reject it
/// before the active tab, browse state, saved position, or retained owner can
/// be changed.
#[test]
fn rejected_non_grouped_music_shape_flashes_and_leaves_every_committed_value_unchanged() {
    let (mut harness, id) = wide_music_harness();
    let before_tab = harness.model().app.tab;
    let before_stack = nav_stack_snapshot(&harness);
    let before_saved = harness.model().app.saved_library_position(0);
    let before_album = music_selected_album_id(&harness, &id);
    harness.model_mut().app.music_levels = vec!["genre".into(), "album".into()];

    harness
        .model_mut()
        .on_recursive_album_activated("lib-music".into(), landed_grouped_album_stack("alb1"));
    harness.step();

    assert!(
        harness.model().app.status.contains("Library error"),
        "the existing library-error feedback fires: {}",
        harness.model().app.status
    );
    assert_eq!(
        harness.model().app.status_severity,
        crate::app::notify_actions::ToastSeverity::Error
    );
    assert_eq!(
        harness.model().app.tab,
        before_tab,
        "the active tab is unchanged"
    );
    assert_eq!(
        nav_stack_snapshot(&harness),
        before_stack,
        "the nav stack is unchanged"
    );
    assert_eq!(
        harness.model().app.saved_library_position(0),
        before_saved,
        "the saved Library position is unchanged"
    );
    let key = crate::app::components::library_panel::LibraryKey::Service {
        service: mbv_core::config::ServiceKind::Emby,
        library_id: "lib-music".into(),
        kind: crate::app::components::library_panel::LibraryKind::Music,
    };
    let retained_album = music_panel(&harness)
        .owner(&key)
        .and_then(|owner| {
            owner
                .as_any()
                .downcast_ref::<crate::app::components::music_content::MusicContent>()
        })
        .and_then(|owner| owner.selected_item().map(|item| item.id.clone()));
    assert_eq!(
        retained_album, before_album,
        "the retained component selection is unchanged"
    );
}

