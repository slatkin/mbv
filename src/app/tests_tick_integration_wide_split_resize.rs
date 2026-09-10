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
    BrowserComponent, ComponentId, Msg, MusicWorkspaceComponent, ShellRequest, UserEvent,
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

fn draw_frame(harness: &mut TickHarness) -> Terminal<TestBackend> {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|f| harness.model_mut().draw_frame(f, false, false))
        .unwrap();
    terminal
}

/// A representative wide surface's gap is unchanged by the boundary painting
/// it: with the boundary mounted and eligible, the frame is byte-identical to
/// the same frame with the boundary unmounted (the pre-change baseline).
#[test]
fn wide_hero_boundary_gap_is_visually_inert_on_a_wide_movies_surface() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let with_boundary = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(harness
        .model()
        .application
        .mounted(&ComponentId::WideHeroBoundary));
    assert!(
        harness.model().wide_hero_boundary_mouse_eligible(),
        "a painted wide Movies split must arm the boundary"
    );
    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Movies surface paints a split");
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

/// A surface that fits the wide breakpoint but paints no split (empty/loading
/// Feeds) arms nothing, and its frame is likewise unchanged by the mounted
/// boundary.
#[test]
fn wide_hero_boundary_arms_nothing_on_an_empty_feeds_surface() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();

    let with_boundary = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        crate::app::render::wide_hero_fits(harness.model().app.layout.main.feeds_area),
        "the breakpoint must fit so emptiness is the only reason to disarm"
    );
    assert!(
        !harness.model().wide_hero_boundary_mouse_eligible(),
        "an empty Feeds surface must not arm the boundary"
    );
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::WideHeroBoundary),
        "a disarmed boundary must not be mouse-subscribed"
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_none());

    // The disarmed boundary delivers nothing: a press-drag-release across the
    // columns where the split would be must not resize (spec: "No resize
    // target where no split is painted").
    let would_be_gap = crate::app::render::wide_library_panes(
        harness.model().app.layout.main.feeds_area,
        0,
        0,
        None,
    )
    .expect("wide feeds")
    .browser_panel
    .right();
    let probe_row = harness.model().app.layout.main.feeds_area.y + 1;
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        harness.inject(mouse(kind, would_be_gap, probe_row));
        let outcome = harness.step();
        assert_eq!(
            live_width(&outcome),
            None,
            "an empty Feeds surface must never deliver a live width"
        );
        assert_no_pane_gesture(&outcome, "empty Feeds");
        apply_outcome(&mut harness, outcome);
    }
    assert_eq!(harness.model().app.list_pane_width, None);

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
        "no gap is painted, so the mounted boundary must be inert"
    );
}

/// A Feeds surface whose group/watched filter empties the visible list paints
/// no split — the painter's wide branch gates on the filtered
/// `visible_entries`, not the unfiltered `all_entries` — so the boundary must
/// not arm there either. The same populated surface does arm before the
/// filter empties it, proving the fix does not over-disarm.
#[test]
fn wide_hero_boundary_arms_nothing_when_a_feed_filter_empties_the_visible_list() {
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
        crate::app::render::wide_hero_fits(harness.model().app.layout.main.feeds_area),
        "the breakpoint must fit so the filter is the only reason to disarm"
    );
    assert!(
        harness.model().wide_hero_boundary_mouse_eligible(),
        "an unfiltered Feeds surface with entries paints the split and must arm"
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_some());

    // Cycle the watched filter `All -> Played`; every entry is unplayed, so
    // the painter's visible list is empty and its wide branch is skipped.
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    assert!(
        !harness.model().wide_hero_boundary_mouse_eligible(),
        "a filtered-to-empty Feeds surface must not arm the boundary"
    );
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::WideHeroBoundary),
        "a disarmed boundary must not be mouse-subscribed"
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_none());
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
/// preferences writer has no field to serialize it under.
#[test]
fn wide_split_drag_never_writes_preferences() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();

    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Movies surface paints a split");
    let content_area = harness
        .model()
        .wide_hero_boundary_content_area()
        .expect("wide Movies content area");
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
/// delivers exactly the boundary owner's resolved live width (asserted
/// exactly, and with no pane claim), the session override follows, the
/// release is a live-only no-op, and the Movies destination's own cursor and
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

    let browser_id = harness
        .model()
        .emby_browser_id
        .clone()
        .expect("the Movies browser is mounted");
    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Movies surface paints a split");
    let content_area = harness
        .model()
        .wide_hero_boundary_content_area()
        .expect("wide Movies content area");
    let browser_state = |harness: &mut TickHarness| {
        let browser = harness
            .model_mut()
            .application
            .get_component_mut(&browser_id)
            .expect("browser mounted")
            .as_any_mut()
            .downcast_mut::<BrowserComponent>()
            .expect("browser component type");
        (browser.cursor(), browser.scroll())
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
/// boundary owner and leaves the split unchanged. Outcomes are deliberately
/// not applied so the overlay stays mounted for every pointer event.
#[test]
fn tick_wide_hero_boundary_gap_drag_is_suppressed_while_a_panel_overlay_is_mounted() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    let _ = draw_frame(&mut harness);
    harness.model_mut().sync_mounted_surfaces();
    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Movies surface paints a split");
    assert!(harness.model().wide_hero_boundary_mouse_eligible());

    harness.model_mut().mount_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    assert!(
        !harness.model().panel_mouse_eligible(),
        "a mounted overlay arbitrates the panel surfaces"
    );
    assert!(!harness.model().wide_hero_boundary_mouse_eligible());
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::WideHeroBoundary),
        "the obscured boundary must not be mouse-subscribed"
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

/// Losing eligibility mid-drag (an overlay mount) resets the boundary's
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
    let gap = harness
        .model()
        .wide_hero_boundary_gap_rect()
        .expect("the wide Movies surface paints a split");
    let content_area = harness
        .model()
        .wide_hero_boundary_content_area()
        .expect("wide Movies content area");

    // Arm a drag on the gap.
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    assert_eq!(live_width(&outcome), None);
    apply_outcome(&mut harness, outcome);

    // An overlay mounts mid-drag: eligibility is lost and the gesture state
    // is reset before the drag is delivered.
    harness.model_mut().mount_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    assert!(!harness.model().wide_hero_boundary_mouse_eligible());

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
    assert!(harness.model().wide_hero_boundary_mouse_eligible());
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

/// A Feeds surface that fits the wide breakpoint but is still loading (no
/// visible entries) paints no split: the boundary arms nothing and delivers
/// no gesture, mirroring the empty state.
#[test]
fn wide_hero_boundary_arms_nothing_and_delivers_nothing_while_feeds_load() {
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

    let area = harness.model().app.layout.main.feeds_area;
    assert!(
        crate::app::render::wide_hero_fits(area),
        "the breakpoint must fit so loading is the only reason to disarm"
    );
    assert!(
        !harness.model().wide_hero_boundary_mouse_eligible(),
        "a loading Feeds surface must not arm the boundary"
    );
    assert!(
        !harness
            .model()
            .mouse_subscribed
            .contains(&ComponentId::WideHeroBoundary)
    );
    assert!(harness.model().wide_hero_boundary_gap_rect().is_none());

    let would_be_gap = crate::app::render::wide_library_panes(area, 0, 0, None)
        .expect("wide feeds")
        .browser_panel
        .right();
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        harness.inject(mouse(kind, would_be_gap, area.y + 1));
        let outcome = harness.step();
        assert_eq!(
            live_width(&outcome),
            None,
            "a loading Feeds surface must never deliver a live width"
        );
        assert_no_pane_gesture(&outcome, "loading Feeds");
        apply_outcome(&mut harness, outcome);
    }
    assert_eq!(harness.model().app.list_pane_width, None);
}
