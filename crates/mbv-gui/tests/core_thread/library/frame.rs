//! The Library section's frame contracts (add-gui-library-panel rows 3.4,
//! 4.3, design D2, D5, D6): the destination tabs, the selected collection
//! names, the content revision, the Hero's artwork from the image cache, and
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

/// The frame carries the playing track's id (add-gui-library-panel, the
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
    core.seed_gui_library_groups_for_test(0, &["grp-hiphop"]);

    let frame = library_of(send_library_frame(&mut test, GuiLibraryCommand::TakeFocus));

    let music = frame
        .content
        .and_then(|content| content.music)
        .expect("the Music destination is active");
    assert_eq!(music.playing_track_id.as_deref(), Some("trk-1"));
}

/// The frame's destination facts (row 4.3, design D2): the Library section
/// carries the presented tabs — the seeded catalog's one Grouped Music
/// library at its open-destination position, marked current — and the Music
/// content names the `libIdx` the set-collections command takes.
#[test]
fn the_library_frame_carries_the_destination_tabs_and_the_lib_idx() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);

    let frame = library_of(send_library_frame(&mut test, GuiLibraryCommand::TakeFocus));

    let content = frame.content.expect("the first frame carries the content");
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

/// The frame carries the selected collection names in pill order once for
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
        GuiLibraryCommand::SetCollections {
            lib_idx: 0,
            group_ids: vec!["grp-hiphop".into()],
        },
    );

    let music = library_of(test.next_frame())
        .content
        .and_then(|content| content.music)
        .expect("the Music destination is active");
    assert_eq!(music.collection_names, ["Hip-Hop"]);
}

/// The Library section carries its content only when the revision changes, and
/// again after a `report_layout` (add-gui-library-panel row 3.4, design D2):
/// the front end keeps the last carried content, so a frame whose content is
/// unchanged carries the revision alone.
#[test]
fn the_library_content_rides_a_changed_revision_and_a_layout_report() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);
    let first = library_of(send_library_frame(
        &mut test,
        GuiLibraryCommand::SetCollections {
            lib_idx: 0,
            group_ids: vec!["grp-hiphop".into()],
        },
    ));
    let revision = first.revision;
    assert!(first.content.is_some(), "the first content rides its frame");

    test.waker.wake();
    assert!(test.core.step());
    let unchanged = library_of(test.next_frame());
    assert_eq!(unchanged.revision, revision);
    assert!(
        unchanged.content.is_none(),
        "an unchanged revision carries no content"
    );

    // Picking the album gives the Hero its facts, so the content changes.
    let changed = library_of(send_library_frame(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-ill".into(),
        },
    ));
    assert_eq!(changed.revision, revision.wrapping_add(1));
    assert!(
        changed.content.is_some(),
        "a changed revision rides its frame"
    );

    // A window reload's report re-arms the section: the content rides again
    // under the revision the window already knows.
    test.tx.send(layout_report()).unwrap();
    assert!(test.core.step());
    let rearmed = library_of(test.next_frame());
    assert_eq!(rearmed.revision, changed.revision);
    assert!(
        rearmed.content.is_some(),
        "the report re-carries the content"
    );
}

/// The Hero's art comes through the frame from the image cache (row 3.4,
/// design D5): the placeholder paints until a fetched image lands, and the
/// landed art rides the frame its revision change asks for.
#[test]
fn the_hero_art_rides_the_frame_after_the_image_lands() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = seeded_music_core(&http);
    let _selected = send_library_frame(
        &mut test,
        GuiLibraryCommand::SetCollections {
            lib_idx: 0,
            group_ids: vec!["grp-hiphop".into()],
        },
    );
    let picked = send_library_frame(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-ill".into(),
        },
    );
    assert_placeholder(hero_art(picked));

    // The cache's enabled gate opens and the album's fetch resolves with an
    // image: the tick's drain lands it, so the next frame carries the art.
    test.core.app_mut().enable_card_images_for_test();
    test.core.app_mut().complete_card_image_for_test(
        &mbv_images::emby_card_cache_key("alb-ill", ""),
        image::DynamicImage::new_rgb8(4, 4),
    );
    test.waker.wake();
    assert!(test.core.step());
    assert_image(hero_art(test.next_frame()));
}
