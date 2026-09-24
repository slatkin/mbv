//! Live-tick coverage for the Wide split owned by `LibraryPanel`.

use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;
use tuirealm::event::{
    Event, Key, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};

use crate::app::components::library_panel::LibraryPanel;
use crate::app::components::{ComponentId, Msg, ShellRequest, UserEvent};
use crate::app::render::make_movie_app;
use crate::app::tests::make_app_stub;
use crate::app::tests::tick_integration::harness::{StepOutcome, TickHarness};
use crate::app::{PanelFocus, PanelMode, SidebarId, TabSelection};
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event<UserEvent> {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn apply(harness: &mut TickHarness, outcome: StepOutcome) {
    let (mut music, mut tv) = (false, false);
    for message in outcome.messages {
        harness
            .model_mut()
            .handle_terminal_message(message, &mut music, &mut tv);
    }
    harness.model_mut().sync_mounted_surfaces();
}

fn draw(harness: &mut TickHarness) {
    let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
    terminal
        .draw(|frame| harness.model_mut().draw_frame(frame, false, false))
        .unwrap();
    harness.model_mut().sync_mounted_surfaces();
}

fn split(harness: &TickHarness) -> (Rect, u16, u16) {
    harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_split())
        .map(|(gap, origin, width, _)| (gap, origin, width))
        .expect("LibraryPanel paints the Wide split")
}

fn drag_once(harness: &mut TickHarness) {
    let (gap, origin, content_width) = split(harness);
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    apply(harness, outcome);
    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x - 6,
        gap.y,
    ));
    let outcome = harness.step();
    let expected = crate::app::state::list_pane_width::normalize_list_pane_width(
        Some(origin - (gap.x - 6)),
        content_width,
    )
    .expect("drag resolves a valid width");
    assert!(outcome.raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::ResizeListPaneLive(width)) if *width == expected
    )));
    apply(harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, Some(expected));
}

fn feeds_harness() -> TickHarness {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    app.terminal_width = 120;
    app.terminal_height = 30;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    harness
}

#[test]
fn empty_feeds_panel_owns_split_drag() {
    let mut harness = feeds_harness();
    draw(&mut harness);
    assert!(harness
        .model()
        .mouse_subscribed
        .contains(&ComponentId::Library));
    drag_once(&mut harness);
}

fn populated_feeds_harness(loading: bool) -> TickHarness {
    let mut harness = feeds_harness();
    let app = &mut harness.model_mut().app;
    app.feed_tab.subscriptions = vec![FeedSubscription {
        name: "Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    app.feed_tab.entries = vec![vec![FeedEntry {
        guid: "one".into(),
        title: "One".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: Some("feed".into()),
        position_ticks: 0,
        played: false,
    }]];
    app.feed_tab.loading = loading;
    app.feed_tab.rebuild_all_entries();
    harness.model_mut().sync_mounted_surfaces();
    harness
}

#[test]
fn filtered_feeds_panel_keeps_owning_split_drag() {
    let mut harness = populated_feeds_harness(false);
    draw(&mut harness);
    harness.inject(Event::Keyboard(KeyEvent {
        code: Key::Char('w'),
        modifiers: KeyModifiers::NONE,
    }));
    let _ = harness.step();
    draw(&mut harness);
    drag_once(&mut harness);
}

#[test]
fn loading_feeds_panel_owns_split_drag() {
    let mut harness = populated_feeds_harness(true);
    draw(&mut harness);
    drag_once(&mut harness);
}

#[test]
fn split_drag_is_live_only_and_persists_once_on_release() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    std::fs::write(crate::config::prefs_path(), r#"{"sentinel":true}"#)
        .expect("write initial prefs");
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    let (gap, origin, content_width) = split(&harness);
    let before = std::fs::read(crate::config::prefs_path()).expect("read initial prefs");

    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    assert!(!harness
        .step()
        .raw_messages
        .iter()
        .any(|m| matches!(m, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));

    let first_column = gap.x - 2;
    let first_expected = crate::app::state::list_pane_width::normalize_list_pane_width(
        Some(origin - first_column),
        content_width,
    )
    .unwrap();
    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        first_column,
        gap.y,
    ));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|m| matches!(
        m,
        Msg::Shell(ShellRequest::ResizeListPaneLive(width)) if *width == first_expected
    )));
    apply(&mut harness, outcome);
    assert_eq!(
        std::fs::read(crate::config::prefs_path()).expect("read live prefs"),
        before,
        "the first live drag position must not write preferences"
    );

    let final_column = gap.x - 5;
    let expected = crate::app::state::list_pane_width::normalize_list_pane_width(
        Some(origin - final_column),
        content_width,
    )
    .unwrap();
    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        final_column,
        gap.y,
    ));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|m| matches!(
        m,
        Msg::Shell(ShellRequest::ResizeListPaneLive(width)) if *width == expected
    )));
    apply(&mut harness, outcome);
    assert_eq!(
        std::fs::read(crate::config::prefs_path()).expect("read live prefs"),
        before,
        "the final live drag position must not write preferences"
    );

    harness.inject(mouse(
        MouseEventKind::Up(MouseButton::Left),
        final_column,
        gap.y,
    ));
    let outcome = harness.step();
    assert!(outcome.raw_messages.iter().any(|m| matches!(
        m,
        Msg::Shell(ShellRequest::ResizeListPaneEnd(width)) if *width == expected
    )));
    apply(&mut harness, outcome);
    assert_eq!(harness.model().app.list_pane_width, Some(expected));
    let after_release = std::fs::read(crate::config::prefs_path()).expect("read end prefs");
    assert_ne!(
        after_release, before,
        "release must be the only preference write"
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&after_release).expect("parse saved prefs")
            ["list_pane_width"]
            .as_u64(),
        Some(expected as u64),
        "release persists the final resolved width"
    );

    // A second gesture with no motion must not rewrite the already-persisted
    // split or emit a persistence request.
    draw(&mut harness);
    let (gap, _, _) = split(&harness);
    let before_click = std::fs::read(crate::config::prefs_path()).expect("read saved prefs");
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    assert!(!harness
        .step()
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::ResizeListPaneEnd(_)))));
    harness.inject(mouse(MouseEventKind::Up(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    assert!(!outcome
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::ResizeListPaneEnd(_)))));
    apply(&mut harness, outcome);
    assert_eq!(
        std::fs::read(crate::config::prefs_path()).expect("read click prefs"),
        before_click,
        "a press-release without motion must not write preferences"
    );
}

#[test]
fn split_drag_ignores_the_selector_band() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);

    let (gap, origin, content_width) = split(&harness);
    let band_bottom = harness
        .model()
        .application
        .get_component(&ComponentId::Library)
        .and_then(|component| component.as_any().downcast_ref::<LibraryPanel>())
        .and_then(|panel| panel.test_wide_geometry())
        .map(|geometry| geometry.hero.y)
        .expect("the Wide skeleton paints a content band below the Selector band");
    assert_eq!(gap.y, band_bottom);
    assert!(band_bottom >= 2);
    assert!(gap.x >= 5);

    // Both the pill and spacer rows are in the panel's painted area, but not
    // in the split gap. A drag starting in either row must not arm the split.
    for row in [band_bottom - 2, band_bottom - 1] {
        harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, row));
        assert!(!harness
            .step()
            .raw_messages
            .iter()
            .any(|message| matches!(message, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));
        harness.inject(mouse(
            MouseEventKind::Drag(MouseButton::Left),
            gap.x - 5,
            row,
        ));
        assert!(!harness
            .step()
            .raw_messages
            .iter()
            .any(|message| matches!(message, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));
        harness.inject(mouse(MouseEventKind::Up(MouseButton::Left), gap.x - 5, row));
        assert!(!harness
            .step()
            .raw_messages
            .iter()
            .any(|message| matches!(message, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));
    }

    // A drag starting in the content band still owns the split and resolves
    // its live width through the mounted panel's subscription.
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    assert!(!harness
        .step()
        .raw_messages
        .iter()
        .any(|message| matches!(message, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));
    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x - 5,
        gap.y,
    ));
    let expected = crate::app::state::list_pane_width::normalize_list_pane_width(
        Some(origin - (gap.x - 5)),
        content_width,
    )
    .expect("content-band drag resolves a valid width");
    assert!(harness.step().raw_messages.iter().any(|message| matches!(
        message,
        Msg::Shell(ShellRequest::ResizeListPaneLive(width)) if *width == expected
    )));
}

#[test]
fn split_drag_is_suppressed_and_reset_when_panel_loses_eligibility() {
    let mut app = make_movie_app();
    app.panel_mode = PanelMode::LibraryOnly;
    app.panel_focus = PanelFocus::Library;
    let mut harness = TickHarness::new(app);
    harness.model_mut().sync_mounted_surfaces();
    draw(&mut harness);
    let (gap, _, _) = split(&harness);
    harness.inject(mouse(MouseEventKind::Down(MouseButton::Left), gap.x, gap.y));
    let outcome = harness.step();
    apply(&mut harness, outcome);
    harness.model_mut().mount_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    for kind in [
        MouseEventKind::Drag(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        harness.inject(mouse(kind, gap.x + 20, gap.y));
        assert!(!harness
            .step()
            .raw_messages
            .iter()
            .any(|m| matches!(m, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));
    }
    harness.model_mut().dismiss_sidebar(SidebarId::Search);
    harness.model_mut().sync_mounted_surfaces();
    harness.inject(mouse(
        MouseEventKind::Drag(MouseButton::Left),
        gap.x + 20,
        gap.y,
    ));
    assert!(!harness
        .step()
        .raw_messages
        .iter()
        .any(|m| matches!(m, Msg::Shell(ShellRequest::ResizeListPaneLive(_)))));
}
