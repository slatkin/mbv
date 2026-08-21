use super::test_helpers::*;
use super::*;
use crate::app::tests::make_app_stub;
use crate::app::TabSelection;
use mbv_core::config::{FeedKind, FeedSubscription};
use mbv_core::playback_queue::FeedEntry;

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
