//! The section batch's contracts (`gui-change-driven-sections` design D1, D2):
//! a tick sends the sections it changed, each under its revision, and nothing
//! else.

use mbv::{
    App, ClientTickHost, stub_app_with_completed_emby_startup,
    stub_app_with_completed_emby_startup_and_contents, stub_app_with_queue,
};
use mbv_emby_model::EmbyItem;
use mbv_emby_model::test_support::make_item;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::bridge::sections::{GuiSection, GuiSectionBatch};
use mbv_gui::core_thread::CoreInput;
use mbv_gui::sections::{Section, SentRevisions};
use mbv_net::mock_http::MockHttp;
use mbv_ui_msg::{Key, KeyEvent, KeyModifiers};
use rstest::rstest;

use crate::{TestCore, test_core, test_core_with, test_emby_client};

/// The attached core's first batch carries `attach: null` (design D4): the
/// window's pre-attach state clears.
#[test]
fn the_first_batch_clears_the_attach_state() {
    let mut test = test_core(2);
    assert!(test.core.step());
    assert_eq!(
        test.next_batch().attach,
        Some(GuiSection {
            revision: 1,
            value: None,
        })
    );
}

/// The attached core's attach and theme sections mint past the revisions the
/// campaign sent (design D4), so the window never skips the `null` for a
/// revision it already holds.
#[test]
fn the_attached_core_continues_the_campaign_revisions() {
    let mut test = test_core(2);
    test.core.resume_after_attach(SentRevisions {
        attach: 4,
        theme: 2,
    });
    assert!(test.core.step());
    let batch = test.next_batch();
    assert_eq!(batch.attach.map(|section| section.revision), Some(5));
    assert_eq!(batch.theme.map(|section| section.revision), Some(3));
}

/// After a batch, `sync` re-sends every present section at the revision it
/// held (design D4) — except the playhead (design D5): its value is a
/// position-at-send sample the front end stamps on receipt, so a replay would
/// rewind the position. Sync forgets the anchor, and the next collect
/// re-adopts a fresh one at a new revision, so the playhead rides the sync
/// batch with a revision strictly past the first batch's.
#[test]
fn sync_re_sends_every_present_section_at_its_revision() {
    let mut test = test_core(2);
    assert!(test.core.step());
    let first = test.next_batch();

    test.tx.send(CoreInput::Sync).unwrap();
    assert!(test.core.step());
    let second = test.next_batch();

    let mut held_at_its_revision = second.clone();
    held_at_its_revision.playhead = None;
    let mut expected = first.clone();
    expected.playhead = None;
    assert_eq!(held_at_its_revision, expected);
    let first_playhead = first.playhead.expect("the first batch carries the anchor");
    let second_playhead = second
        .playhead
        .expect("the sync batch re-adopts the anchor");
    assert!(second_playhead.revision > first_playhead.revision);
}

/// A queue cursor move sends `queue` with a new revision and no other section
/// (design D2): the cursor is the only fact the key changed, and the compare
/// finds no other section moved.
#[test]
fn a_queue_cursor_move_sends_only_the_queue_section() {
    let mut test = test_core(3);
    assert!(test.core.step());
    let first_revision = test
        .next_batch()
        .queue
        .map(|section| section.revision)
        .expect("the first batch carries the queue");

    test.tx
        .send(CoreInput::Key(KeyEvent::new(Key::Down, KeyModifiers::NONE)))
        .unwrap();
    assert!(test.core.step());
    let batch = test.next_batch();

    let queue = batch
        .queue
        .clone()
        .expect("the cursor move sends the queue");
    assert!(queue.revision > first_revision);
    assert_eq!(
        batch,
        GuiSectionBatch {
            queue: Some(queue),
            ..GuiSectionBatch::default()
        }
    );
}

/// The gated sections a batch carries (design D2), in ledger order.
fn carried_gated(batch: &GuiSectionBatch) -> Vec<Section> {
    [
        (Section::Theme, batch.theme.is_some()),
        (Section::Library, batch.library.is_some()),
        (Section::Search, batch.search.is_some()),
        (Section::Settings, batch.settings.is_some()),
    ]
    .into_iter()
    .filter_map(|(section, carried)| carried.then_some(section))
    .collect()
}

/// A Continue Watching item whose id is its name.
fn continue_item(id: &str) -> EmbyItem {
    let mut item = make_item(id, "Movie");
    item.id = id.to_string();
    item
}

/// An App whose Home bootstrap completes on the first step, with two
/// Continue Watching items for the Home rows.
fn library_app() -> App {
    stub_app_with_completed_emby_startup_and_contents(
        Vec::new(),
        vec![continue_item("cw-first"), continue_item("cw-second")],
    )
}

fn empty_app() -> App {
    stub_app_with_queue(0)
}

/// Seed one Music library whose group holds two albums, so a selected album's
/// Hero carries card art (as the Library frame tests seed it), then pick the
/// group and open one album's detail. The Hero paints its placeholder until the
/// album's card image lands.
fn show_album(test: &mut TestCore) {
    let http = MockHttp::new();
    let app = test.core.app_mut();
    app.seed_gui_music_library_for_test(test_emby_client(&http));
    app.seed_gui_library_groups_for_test(0, "grp-hiphop");
    app.seed_gui_music_albums_for_test(
        0,
        "grp-hiphop",
        &[
            ("alb-ill", "Licensed to Ill"),
            ("alb-paul", "Paul's Boutique"),
        ],
    );
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::SelectCollection {
                lib_idx: 0,
                group_id: "grp-hiphop".into(),
            }
            .into(),
        ))
        .unwrap();
    // One step applies one queued input, so the group pick gets its own step
    // before the album open the row's settle step applies.
    assert!(test.core.step());
    test.held();
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::ShowDetail {
                item_id: "alb-paul".into(),
            }
            .into(),
        ))
        .unwrap();
}

/// Open Home, the first destination (position 0).
fn open_home(test: &mut TestCore) {
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::OpenDestination { position: 0 }.into(),
        ))
        .unwrap();
}

/// Open the Search field with a query (row 12.1).
fn open_search(test: &mut TestCore) {
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::SearchQuery {
                query: "blue".into(),
            }
            .into(),
        ))
        .unwrap();
}

/// A Home refresh the Library drain applies on the next step.
fn land_home_refresh(test: &mut TestCore) {
    test.core
        .app_mut()
        .queue_home_content_refreshed_for_test(vec![continue_item("cw-third")]);
}

/// The shown album's fetched card image, which the card drain lands on the
/// next step.
fn land_card_image(test: &mut TestCore) {
    test.core.app_mut().enable_card_images_for_test();
    test.core.app_mut().complete_card_image_for_test(
        &mbv_images::emby_card_cache_key("alb-paul", ""),
        image::DynamicImage::new_rgb8(4, 4),
    );
}

/// The search worker's result the search drain stores on the next step.
fn land_search_result(test: &mut TestCore) {
    test.core
        .app_mut()
        .send_gui_search_result_for_test("blue", vec![continue_item("blue-1")]);
}

/// One drain hook's row (design D3 rule 2): a settled first batch, then an
/// event the hook drains. The next batch carries only the gated sections the
/// event changed. `prepare` sends the input that opens the destination first.
#[rstest]
#[case::library_data(library_app, open_home, land_home_refresh, &[Section::Library])]
#[case::card_images(empty_app, show_album, land_card_image, &[Section::Library])]
#[case::search_results(empty_app, open_search, land_search_result, &[Section::Search])]
fn a_drain_hook_sends_the_gated_sections_it_changed(
    #[case] app: fn() -> App,
    #[case] prepare: fn(&mut TestCore),
    #[case] land: fn(&mut TestCore),
    #[case] expected: &[Section],
) {
    let mut test = test_core_with(app());
    prepare(&mut test);
    assert!(test.core.step());
    test.next_batch();

    land(&mut test);
    assert!(test.core.step());
    assert_eq!(carried_gated(&test.next_batch()), expected.to_vec());
}

/// The startup drain's catalog reaches the Library section (design D3 rule 2).
/// The stub queues the Emby startup at construction, so no batch precedes its
/// drain: this test reads the first batch, which also carries every gated
/// section's first build (design D2). It checks the catalog content, not the
/// section set, so it is not a `#[case]` row of the table above.
#[test]
fn the_startup_drain_sends_the_music_tab_in_the_library_section() {
    let music = EmbyItem::folder("lib-music".into(), "Music".into(), "music".into());
    let mut test = test_core_with(stub_app_with_completed_emby_startup(vec![music]));
    assert!(test.core.step());

    let content = test
        .next_batch()
        .library
        .expect("the startup drain sends the library")
        .value
        .expect("the catalog builds a library frame");
    assert!(content.tabs.iter().any(|tab| tab.label == "Music"));
}

/// Past the navigation-idle delay (`NAV_IDLE_DELAY` is 150 ms): one second is
/// long enough, and the test moves the clock instead of waiting.
const IDLE_PAST_THE_GATE: std::time::Duration = std::time::Duration::from_secs(1);

/// Seed one Music group of six albums, `alb-0` through `alb-5`. The default pick
/// is `alb-0`, and its first build warms `alb-1` through `alb-3` while the gate
/// is still open at construction. So the last album's only unwarmed neighbour is
/// `alb-4`, and a test of the gate's opening needs that one.
fn seed_six_albums(test: &mut TestCore) {
    let http = MockHttp::new();
    let albums: Vec<(String, String)> = (0..6)
        .map(|index| (format!("alb-{index}"), format!("Album {index}")))
        .collect();
    let albums: Vec<(&str, &str)> = albums
        .iter()
        .map(|(id, name)| (id.as_str(), name.as_str()))
        .collect();
    let app = test.core.app_mut();
    app.seed_gui_music_library_for_test(test_emby_client(&http));
    app.seed_gui_music_albums_for_test(0, "grp-hiphop", &albums);
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::SelectCollection {
                lib_idx: 0,
                group_id: "grp-hiphop".into(),
            }
            .into(),
        ))
        .unwrap();
}

/// A passed navigation gate starts the Hero neighbour warm with no input
/// (design D3 rule 3). The pick closes the gate, so its warm starts nothing. The
/// next tick, with no input, sees the gate open, marks the Library section, and
/// the rebuilt Library starts the neighbour's fetch (`alb-4` is one behind the
/// picked `alb-5`). The audit is off: an audit build would start the same fetch,
/// so only the mark can be the cause (design D3).
#[test]
fn a_passed_navigation_gate_starts_the_hero_neighbour_fetch_with_no_input() {
    let mut test = test_core_with(empty_app());
    test.core.set_section_audit_for_test(false);
    test.core.app_mut().enable_card_images_for_test();
    seed_six_albums(&mut test);
    assert!(test.core.step());
    test.tx
        .send(CoreInput::Library(
            GuiLibraryCommand::ShowDetail {
                item_id: "alb-5".into(),
            }
            .into(),
        ))
        .unwrap();
    assert!(test.core.step());
    let neighbour = mbv_images::emby_card_cache_key("alb-4", "");
    assert!(!test.core.app_mut().card_image_loading_for_test(&neighbour));

    test.core
        .app_mut()
        .backdate_navigation_for_test(IDLE_PAST_THE_GATE);
    assert!(test.core.step());

    assert!(test.core.app_mut().card_image_loading_for_test(&neighbour));
}

/// One Music track's playing item, as the Owner's queue carries it.
fn playing_track(id: &str) -> mbv_queue::QueueItem {
    let mut item = make_item(id, "Audio");
    item.id = id.into();
    item.media_type = "Audio".into();
    mbv_queue::QueueItem::Emby(Box::new(item))
}

/// A playing-track change with no input sends the Library section (design D3
/// rule 4). The Owner's active slot moves to the second track, and the next
/// tick, with no input, marks the Library and sends the Music frame with the
/// new playing row. `libraryDetail` is not a section until the Library section
/// splits in group 6, so the one gated `library` section carries the change.
#[test]
fn a_playing_track_change_with_no_input_sends_the_library_section() {
    let http = MockHttp::new();
    let app =
        mbv::stub_app_with_playing_items(vec![playing_track("trk-1"), playing_track("trk-2")]);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(&http));
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");
    test.tx
        .send(CoreInput::Library(GuiLibraryCommand::TakeFocus.into()))
        .unwrap();
    assert!(test.core.step());
    test.next_batch();

    test.core.app_mut().set_playing_index_for_test(1);
    assert!(test.core.step());

    assert_eq!(carried_gated(&test.next_batch()), [Section::Library]);
    let held = test.held();
    let music = held
        .library()
        .and_then(|content| content.music.as_ref())
        .expect("the Music frame is presented");
    assert_eq!(music.detail.playing_track_id.as_deref(), Some("trk-2"));
}
