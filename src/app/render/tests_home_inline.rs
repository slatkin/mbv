use super::test_helpers::*;
use crate::app::tests::make_app_stub;
use crate::app::types_playback::HomeLatestSource;
use crate::app::{PanelFocus, TabSelection};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::config::FeedKind;
use mbv_core::playback_queue::{FeedEntry, QueueItem};

fn home_emby_app() -> crate::app::App {
    let mut app = make_app_stub();
    let movie_app = make_movie_app();
    app.home.continue_items = vec![movie_app.libs[0].nav_stack[0].items[0].clone()];
    app.home.section = 0;
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app
}

#[test]
fn narrow_home_feed_renders_text_only_without_artwork() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    app.home.latest = vec![(
        "Feeds".into(),
        HomeLatestSource::Feeds,
        vec![QueueItem::Feed(FeedEntry {
            guid: "home-feed-entry".into(),
            title: "Home Feed entry".into(),
            enclosure_url: None,
            link: None,
            mime_type: None,
            duration_ticks: Some(65 * TICKS_PER_SECOND as u64),
            pub_date_secs: None,
            feed_kind: Some(FeedKind::Audio),
            feed_id: None,
            position_ticks: 0,
            played: false,
        })],
        0,
    )];
    app.home.section = 1;
    app.home.home_cursor = 0;

    let (terminal, layout) = render_view_to_terminal(&mut app, 60, 20);
    let output = buffer_to_string(&terminal);

    assert!(output.contains("Home Feed entry"), "title: {output:?}");
    assert!(output.contains("1:05"), "duration metadata: {output:?}");
    assert_eq!(
        layout.hero_area.height, 7,
        "text-only hero: {:?}",
        layout.hero_area
    );
    assert!(app.card_image_loading.is_empty());
    assert!(app.card_image_states.is_empty());
}

#[test]
fn narrow_home_inserts_selected_detail_into_the_section_flow() {
    let mut app = home_emby_app();
    // Mini view defaults to queue-only, which doesn't render the Home tab at
    // all; opt into the library side so this test exercises the narrow
    // inline-detail flow it was written for.
    app.mini_view_focus = PanelFocus::Library;
    let layout = render_view(&mut app, 60, 40);

    assert!(layout.hero_area.height > 0);
    assert!(layout.hero_area.y >= layout.left_area.y);
}

#[test]
fn narrow_home_suppresses_detail_when_the_viewport_is_too_short() {
    let mut app = home_emby_app();
    app.mini_view_focus = PanelFocus::Library;
    let layout = render_view(&mut app, 60, 4);

    assert_eq!(layout.hero_area.height, 0);
}
