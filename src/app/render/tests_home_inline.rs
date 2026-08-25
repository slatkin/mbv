use super::test_helpers::*;
use crate::app::components::{ComponentId, HomeComponent};
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
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app
}

/// Task 5.3d, Home legacy underpaint removal: this renders through the
/// mounted `HomeComponent` (shell-equivalent `render_home_shell`), and the
/// text-only hero is characterized from the component's own painted
/// `hero_area` rather than a legacy `LayoutMain` copy. The behavioral
/// assertions are preserved: the text-only feed hero still paints its row
/// title and duration metadata, still sizes to its content (height 7) rather
/// than a tall artwork hero, and still requests no card artwork.
#[test]
fn narrow_home_feed_renders_text_only_without_artwork() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    // Select the Feeds pill through the real pending-source boundary (task
    // 5.3d, numeric Home section deletion): `render_home_shell`'s
    // `push_home_content`
    // restores the Feeds section once its pill exists.
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
    app.home_section_pending = Some(HomeLatestSource::Feeds);

    let (model, terminal) = render_home_shell(app, 60, 20);
    let output = buffer_to_string(&terminal);

    assert!(output.contains("Home Feed entry"), "title: {output:?}");
    assert!(output.contains("1:05"), "duration metadata: {output:?}");
    let home = model
        .application
        .get_component(&ComponentId::Home)
        .expect("Home component mounted")
        .as_any()
        .downcast_ref::<HomeComponent>()
        .expect("Home component type");
    assert_eq!(
        home.hero_area().map(|a| a.height),
        Some(7),
        "text-only hero"
    );
    assert!(model.app.card_image_loading.is_empty());
    assert!(model.app.card_image_states.is_empty());
}

/// Task 5.3d, Home legacy underpaint removal: renders through the mounted
/// `HomeComponent`; the narrow inline-detail flow is characterized from the
/// component's own painted hero and list rects (single painter) instead of
/// `LayoutMain` copies.
#[test]
fn narrow_home_inserts_selected_detail_into_the_section_flow() {
    let mut app = home_emby_app();
    // Mini view defaults to queue-only, which doesn't render the Home tab at
    // all; opt into the library side so this test exercises the narrow
    // inline-detail flow it was written for.
    app.mini_view_focus = PanelFocus::Library;
    let (model, _terminal) = render_home_shell(app, 60, 40);

    let home = model
        .application
        .get_component(&ComponentId::Home)
        .expect("Home component mounted")
        .as_any()
        .downcast_ref::<HomeComponent>()
        .expect("Home component type");
    let hero = home
        .hero_area()
        .expect("narrow detail flow should admit a hero");
    let (left_area, _) = home.menu_placement_geometry();
    assert!(hero.height > 0);
    assert!(hero.y >= left_area.y);
}

/// Task 5.3d, Home legacy underpaint removal: renders through the mounted
/// `HomeComponent`; a viewport too short for the inline detail suppresses
/// the hero — the reserved home area is empty, so the component paints no
/// hero at all — asserted from the component's own painted geometry.
#[test]
fn narrow_home_suppresses_detail_when_the_viewport_is_too_short() {
    let mut app = home_emby_app();
    app.mini_view_focus = PanelFocus::Library;
    let (model, _terminal) = render_home_shell(app, 60, 4);

    let home = model
        .application
        .get_component(&ComponentId::Home)
        .expect("Home component mounted")
        .as_any()
        .downcast_ref::<HomeComponent>()
        .expect("Home component type");
    assert_eq!(home.hero_area(), None);
}
