use super::*;
use ratatui::layout::Position;
// ── Task 2.3: the tree is the one Grouped Music browser owner and painter ──

pub(in super::super) fn music_panel(
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
pub(in super::super) fn draw_music_frame(harness: &mut TickHarness) {
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
pub(in super::super) fn mounted_music_app_at(
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

pub(in super::super) fn music_panel_mut(
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
            let area = music_panel(&harness)
                .test_list_rect()
                .expect("painted album content");
            let point = (area.y..area.bottom())
                .flat_map(|y| (area.x..area.right()).map(move |x| Position::new(x, y)))
                .find(|point| music.browser.resolve_current_point(*point) == Some(&node))
                .expect("painted album row");
            (point.x, point.y)
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
        Msg::Shell(shell_boxed) => match shell_boxed.as_ref() {
            ShellRequest::MusicRowContextMenu(
                crate::app::state::types::context_menu::ContextMenuTargets::Emby(items),
                _,
            ) => Some(items),
            _ => None,
        },
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
        Msg::Shell(Box::new(ShellRequest::ClearMultiSelection(wrong_origin))),
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
        Msg::Shell(Box::new(ShellRequest::ClearMultiSelection(origin))),
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
pub(in super::super) fn tick_key(harness: &mut TickHarness, code: Key) {
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
