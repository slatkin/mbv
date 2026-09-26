use super::*;

/// Draw one frame through the real shell and settle both sync passes, so the
/// tests below resolve geometry from a completed frame.
fn settle_frame(harness: &mut TickHarness) {
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

/// Dispatch every surviving message of a step outcome through the shell and
/// re-run the production sync pass.
fn dispatch_outcome(
    harness: &mut TickHarness,
    outcome: crate::app::tests::tick_integration::harness::StepOutcome,
) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
    harness.model_mut().sync_mounted_surfaces();
}

/// A mounted Grouped Music tree click focuses Library, while a click on an
/// already-focused Queue leaves Queue focused. Context clicks also cross the
/// Music-specific focus-before-menu shell boundary; generic Queue context
/// requests remain generic.
#[test]
fn music_tree_click_and_context_menu_focus_library_but_queue_stays_generic() {
    let mut app = make_music_group_app();
    app.panel_mode = PanelMode::Both;
    app.panel_focus = PanelFocus::Queue;
    app.player_tab
        .set_items(vec![make_item("Queue Item", "Audio")], 0);
    let mut harness = TickHarness::new(app);
    settle_frame(&mut harness);

    let tree_point = {
        let music = harness.model().test_music_owner();
        let root = music
            .browser
            .visible_targets()
            .first()
            .expect("tree root")
            .clone();
        tree_node_point(&harness, &root)
    };
    let right_click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Right),
            column,
            row,
            modifiers: tuirealm::event::KeyModifiers::NONE,
        })
    };

    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(crate::app::components::library_panel::LibraryPanel::test_list_rect)
        .expect("painted Music list area");
    assert!(list_area.contains(ratatui::layout::Position::new(tree_point.0, tree_point.1)));
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));

    // Queue's ordinary context request remains the generic shell variant and
    // does not acquire Library focus merely because Music has a special arm.
    let queue_point = harness
        .model()
        .application
        .get_component(&ComponentId::Queue)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::QueueComponent>()
        })
        .and_then(crate::app::components::QueueComponent::selected_row_rect)
        .expect("painted queue row");
    harness.inject(right_click(queue_point.x, queue_point.y));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)  if matches!(shell_boxed.as_ref(), ShellRequest::RowContextMenu(
            crate::app::state::types::context_menu::ContextMenuTargets::Queue(_),
            Some((x, y)),
        ) if *x == queue_point.x && *y == queue_point.y))));
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Queue
    );

    harness.inject(left_click(tree_point.0, tree_point.1));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|message| {
            matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::LibraryPanelFocus))
                || matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::MusicArtistTracks { .. }))
        }),
        "tree click messages: {:?}",
        outcome.raw_messages
    );
    dispatch_outcome(&mut harness, outcome);
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Library
    );

    // The click's mutation invalidated the completed frame; re-paint so the
    // right-click resolves the latest geometry.
    draw_frame(&mut harness);
    harness.model_mut().app.panel_focus = PanelFocus::Queue;
    harness.inject(right_click(tree_point.0, tree_point.1));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
             if matches!(shell_boxed.as_ref(), ShellRequest::MusicRowContextMenu(_, Some((x, y))) if *x == tree_point.0 && *y == tree_point.1))));
    dispatch_outcome(&mut harness, outcome);
    assert_eq!(
        harness.model().app.effective_panel_focus(),
        PanelFocus::Library
    );
    harness.model_mut().sync_mounted_surfaces();
    let menu_id = ComponentId::Overlay(crate::app::components::OverlayId::ContextMenu);
    assert!(harness.model().application.mounted(&menu_id));
}

/// Grouped Music resolves its artist-root and album-leaf clicks through the
/// tree's completed hit map, then rejects that map as soon as a new frame is
/// configured. The mounted path is the real LibraryPanel/Application tick;
/// there is no shell-side album row geometry to fall back to.
#[test]
fn music_tree_mouse_resolves_current_rows_and_rejects_an_invalidated_frame() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Second Album", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    settle_frame(&mut harness);

    let list_area = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(crate::app::components::library_panel::LibraryPanel::test_list_rect)
        .expect("Music tree list geometry");
    let (root_point, album_point) = {
        let music = harness.model().test_music_owner();
        let point_for = |target: Option<&str>| {
            let node = music
                .browser
                .visible_targets()
                .into_iter()
                .find(|candidate| candidate.album_leaf_target() == target)
                .expect("painted tree node");
            tree_node_point(&harness, &node)
        };
        (point_for(None), point_for(Some("album-1")))
    };
    assert!(list_area.contains(ratatui::layout::Position::new(root_point.0, root_point.1)));
    assert!(list_area.contains(ratatui::layout::Position::new(album_point.0, album_point.1)));

    // The artist root is a painted, focusable tree row but not an album
    // target, so its click changes only the component-local selection.
    harness.inject(left_click(root_point.0, root_point.1));
    let outcome = harness.step();
    assert!(outcome
        .messages
        .iter()
        .all(|message| { !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumCursor { .. })) }));
    assert!(harness.model().test_music_owner().selected_is_artist());

    // The root click's mutation invalidated the completed frame; re-paint so
    // the leaf click resolves the latest geometry.
    draw_frame(&mut harness);
    harness.inject(left_click(album_point.0, album_point.1));
    let outcome = harness.step();
    assert!(outcome.messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ref shell_boxed)
     if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumCursor { target: 0, .. }))));
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_target()
            .and_then(|target| target.album_leaf_target()),
        Some("album-1")
    );

    // A content/geometry transition invalidates the completed tree result
    // before the replacement frame paints. The mounted parent still receives
    // the event through Application::tick, but MusicContent claims no stale
    // row and emits no cursor request.
    harness
        .model_mut()
        .test_music_owner_mut()
        .browser
        .invalidate_paint();
    harness.inject(left_click(album_point.0, album_point.1));
    let outcome = harness.step();
    assert!(outcome
        .messages
        .iter()
        .all(|message| { !matches!(message, Msg::Shell(ref shell_boxed) if matches!(shell_boxed.as_ref(), ShellRequest::MusicAlbumCursor { .. })) }));
    assert_eq!(
        harness
            .model()
            .test_music_owner()
            .browser
            .selected_target()
            .and_then(|target| target.album_leaf_target()),
        Some("album-1"),
        "the stale click did not mutate the tree selection"
    );
}
