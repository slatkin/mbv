use super::test_helpers::*;
use super::*;
use crate::app::render::arrangements::hero_left;
use crate::app::tests::make_app_stub;
use crate::app::TabSelection;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use ratatui::Terminal;

fn feed_app() -> App {
    let mut app = make_app_stub();
    app.tab = TabSelection::Feeds;
    app.feed_tab.subscriptions = vec![FeedSubscription {
        name: "Test Feed".into(),
        url: "https://example.test/feed".into(),
        kind: FeedKind::Audio,
    }];
    app.feed_tab.entries = vec![vec![FeedEntry {
        guid: "entry-1".into(),
        title: "Entry One".into(),
        enclosure_url: None,
        link: None,
        mime_type: None,
        duration_ticks: None,
        pub_date_secs: None,
        feed_kind: Some(FeedKind::Audio),
        feed_id: None,
        position_ticks: 0,
        played: false,
    }]];
    app.feed_tab.rebuild_all_entries();
    app
}

#[test]
fn wide_feeds_use_a_left_detail_and_right_entry_workspace() {
    let mut app = feed_app();
    let layout = render_view(&mut app, 140, 30);

    assert!(layout.hero_area.width < 140, "hero={:?}", layout.hero_area);
    assert!(
        layout.left_area.x > layout.hero_area.x,
        "hero={:?} list={:?}",
        layout.hero_area,
        layout.left_area
    );
    assert!(!layout.selector_tabs.is_empty());
}

#[test]
fn narrow_feeds_insert_selected_entry_detail_into_the_list_flow() {
    let mut app = feed_app();
    // Mini view defaults to queue-only, which doesn't render the Feeds tab at
    // all; opt into the library side so this test exercises the narrow
    // inline-detail flow it was written for.
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let layout = render_view(&mut app, 60, 20);

    assert!(layout.hero_area.height > 0);
    assert!(
        layout.hero_area.y >= layout.left_area.y,
        "hero={:?} list={:?}",
        layout.hero_area,
        layout.left_area
    );
}

#[test]
fn narrow_feeds_suppress_detail_when_the_viewport_is_too_short() {
    let mut app = feed_app();
    app.mini_view_focus = crate::app::PanelFocus::Library;
    let layout = render_view(&mut app, 60, 4);

    assert_eq!(layout.hero_area.height, 0);
}

fn render_feed_buffer(width: u16, height: u16, focused: bool) -> String {
    let mut app = feed_app();
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut layout = LayoutMain::default();
    terminal
        .draw(|f| {
            app.render_feeds(f, Rect::new(0, 0, width, height), focused, &mut layout);
        })
        .unwrap();
    buffer_to_string(&terminal)
}

#[test]
fn feeds_buffer_characterization_covers_default_focused_narrow_and_selected_states() {
    for (width, height, focused) in [
        (140, 30, false),
        (140, 30, true),
        (60, 20, true),
        (40, 20, false),
    ] {
        let output = render_feed_buffer(width, height, focused);
        assert!(
            output.contains("Test Feed"),
            "missing feed selector: {output:?}"
        );
        assert!(
            output.contains("Entry One"),
            "missing selected entry: {output:?}"
        );
    }
}

#[test]
fn feeds_pill_row_and_targets_are_characterized_end_to_end() {
    let assert_geometry = |terminal: &Terminal<TestBackend>, layout: &LayoutMain| {
        let panel = Rect::new(0, 0, 60, 20);
        let areas = hero_left::pill_bar_areas(panel);
        assert_surface_pills(
            terminal,
            layout,
            panel,
            1,
            ratatui::style::Color::Reset,
            &[0, 1],
            &["⌘", "All", "Test Feed"],
            0,
        );
        assert_eq!(layout.selector_tabs[0].0.y, areas.pills_area.y);
        assert_eq!(layout.left_area.y, areas.spacer_area.bottom() + 2);
        let buffer = terminal.backend().buffer();
        let filter_row = (0..buffer.area().width)
            .map(|x| buffer[(x, areas.spacer_area.bottom())].symbol())
            .collect::<String>();
        assert!(
            filter_row.contains("All"),
            "missing watched All filter: {filter_row:?}"
        );
        assert!(
            filter_row.contains("Played") && filter_row.contains("Unplayed"),
            "missing watched filters: {filter_row:?}"
        );
    };

    // No visible entries keeps the selector above the placeholder.
    let mut no_hero_app = feed_app();
    no_hero_app.feed_tab.entries.clear();
    no_hero_app.feed_tab.rebuild_all_entries();
    no_hero_app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut no_hero_layout = LayoutMain::default();
    let mut no_hero_terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    no_hero_terminal
        .draw(|f| no_hero_app.render_feeds(f, Rect::new(0, 0, 60, 20), true, &mut no_hero_layout))
        .unwrap();
    assert_geometry(&no_hero_terminal, &no_hero_layout);

    // Visible entries exercise the selector after the inline hero flow is admitted.
    let mut post_hero_app = feed_app();
    post_hero_app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut post_hero_layout = LayoutMain::default();
    let mut post_hero_terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    post_hero_terminal
        .draw(|f| {
            post_hero_app.render_feeds(f, Rect::new(0, 0, 60, 20), true, &mut post_hero_layout)
        })
        .unwrap();
    assert_geometry(&post_hero_terminal, &post_hero_layout);
    assert!(post_hero_layout.hero_area.height > 0);

    let mut no_subscriptions_app = make_app_stub();
    no_subscriptions_app.tab = TabSelection::Feeds;
    no_subscriptions_app.mini_view_focus = crate::app::PanelFocus::Library;
    let mut no_subscriptions_layout = LayoutMain::default();
    let mut no_subscriptions_terminal = Terminal::new(TestBackend::new(60, 20)).unwrap();
    no_subscriptions_terminal
        .draw(|f| {
            no_subscriptions_app.render_feeds(
                f,
                Rect::new(0, 0, 60, 20),
                true,
                &mut no_subscriptions_layout,
            )
        })
        .unwrap();
    assert!(no_subscriptions_layout.selector_tabs.is_empty());
    assert_eq!(no_subscriptions_layout.left_area.y, 3);
    let empty_row = (0..60)
        .map(|x| no_subscriptions_terminal.backend().buffer()[(x, 3)].symbol())
        .collect::<String>();
    assert!(
        empty_row.contains("No feed subscriptions configured"),
        "empty/help content moved: {empty_row:?}"
    );
}
