//! Live-tick coverage for the Wide hero split boundary
//! (add-mouse-wide-split-resize rows 2.1/2.2). Proves the mounted gap
//! component arms only where a split is actually painted, is visually inert
//! on the gap it owns, applies the live width through the shell dispatch arm,
//! and leaves the adjacent pane gestures to their own components.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use crate::app::components::{
    ComponentId, Msg, MusicWorkspaceComponent, ShellRequest, UserEvent,
};
use crate::app::render::make_movie_app;
use crate::app::tests::make_app_stub;
use crate::app::tests_tick_harness::{StepOutcome, TickHarness};
use crate::app::{PanelFocus, PanelMode, SidebarId, TabSelection};
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;

fn feed_entry(guid: &str, title: &str) -> FeedEntry {
    FeedEntry {
        guid: guid.into(),
        title: title.into(),
        enclosure_url: Some(format!("https://example.test/{guid}.mp3")),
        link: None,
        mime_type: Some("audio/mpeg".into()),
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("https://example.test/feed".into()),
        position_ticks: 0,
        played: false,
    }
}

fn apply_outcome(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music_resize, mut tv_resize) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music_resize, &mut tv_resize);
    }
}

/// The exact live width a gap drag at `column` resolves to, derived from the
/// active surface's own reported content area rather than a hardcoded
/// coordinate.
fn resolved_width(column: u16, content_area: Rect) -> Option<u16> {
    crate::app::list_pane_width::normalize_list_pane_width(
        Some(column.saturating_sub(content_area.x)),
        content_area.width,
    )
}

/// The live-width message, if the boundary owner emitted one for this step.
fn live_width(outcome: &StepOutcome) -> Option<u16> {
    outcome.raw_messages.iter().find_map(|msg| match msg {
        Msg::Shell(ShellRequest::ResizeListPaneLive(width)) => Some(*width),
        _ => None,
    })
}

/// The non-terminal messages the mouse fold actually delivered: one gap
/// gesture must deliver the boundary owner's claim and nothing else.
fn delivered_claims(outcome: &StepOutcome) -> Vec<&Msg> {
    outcome
        .messages
        .iter()
        .filter(|msg| !matches!(msg, Msg::TerminalEvent(_)))
        .collect()
}

/// No pane destination may claim the boundary's gap gesture (design.md:
/// "the panes' hit geometry excludes the gap").
fn assert_no_pane_gesture(outcome: &StepOutcome, what: &str) {
    assert!(
        outcome.raw_messages.iter().all(|msg| !matches!(
            msg,
            Msg::Shell(ShellRequest::BrowserRowClick { .. })
                | Msg::Shell(ShellRequest::MusicAlbumCursor { .. })
                | Msg::Shell(ShellRequest::QueueRowClick { .. })
                | Msg::Shell(ShellRequest::FeedsRowClick)
        )),
        "{what}: no pane component may claim the gap gesture"
    );
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

/// The split geometry the mounted `LibraryPanel` retained from its last
/// painted Wide frame: (gap, pane-origin x, content width, list-pane width)
/// — the facts the old `WideHeroBoundaryComponent::sync` carried, now on the
/// panel's own painted skeleton (task 5.9/6.1).
fn panel_split(harness: &TickHarness) -> Option<(Rect, u16, u16, u16)> {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| {
            component
                .as_any()
                .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        })
        .and_then(|panel| panel.test_split())
}

/// Drag the panel's painted split gap once and return the live width applied
/// to the session override (task 7.3: the registered destination's panel owns
/// the split gesture).
fn drag_panel_gap_live(harness: &mut TickHarness) -> u16 {
    let (gap, pane_origin_x, content_width, _width) =
        panel_split(harness).expect("the panel painted a Wide split");
    let content_area = Rect {
        x: pane_origin_x,
        width: content_width,
        ..Rect::default()
    };
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    apply_outcome(harness, outcome);
    harness.inject(mouse(MouseEventKind::Drag(MouseButton::Left), gap.x + 6, gap.y));
    let outcome = harness.step();
    let expected = resolved_width(gap.x + 6, content_area).expect("a valid width");
    assert_eq!(live_width(&outcome), Some(expected));
    apply_outcome(harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, Some(expected));
    expected
}

fn draw_frame(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

/// A representative wide surface's gap is unchanged by the boundary painting
/// A representative wide surface's gap is unchanged by the boundary painting
/// it: with the boundary mounted and eligible, the frame is byte-identical to
/// the same frame with the boundary unmounted (the pre-change baseline).
/// Task 6.1 moved the Movies surface's split to the `LibraryPanel` (which
/// paints the whole skeleton and has no separate gap painter), so the
/// boundary-inertness property runs on grouped Music — the standing
/// still-mounted destination the boundary serves.
#[test]
fn wide_hero_boundary_gap_is_visually_inert_on_a_wide_music_surface() {
    let mut app = crate::app::render::make_music_group_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    // Steady state: one throwaway draw publishes `root_frame`, the sync it
    // gates mounts the chrome panels (tasks 2.1-2.2), and both recorded
    // frames below are then drawn from the same mounted set.
    let _priming = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let with_boundary = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness
        .model()
        .application
        .mounted(&ComponentId::WideHeroBoundary));
    assert!(
        harness.model().wide_hero_boundary_mouse_eligible(),
        "a painted wide Music split must arm the boundary"
    );
    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Music surface paints a split");
    assert!(gap.width > 0 && gap.height > 0);

    let preserved = with_boundary.backend().buffer().clone();
    harness
        .model_mut()
        .application
        .umount(&ComponentId::WideHeroBoundary)
        .expect("umount boundary");
    harness.model_mut().sync_mounted_surfaces();
    let without_boundary = draw_frame(&mut harness);

    assert_eq!(
        *without_boundary.backend().buffer(),
        preserved,
        "the boundary must repaint the gap with the backdrop it already showed"
    );
}

/// An empty Feeds surface still paints the panel's shared Wide split (task
/// 7.2: the skeleton paints the hero pane regardless of the filtered list),
/// so the panel — not the old shell boundary — owns the split and drags it
/// live. The shell boundary stays inert for the registered surface so the one
/// painted gap never has two gesture owners.
#[test]
fn panel_owns_and_drags_an_empty_feeds_split() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    // Steady state: throwaway draws publish `root_frame` and mount the chrome
    // panels before the geometry is read.
    let _priming = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    let _steady = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let (gap, _origin, _width_area, _width) =
        panel_split(&harness).expect("the empty Feeds surface still paints the panel's Wide split");
    assert!(gap.width > 0 && gap.height > 0);
    assert!(
        !harness.model().wide_hero_boundary_mouse_eligible(),
        "the shell boundary is inert for the registered Feeds surface"
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_none());
    assert!(
        harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::Library),
        "the panel that painted the split is mouse-eligible"
    );
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::WideHeroBoundary),
        "the inert shell boundary must not be mouse-subscribed"
    );

    drag_panel_gap_live(&mut harness);
}

/// A Feeds surface whose group/watched filter empties the visible list still
/// paints the panel's Wide split, so the panel keeps owning the drag with the
/// same painted geometry. This replaces the pre-registration
/// `feeds_has_visible_entries` gate, which disarmed a split the skeleton
/// still painted (task 7.3, recorded review obligation).
#[test]
fn panel_owns_a_filtered_empty_feeds_split() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.feed_tab.subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    app.feed_tab.entries = vec![vec![feed_entry("one", "One"), feed_entry("two", "Two")]];
    app.feed_tab.rebuild_all_entries();
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        panel_split(&harness).is_some(),
        "an unfiltered Feeds surface paints the panel split"
    );

    // Cycle the watched filter `All -> Played`; every entry is unplayed, so
    // the visible list is empty — the split is still painted and owned.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        panel_split(&harness).is_some(),
        "a filtered-to-empty Feeds surface still paints the panel split"
    );
    assert!(
        !harness.model().wide_hero_boundary_mouse_eligible(),
        "the shell boundary stays inert for the registered surface"
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_none());
    drag_panel_gap_live(&mut harness);
}

/// The boundary owns only the gap columns: a click on the pane column
/// adjacent to the gap still resolves through the pane's own component, while
/// a drag from the gap column emits the live width and updates the session
/// override through the shell's exhaustive dispatch arm.
#[test]
fn wide_hero_boundary_owns_the_gap_and_adjacent_panes_keep_their_gestures() {
    let mut app = crate::app::render::make_music_group_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let music_id = harness
        .model()
        .music_workspace_id
        .clone()
        .expect("grouped Music workspace mounted");

    draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness
        .model()
        .application
        .mounted(&ComponentId::WideHeroBoundary));
    assert!(harness.model().wide_hero_boundary_mouse_eligible());
    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Music surface paints a split");
    let browser_area: Rect = harness
        .model()
        .application
        .get_component(&music_id)
        .and_then(|component| component.as_any().downcast_ref::<MusicWorkspaceComponent>())
        .map(|music| music.layout().wide_music_browser_area)
        .expect("Music layout");
    assert!(
        browser_area.right() <= gap.x,
        "the browser pane's painted content ends at or before the gap"
    );

    // A pane gesture on the pane column nearest the gap resolves to Music and
    // never to the boundary.
    harness.inject(mouse(
        MouseEventKind::Down(MouseButton::Left),
        browser_area.right() - 1,
        browser_area.y + 1,
    ));
    let outcome = harness.step();
    assert!(
        outcome
            .raw_messages
            .iter()
            .any(|msg| matches!(msg, Msg::Shell(ShellRequest::MusicAlbumCursor { .. }))),
        "the pane-adjacent click must resolve through the Music component"
    );
    assert!(outcome.raw_messages.iter().all(|msg| {
        !matches!(msg, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))
    }));
    apply_outcome(&mut harness, outcome);

    // Arming inside the gap emits nothing on press.
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().all(|msg| {
        !matches!(msg, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))
    }));
    apply_outcome(&mut harness, outcome);

    // Dragging five columns right emits the live width, and the shell's
    // dispatch arm stores it as the session override (clamped against the
    // active content width).
    let content_area = harness
        .model()
        .wide_hero_boundary_content_area()
        .expect("wide Music content area");
    let expected = crate::app::list_pane_width::normalize_list_pane_width(
        Some(gap.x + 5 - content_area.x),
        content_area.width,
    )
    .expect("a valid width");
    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x + 5,
        gap.y,
    ));
    let outcome = harness.step();
    let live_width = outcome.raw_messages.iter().find_map(|msg| match msg {
        Msg::Shell(ShellRequest::ResizeListPaneLive(width)) => Some(*width),
        _ => None,
    });
    assert_eq!(
        live_width,
        Some(expected),
        "the boundary owner must emit the resolved live width"
    );
    assert!(
        outcome.raw_messages.iter().all(|msg| !matches!(
            msg,
            Msg::Shell(ShellRequest::MusicAlbumCursor { .. })
        )),
        "the gap drag must not also resolve through the pane component"
    );
    apply_outcome(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, Some(expected));
}

/// The split is session-only: a full press/drag/release on the gap moves the
/// live split but writes no preference or config value, and the generic
/// preferences writer has no field to serialize it under. Task 6.1: the
/// Movies surface's split gesture lives on the mounted `LibraryPanel`, so
/// the drag is driven through the panel's own painted gap geometry.
#[test]
fn wide_split_drag_never_writes_preferences() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let (gap, pane_origin_x, content_width, _width) = panel_split(&harness)
        .expect("the wide Movies surface paints a panel split");
    let content_area = Rect {
        x: pane_origin_x,
        width: content_width,
        ..Rect::default()
    };
    let prefs_path = crate::config::prefs_path();
    let before = std::fs::read(&prefs_path).ok();

    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    apply_outcome(&mut harness, outcome);

    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x + 6,
        gap.y,
    ));
    let outcome = harness.step();
    assert!(
        outcome.raw_messages.iter().any(|msg| matches!(
            msg,
            Msg::Shell(ShellRequest::ResizeListPaneLive(_))
        )),
        "the drag must reach the live resize path"
    );
    apply_outcome(&mut harness, outcome);

    harness.inject(mouse(
        MouseEventKind::Up(MouseButton::Left),
        gap.x + 6,
        gap.y,
    ));
    let outcome = harness.step();
    apply_outcome(&mut harness, outcome);

    let expected = crate::app::list_pane_width::normalize_list_pane_width(
        Some(gap.x + 6 - content_area.x),
        content_area.width,
    );
    assert_eq!(
        harness.model().app.list_pane_width,
        expected,
        "the drag applied the live session override"
    );
    assert!(harness.model().app.list_pane_width.is_some());
    assert_eq!(
        std::fs::read(&prefs_path).ok(),
        before,
        "a split drag must not write preferences or config"
    );

    // Even an unrelated preference save has no field to serialize the split
    // under: it is not part of the persisted schema.
    harness.model().app.save_prefs();
    let saved = std::fs::read_to_string(&prefs_path).expect("prefs written");
    let saved: serde_json::Value = serde_json::from_str(&saved).unwrap();
    assert!(
        saved.get("list_pane_width").is_none(),
        "the session split must have no persisted preference key"
    );
}

/// A full press-drag-release on the gap tracks the pointer at one-column
/// precision through the real `Application::tick()` path: each drag step
/// delivers exactly the panel split's resolved live width (asserted
/// exactly, and with no pane claim), the session override follows, the
/// release is a live-only no-op, and the Movies owner's own cursor and
/// scroll never move.
#[test]
fn tick_wide_hero_boundary_press_drag_release_tracks_exact_live_widths() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let (gap, pane_origin_x, content_width, _width) = panel_split(&harness)
        .expect("the wide Movies surface paints a panel split");
    let content_area = Rect {
        x: pane_origin_x,
        width: content_width,
        ..Rect::default()
    };
    let browser_state = |harness: &mut TickHarness| {
        let (_, key, _) = harness
            .model()
            .active_migrated_browser_owner()
            .expect("the Movies owner has migrated");
        harness
            .model()
            .application
            .get_component(&ComponentId::Library)
            .and_then(|component| component.as_any().downcast_ref::<crate::app::components::library_panel::LibraryPanel>())
            .and_then(|panel| panel.owner(&key))
            .and_then(|owner| owner.as_any().downcast_ref::<crate::app::components::browser_content::BrowserContent>())
            .map(|owner| (owner.cursor(), owner.scroll()))
            .expect("browser owner installed")
    };
    let before = browser_state(&mut harness);

    // Press: arms the gesture but resizes nothing.
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    assert_eq!(live_width(&outcome), None, "a press must not resize");
    assert!(
        delivered_claims(&outcome).is_empty(),
        "a press inside the gap claims nothing"
    );
    apply_outcome(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, None);

    // Drag out, further out, then back: each step delivers exactly the
    // one-column resolved live width and nothing else.
    for column in [gap.x + 1, gap.x + 5, gap.x - 2] {
        harness.inject(mouse(MouseEventKind::Drag(MouseButton::Left), column, gap.y));
        let outcome = harness.step();
        let expected = resolved_width(column, content_area).expect("a valid width");
        assert_eq!(
            live_width(&outcome),
            Some(expected),
            "drag to column {column} must deliver the exact resolved live width"
        );
        assert_eq!(
            delivered_claims(&outcome).len(),
            1,
            "one gap drag delivers only the boundary owner's message"
        );
        assert_no_pane_gesture(&outcome, "gap drag");
        apply_outcome(&mut harness, outcome);
        assert_eq!(harness.model().app.list_pane_width, Some(expected));
    }
    let last = resolved_width(gap.x - 2, content_area).expect("a valid width");

    // Release: live-only, so `DragEnd` emits nothing and nothing is written.
    harness.inject(mouse(MouseEventKind::Up(MouseButton::Left), gap.x - 2, gap.y));
    let outcome = harness.step();
    assert_eq!(live_width(&outcome), None, "release must not resize");
    assert!(
        delivered_claims(&outcome).is_empty(),
        "release claims nothing"
    );
    assert_no_pane_gesture(&outcome, "gap release");
    apply_outcome(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, Some(last));

    assert_eq!(
        browser_state(&mut harness),
        before,
        "the Movies pane must not handle the gap gesture"
    );
}

/// Overlay arbitration (mouse-input spec: "Overlay arbitration suppresses the
/// boundary"): while a panel-covering overlay is mounted, a
/// press-drag-release across the gap columns delivers no gesture to the
/// panel's split gesture and leaves the split unchanged. Outcomes are
/// deliberately not applied so the overlay stays mounted for every pointer
/// event.
#[test]
fn tick_wide_hero_boundary_gap_drag_is_suppressed_while_a_panel_overlay_is_mounted() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    let (gap, _origin, _content_width, _width) = panel_split(&harness)
        .expect("the wide Movies surface paints a panel split");
    assert!(harness.model().panel_mouse_eligible());
    assert!(harness.model().mouse_subscribed.contains(&ComponentId::Library));

    harness.model_mut().mount_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness.model().panel_mouse_eligible(),
        "a mounted overlay arbitrates the panel surfaces"
    );
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::Library),
        "the obscured panel must not be mouse-subscribed"
    );

    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        harness.inject(mouse(kind, gap.x + 8, gap.y));
        let outcome = harness.step();
        assert_eq!(
            live_width(&outcome),
            None,
            "the obscured boundary must receive no gesture"
        );
        assert_no_pane_gesture(&outcome, "overlay suppression");
    }
    assert_eq!(
        harness.model().app.list_pane_width,
        None,
        "the split must not change while an overlay arbitrates"
    );
}

/// Losing eligibility mid-drag (an overlay mount) resets the panel's split
/// gesture state before the next delivery, so no stale width can be emitted
/// after eligibility ends — not while suppressed, and not once eligibility
/// returns: a drag without a fresh press is inert. A new press arms again.
#[test]
fn tick_wide_hero_boundary_mid_drag_eligibility_loss_emits_no_stale_width() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    let (gap, pane_origin_x, content_width, _width) = panel_split(&harness)
        .expect("the wide Movies surface paints a panel split");
    let content_area = Rect {
        x: pane_origin_x,
        width: content_width,
        ..Rect::default()
    };

    // Arm a drag on the gap.
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    assert_eq!(live_width(&outcome), None);
    apply_outcome(&mut harness, outcome);

    // An overlay mounts mid-drag: eligibility is lost and the gesture state
    // is reset before the drag is delivered.
    harness.model_mut().mount_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().panel_mouse_eligible());

    harness.inject(mouse(MouseEventKind::Drag(MouseButton::Left), gap.x + 30, gap.y));
    let outcome = harness.step();
    assert_eq!(
        live_width(&outcome),
        None,
        "no stale width may be emitted after eligibility ends"
    );
    apply_outcome(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, None);

    // Dismissing the overlay restores eligibility, but the pre-overlay drag
    // anchor is gone: a drag without a fresh press emits nothing.
    harness.model_mut().dismiss_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    assert!(harness.model().panel_mouse_eligible());
    harness.inject(mouse(MouseEventKind::Drag(MouseButton::Left), gap.x + 30, gap.y));
    let outcome = harness.step();
    assert_eq!(
        live_width(&outcome),
        None,
        "the reset gesture must not revive when eligibility returns"
    );
    apply_outcome(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, None);

    // A fresh press arms the gesture again.
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    apply_outcome(&mut harness, outcome);
    harness.inject(mouse(MouseEventKind::Drag(MouseButton::Left), gap.x + 3, gap.y));
    let outcome = harness.step();
    let expected = resolved_width(gap.x + 3, content_area).expect("a valid width");
    assert_eq!(live_width(&outcome), Some(expected));
    apply_outcome(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, Some(expected));
}

/// A Feeds surface that is still loading still paints the panel's Wide split
/// (the skeleton paints it whenever the breakpoint fits), so the panel owns
/// and drags the split instead of the old shell gate disarming it.
#[test]
fn panel_owns_and_drags_a_loading_feeds_split() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.feed_tab.subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    app.feed_tab.loading = true;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        panel_split(&harness).is_some(),
        "a loading Feeds surface paints the panel split"
    );
    assert!(
        !harness.model().wide_hero_boundary_mouse_eligible(),
        "the shell boundary stays inert for the registered surface"
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_none());
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::WideHeroBoundary)
    );
    drag_panel_gap_live(&mut harness);
}
