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
use crate::app::{PanelFocus, PanelMode, TabSelection};
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
