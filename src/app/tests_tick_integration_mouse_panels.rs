use super::*;

#[test]
fn queue_boundary_unmounts_outside_the_two_panel_layout() {
    for mode in [PanelMode::QueueOnly, PanelMode::LibraryOnly] {
        let mut app = crate::app::render::make_queue_app(2);
        app.panel_mode = mode;
        let mut harness = TickHarness::new(app);
        assert!(
            harness
                .model()
                .application
                .mounted(&ComponentId::QueueBoundary),
            "the boundary starts mounted"
        );
        harness.model_mut().sync_mounted_surfaces();
        assert!(
            !harness
                .model()
                .application
                .mounted(&ComponentId::QueueBoundary),
            "{mode:?} unmounts the boundary"
        );
        harness.model_mut().app.panel_mode = PanelMode::Both;
        harness.model_mut().sync_mounted_surfaces();
        assert!(
            harness
                .model()
                .application
                .mounted(&ComponentId::QueueBoundary),
            "returning to the two-panel layout remounts the boundary"
        );
    }

    // Mini view derives its mode from `mini_view_focus`, never the stored
    // Both: a narrow terminal in the Library mini view has no boundary either.
    let mut mini = crate::app::render::make_queue_app(2);
    mini.terminal_width = 70;
    let mut harness = TickHarness::new(mini);
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness
        .model()
        .application
        .mounted(&ComponentId::QueueBoundary));
}

#[test]
fn tick_queue_boundary_drag_is_suppressed_by_blocking_overlay() {
    let mut app = crate::app::render::make_queue_app(2);
    app.panel_mode = PanelMode::Both;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
    let boundary = harness
        .model()
        .app
        .layout
        .root_frame
        .queue_boundary
        .expect("two-panel layout places the boundary in RootFrame");
    let width = harness.model().app.queue_column_width;
    harness.model_mut().app.pending_overlay = Some(OverlayRequest::Confirm(ConfirmModal {
        title: "Block resize?".into(),
        message: "overlay owns the pointer".into(),
        hint: "[Esc] Cancel".into(),
        on_confirm: ConfirmAction::ClearQueue,
    }));
    harness.model_mut().sync_mounted_surfaces();
    assert_eq!(harness.model().mouse_subscribed.len(), 1);

    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        harness.inject(Event::Mouse(MouseEvent {
            kind,
            column: boundary.x.saturating_add(20),
            row: boundary.y,
            modifiers: KeyModifiers::NONE,
        }));
        let outcome = harness.step();
        assert!(outcome.raw_messages.iter().all(|msg| {
            !matches!(
                msg,
                Msg::Queue(crate::app::components::QueueRequest::ResizeColumnLive(_))
            ) && !matches!(
                msg,
                Msg::Queue(crate::app::components::QueueRequest::ResizeColumnEnd(_))
            ) && !matches!(msg, Msg::Shell(ShellRequest::QueueRowClick { .. }))
                && !matches!(msg, Msg::Shell(ShellRequest::EmbyLibraryRowClick { .. }))
        }));
    }
    assert_eq!(harness.model().app.queue_column_width, width);
}

// --- Task 7.3 (breakpoint half): the Movies destination keeps its embedded
// canonical fixed-row control when a resize crosses the wide/narrow breakpoint.
// A click must resolve against the
// `row_geometry` the CURRENT frame painted, never the rect a prior frame left
// behind. The scroll half of this proof already lives in
// `media_list::tests::resolve_point::wide_resolves_against_a_scrolled_viewport`.

#[test]
fn browser_row_click_resolves_against_the_current_breakpoints_geometry_not_a_stale_one() {
    let mut app = crate::app::render::make_movie_app();
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    // Task 6.1: the Movies surface paints inside the mounted `LibraryPanel`,
    // whose retained skeleton geometry is the click-resolution truth.
    let browser_test_layout = |harness: &mut TickHarness| {
        harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
            .and_then(|panel| panel.test_list_rect())
            .expect("the panel painted a list slot")
    };

    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };

    // Wide breakpoint: paints via `WideMediaList` into the right-hand list
    // pane, well clear of column 0.
    let mut wide_terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    wide_terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    let wide_list_area = browser_test_layout(&mut harness);
    assert!(
        wide_list_area.width > 0 && wide_list_area.height > 0,
        "the wide breakpoint must have painted a non-empty list area"
    );

    // The panel's list rect starts at the painted row flow (the old
    // component's `left_area` included the pill row above it), so the blank
    // probe is the area below the fixture's two rows: it must not claim.
    harness.inject(click(
        wide_list_area.x,
        wide_list_area.bottom().saturating_sub(1),
    ));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::EmbyLibraryRowClick { .. }))),
        "a blank wide-list click must not claim without a resolved target"
    );
    apply_outcome(&mut harness, outcome);

    // Resize below the two-column threshold: the same fixed-row owner is
    // painted in the new list geometry.
    let mut narrow_terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    narrow_terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    let narrow_list_area = browser_test_layout(&mut harness);
    assert!(
        narrow_list_area.width > 0 && narrow_list_area.height > 0,
        "the narrow breakpoint must have painted a non-empty list area"
    );
    assert_ne!(
        wide_list_area, narrow_list_area,
        "the breakpoint change must have actually repainted different list geometry"
    );

    // A click on the OLD wide list's bottom row, now below the bottom edge
    // of the shorter narrow-painted list area (the wide browser pane sits
    // left of the hero and is vertically taller than the narrow list), must
    // not resolve to a row: if resolution consulted stale wide geometry
    // instead of the freshly painted narrow layout, this click would
    // incorrectly still land on a list row.
    let stale_probe_col = wide_list_area.x;
    let stale_probe_row = wide_list_area.bottom() - 1;
    assert!(
        stale_probe_row >= narrow_list_area.bottom()
            && stale_probe_col >= narrow_list_area.x
            && stale_probe_col < narrow_list_area.right(),
        "the old wide list must genuinely extend below the narrow list's new bottom edge at a \
         shared column for this click to be a meaningful stale-geometry probe"
    );
    harness.inject(click(stale_probe_col, stale_probe_row));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .all(|msg| !matches!(msg, Msg::Shell(ShellRequest::EmbyLibraryRowClick { .. }))),
        "a click at the old wide-list position must not resolve through stale wide geometry \
         after the narrow repaint"
    );
    apply_outcome(&mut harness, outcome);
}

/// A Music click delivered through `Application::tick` resolves against the
/// retained wide-list geometry rather than a parent hitmap.
#[test]
fn music_click_resolves_current_retained_geometry_through_application_tick() {
    let mut app = make_music_group_app();
    let mut second_album = make_item("Album 2", "MusicAlbum");
    second_album.id = "album-2".into();
    second_album.artist = "Alpha".into();
    app.libs[0].nav_stack[1].items.push(second_album);
    app.panel_focus = PanelFocus::Library;
    app.panel_mode = PanelMode::LibraryOnly;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let music_id = ComponentId::Library;
    let click = |column, row| {
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })
    };

    let mut wide_terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
    wide_terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    let wide_area = harness
        .model()
        .application
        .get_component(&music_id)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_list_rect())
        .expect("Music panel list geometry");
    assert!(wide_area.width > 0 && wide_area.height > 0);
    harness.inject(click(wide_area.x + 1, wide_area.y + 1));
    let outcome = harness.step();
    assert!(outcome
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))));
    apply_outcome(&mut harness, outcome);
}
