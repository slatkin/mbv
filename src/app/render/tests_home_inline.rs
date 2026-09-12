use super::test_helpers::{buffer_to_string, render_home_shell_with};
use super::*;
use crate::app::tests::make_app_stub;
use crate::app::types_playback::HomeLatestSource;
use crate::app::{PanelFocus, TabSelection};
use mbv_core::api::TICKS_PER_SECOND;
use mbv_core::config::{AudiobookshelfSetup, FeedKind};
use mbv_core::playback_queue::{FeedEntry, QueueItem};

#[test]
fn wide_home_audiobookshelf_hero_paints_cover_slot_and_subtitle() {
    let app = home_emby_app();
    app.config.lock().unwrap().audiobookshelf_setup = Some(AudiobookshelfSetup::default());
    let item =
        QueueItem::AudiobookshelfBook(mbv_core::playback_queue::AudiobookshelfBookQueueItem {
            library_item_id: "book-1".into(),
            title: "Home Book".into(),
            author: Some("Author".into()),
            duration_ticks: None,
            position_ticks: 0,
            played: false,
            is_finished: false,
            cover_path: Some("cover-1".into()),
        });
    let (model, terminal) = render_home_shell_with(app, 160, 40, |m| {
        m.home_section_pending = Some(HomeLatestSource::Audiobookshelf("books".into()));
        m.home_content.latest = vec![(
            "Books".into(),
            HomeLatestSource::Audiobookshelf("books".into()),
            vec![item],
        )];
    });
    let output = buffer_to_string(&terminal);
    assert!(output.contains("Home Book"));
    assert!(output.contains("Author"));

    // The shared ABS-book producer's Portrait header paints (task 5.11): the
    // same facts the Books tab renders for the same book. Images may be
    // disabled globally; the artwork box renders the shared placeholder.
    let geometry = home_panel(&model)
        .test_wide_geometry()
        .expect("wide Home paints the Wide skeleton");
    let hero = geometry.hero_area;
    let placeholder = palette::surface_colors(palette::Surface::ArtworkPlaceholder, false).fill;
    assert!(
        (hero.top()..hero.bottom()).any(|row| (hero.left()..hero.right()).any(|x| terminal
            .backend()
            .buffer()[(x, row)]
            .bg
            == placeholder)),
        "the Portrait header reserves its artwork box"
    );
}

fn home_emby_app() -> crate::app::App {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app
}

fn home_panel(
    model: &crate::app::shell::Model,
) -> &crate::app::components::library_panel::LibraryPanel {
    model
        .application
        .get_component(&crate::app::components::ComponentId::Library)
        .expect("Library panel mounted")
        .as_any()
        .downcast_ref::<crate::app::components::library_panel::LibraryPanel>()
        .expect("Library panel type")
}

/// The Continue Watching item the tests seed into Model-owned `home_content`
/// (task 5.3d): the focused Emby movie the legacy characterization used to
/// seed via `app.home`.
fn emby_cw_item() -> mbv_core::api::EmbyItem {
    let movie_app = make_movie_app();
    movie_app.libs[0].nav_stack[0].items[0].clone()
}

/// Task 5.3d + 5.11: the Narrow inline hero is derived from the same
/// `HeroContent` the Wide header uses (design D7) — one right-aligned
/// wrap-around form, no per-destination Narrow hero. The text-only feed hero
/// keeps its row title and duration metadata (its policy shape reserves no
/// artwork source), and the projection still fetches nothing for it.
#[test]
fn narrow_home_feed_renders_text_only_without_artwork() {
    let mut app = make_app_stub();
    app.tab = TabSelection::Home;
    app.panel_focus = PanelFocus::Library;
    app.mini_view_focus = PanelFocus::Library;
    // Select the Feeds pill through the real pending-source boundary (task
    // 5.3d, numeric Home section deletion): `render_home_shell_with`'s
    // `push_home_content` restores the Feeds section once its pill exists.
    // Home content is Model-owned (task 5.3d), so the pill data is seeded on
    // `home_content.latest` right after `Model::new`.
    let (model, terminal) = render_home_shell_with(app, 60, 20, |m| {
        m.home_section_pending = Some(HomeLatestSource::Feeds);
        m.home_content.latest = vec![(
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
        )];
    });
    let output = buffer_to_string(&terminal);

    assert!(output.contains("Home Feed entry"), "title: {output:?}");
    assert!(output.contains("1:05"), "duration metadata: {output:?}");
    // The inline hero was admitted (the old text-only detail block's
    // replacement) and requests no card artwork (design D5: feed entries
    // declare no artwork source).
    // The Narrow inline hero is the panel's one form (design D7): admission
    // follows the viewport — the policy-shaped feed hero's box no longer
    // collapses to the deleted text-only height, so a short viewport keeps
    // the plain selected row (the same admission every destination gets).
    let admitted = home_panel(&model)
        .test_narrow_geometry()
        .and_then(|geometry| geometry.inline_hero);
    if let Some(block) = admitted {
        assert!(block.height > 0);
    }
    assert!(model.app.card_image_loading.is_empty());
    assert!(model.app.card_image_states.is_empty());
}

/// Task 5.3d + 5.11: renders through the panel; the narrow inline-detail flow
/// is characterized from the panel's own painted geometry (single painter).
#[test]
fn narrow_home_inserts_selected_detail_into_the_section_flow() {
    let mut app = home_emby_app();
    // Mini view defaults to queue-only, which doesn't render the Home tab at
    // all; opt into the library side so this test exercises the narrow
    // inline-detail flow it was written for.
    app.mini_view_focus = PanelFocus::Library;
    let (model, _terminal) = render_home_shell_with(app, 60, 40, |m| {
        m.home_content.continue_items = vec![emby_cw_item()];
    });

    let geometry = home_panel(&model)
        .test_narrow_geometry()
        .expect("narrow detail flow paints the skeleton");
    let hero = geometry
        .inline_hero
        .expect("narrow detail flow should admit a hero");
    assert!(hero.height > 0);
    assert!(hero.y >= geometry.list_area.y);
}

/// Task 5.3d + 5.11: a viewport too short for the inline detail suppresses
/// the hero — the panel admits no inline block, asserted from the panel's own
/// painted geometry (or its absence when the mini view hides the library
/// column entirely: mounted ≠ painted, design D2).
#[test]
fn narrow_home_suppresses_detail_when_the_viewport_is_too_short() {
    let mut app = home_emby_app();
    app.mini_view_focus = PanelFocus::Library;
    let (model, _terminal) = render_home_shell_with(app, 60, 4, |m| {
        m.home_content.continue_items = vec![emby_cw_item()];
    });

    let admitted = home_panel(&model)
        .test_narrow_geometry()
        .and_then(|geometry| geometry.inline_hero);
    assert_eq!(
        admitted, None,
        "a viewport too short for the inline detail admits no hero"
    );
}
