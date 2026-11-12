//! The Library section's contracts (add-gui-library-panel rows 3.4, 4.3,
//! design D2, D5, D6, and `gui-change-driven-sections` design D2): the
//! destination tabs, the selected collection names, the content the section
//! carries only when it changes, the Hero's artwork from the image cache, and
//! the playing row's track id.

use mbv::{ClientTickHost, stub_app_with_playing_items};
use mbv_gui::bridge::frame::library::GuiLibraryTabFrame;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;

use crate::{layout_report, test_core_with, test_emby_client};

use super::{
    assert_image, assert_placeholder, hero_art, library_of, seeded_music_core, send_library,
    send_library_frame,
};

/// The section carries the playing track's id (add-gui-library-panel, the
/// Library's playing row): the Queue panel's own authority — the Owner status
/// and the playback queue's item at the active index — so the tracklist's
/// playing row paints what the Queue's active slot paints.
#[test]
fn the_music_frame_carries_the_playing_track_id() {
    let http = MockHttp::new();
    let mut playing = mbv_emby_model::test_support::make_item("Rhymin & Stealin", "Audio");
    playing.id = "trk-1".into();
    playing.media_type = "Audio".into();
    let app = stub_app_with_playing_items(vec![mbv_queue::QueueItem::Emby(Box::new(playing))]);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(&http));
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");

    let content = library_of(&send_library_frame(&mut test, GuiLibraryCommand::TakeFocus));

    let music = content
        .and_then(|content| content.music)
        .expect("the Music destination is active");
    assert_eq!(music.playing_track_id.as_deref(), Some("trk-1"));
}

/// The section's destination facts (row 4.3, design D2): the Library section
/// carries the presented tabs — the seeded catalog's one Grouped Music
/// library at its open-destination position, marked current — and the Music
/// content names the `libIdx` the set-collections command takes.
#[test]
fn the_library_section_carries_the_destination_tabs_and_the_lib_idx() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);

    let content = library_of(&send_library_frame(&mut test, GuiLibraryCommand::TakeFocus))
        .expect("the first batch carries the content");
    assert_eq!(
        content.tabs,
        [GuiLibraryTabFrame {
            position: 1,
            label: "Music".to_string(),
        }]
    );
    assert_eq!(content.current_tab, Some(1));
    let music = content.music.expect("the Music destination is active");
    assert_eq!(music.lib_idx, 0);
}

/// The section carries the selected collection names in pill order once for
/// the whole panel (design D6): the list header's, the Status bar location's,
/// and the Hero header's names read this one fact. Regression for the
/// Phase-1 gate finding: the names were derived separately in the projection,
/// the Status bar, and the list header.
#[test]
fn the_music_frame_carries_the_selected_collection_names() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-hiphop".into(),
        },
    );

    let music = library_of(&test.held())
        .and_then(|content| content.music)
        .expect("the Music destination is active");
    assert_eq!(music.collection_names, ["Hip-Hop"]);
}

/// The Library section rides a batch only when its content changes (design D2):
/// a tick that changes nothing sends no batch, a layout report moves no
/// content, and a picked album that changes the Hero sends the new content.
#[test]
fn the_library_section_rides_only_a_changed_content() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);
    send_library(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-hiphop".into(),
        },
    );
    let first = test.held().library().cloned();
    assert!(first.is_some(), "the first content rides its section");

    test.waker.wake();
    assert!(test.core.step());
    crate::assert_no_batch(&test.batches);

    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    crate::assert_no_batch(&test.batches);

    // Picking the other album moves the Hero off its default first album, so the
    // content changes.
    send_library(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-paul".into(),
        },
    );
    assert_ne!(test.held().library().cloned(), first);
}

/// The Hero's art comes through the section from the image cache (row 3.4,
/// design D5): the placeholder paints until a fetched image lands, and the
/// landed art rides the batch its content change asks for.
#[test]
fn the_hero_art_rides_the_section_after_the_image_lands() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);
    let _selected = send_library_frame(
        &mut test,
        GuiLibraryCommand::SelectCollection {
            lib_idx: 0,
            group_id: "grp-hiphop".into(),
        },
    );
    let picked = send_library_frame(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-paul".into(),
        },
    );
    assert_placeholder(hero_art(&picked));

    // The cache's enabled gate opens and the album's fetch resolves with an
    // image: the tick's drain lands it, so the next batch carries the art.
    test.core.app_mut().enable_card_images_for_test();
    test.core.app_mut().complete_card_image_for_test(
        &mbv_images::emby_card_cache_key("alb-paul", ""),
        image::DynamicImage::new_rgb8(4, 4),
    );
    test.waker.wake();
    assert!(test.core.step());
    assert_image(hero_art(&test.held()));
}
