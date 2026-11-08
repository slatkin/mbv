//! The Library panel's contracts (add-gui-library-panel rows 3.1, 3.4): every
//! command family reaches the `App` effect it names — the destination, the
//! stored selection or detail, the picked album's track load, and the mocked
//! Owner request a play, an add, or a shuffle sends — and the Library section's
//! content revision and Hero artwork reach the frame from the image cache. The
//! Emby answers come from the in-memory mock transport, so no request leaves
//! the process, and no live Owner, Service, or webview runs (design D13).

use std::sync::mpsc::Receiver;

use mbv::{ClientTickHost, stub_app_with_playing_items, stub_app_with_queue_and_cmd_rx};
use mbv_ctrl::CtrlCmd;
use mbv_emby_model::EmbyItem;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::frame::focus::{GuiPanel, GuiSurface};
use mbv_gui::bridge::frame::library::{GuiLibraryArt, GuiLibraryFrame, GuiLibraryTabFrame};
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::QueueSource;
use mbv_ui_model::tab_selection::TabSelection;

use crate::{TestCore, layout_report, test_core_with, test_emby_client};

/// One group's album listing, as the mock transport returns it.
const ALBUMS: &str = r#"{"Items":[{"Id":"alb-ill","Name":"Licensed to Ill","Type":"MusicAlbum"}],"TotalRecordCount":1}"#;

/// One album's playable tracks, as the album expansion's fetch returns them.
const TRACKS: &str = r#"{"Items":[{"Id":"trk-1","Name":"Rhymin & Stealin","Type":"Audio","MediaType":"Audio","RunTimeTicks":2160000000},{"Id":"trk-2","Name":"The New Style","Type":"Audio","MediaType":"Audio","RunTimeTicks":2100000000}],"TotalRecordCount":2}"#;

/// A core thread over an App with an empty queue and a player whose command
/// channel records what an action sends, with the grouped Music library seeded
/// after the core exists: the launch Home load sees no Service, so it starts
/// no Emby request the test would have to script (design D13).
fn music_core(http: &MockHttp) -> (TestCore, Receiver<CtrlCmd>) {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(http));
    // The selection the restore would otherwise apply: a pinned set makes the
    // restore's own application skip this destination, so the mock answers
    // serve the test's commands alone.
    core.seed_gui_library_groups_for_test(0, &["grp-hiphop"]);
    (test, cmd_rx)
}

/// Send one Library command, as the `library` command's bridge translation
/// hands it to the core, and run one step.
fn send_library(test: &mut TestCore, command: GuiLibraryCommand) {
    test.tx.send(CoreInput::Library(command.into())).unwrap();
    assert!(test.core.step());
}

/// The queue replacement the Owner was asked for, from the stub player's
/// command channel: the queue source and the Emby item ids.
fn replacement(cmd_rx: &Receiver<CtrlCmd>) -> (QueueSource, Vec<String>) {
    cmd_rx
        .try_iter()
        .find_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueueReplace { items, source, .. } => Some((source, emby_ids(items))),
            _ => None,
        })
        .expect("the Owner received a queue replacement")
}

/// The Emby item ids the Owner was asked to append, with the slot the append
/// names as its insertion point.
fn appended_ids(cmd_rx: &Receiver<CtrlCmd>) -> (Option<u64>, Vec<String>) {
    cmd_rx
        .try_iter()
        .find_map(|cmd| match cmd {
            CtrlCmd::UnifiedQueueAppend { items, before, .. } => Some((before, emby_ids(items))),
            _ => None,
        })
        .expect("the Owner received a queue append")
}

/// The Emby item ids of a queue command's items.
fn emby_ids(items: Vec<mbv_queue::QueueItem>) -> Vec<String> {
    items
        .into_iter()
        .filter_map(|item| item.as_emby().map(|item| item.id.clone()))
        .collect()
}

/// An open-destination command makes the destination active (design D2): Home
/// holds position 0 and the first library position 1.
#[test]
fn the_open_destination_command_opens_the_music_tab() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 0 },
    );
    assert_eq!(test.core.app_mut().tab_for_test(), TabSelection::Home);

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 1 },
    );
    assert_eq!(
        test.core.app_mut().tab_for_test(),
        TabSelection::EmbyLibrary(0)
    );
}

/// An open-destination command for a destination the GUI does not present is
/// a no-op (design D2: the top bar carries only the presented tabs, and a
/// destination the GUI does not present is absent — a digit key for one opens
/// nothing). Regression for the Phase-1 gate finding: the command applied the
/// raw position, so a position past the presented tabs activated the
/// unpresented library.
#[test]
fn an_open_destination_command_for_an_unpresented_destination_is_a_no_op() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);
    test.core
        .app_mut()
        .push_gui_library_tab_for_test(EmbyItem::folder(
            "lib-shows".into(),
            "Shows".into(),
            "tvshows".into(),
        ));

    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 0 },
    );
    send_library(
        &mut test,
        GuiLibraryCommand::OpenDestination { position: 2 },
    );

    assert_eq!(test.core.app_mut().tab_for_test(), TabSelection::Home);
}

/// The take-focus command (design D4): the Library panel takes Panel focus,
/// and the next frame carries it. The Library is mounted, so the stack
/// resolves it as the focused surface.
#[test]
fn the_take_focus_command_puts_the_library_in_the_next_frame() {
    let mut test = crate::test_core(2);
    send_library(&mut test, GuiLibraryCommand::TakeFocus);

    let frame = test.next_frame();
    assert_eq!(frame.focus.panel, GuiPanel::Library);
    assert_eq!(frame.focus.surface, Some(GuiSurface::Library));
}

/// The take-queue-focus command returns Panel focus to the Queue panel on
/// both authorities (design D4): the App's stored focus, which its key gates
/// read, and the core's `FocusStack`, which the frame's focus derives from.
/// Regression for the Phase-1 gate finding: the command vocabulary had the
/// Library's take-focus alone, so Tab could never leave the Library panel.
#[test]
fn the_take_queue_focus_command_moves_both_focus_authorities_to_the_queue() {
    let mut test = crate::test_core(2);
    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    send_library(&mut test, GuiLibraryCommand::TakeQueueFocus);

    let _library_frame = test.next_frame();
    let frame = test.next_frame();
    assert_eq!(frame.focus.panel, GuiPanel::Queue);
    assert_eq!(frame.focus.surface, Some(GuiSurface::Queue));
    assert_eq!(
        test.core.app_mut().gui_panel_focus(),
        mbv_ui_model::settings::PanelFocus::Queue
    );
}

/// The set-collections command stores the destination's selection in pill
/// order (design D5, D6): the core keeps it, so the frame's pills paint the
/// core's set.
#[test]
fn the_set_collections_command_stores_the_selected_groups() {
    let http = MockHttp::new();
    http.respond(200, ALBUMS);
    http.respond(200, ALBUMS);
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::SetCollections {
            lib_idx: 0,
            group_ids: vec!["grp-hiphop".into(), "grp-soul".into()],
        },
    );

    assert_eq!(
        test.core.app_mut().gui_library_groups_for_test(0),
        ["grp-hiphop", "grp-soul"]
    );
}

/// The show-detail command makes the picked item the Hero's (design D2).
#[test]
fn the_show_detail_command_shows_the_picked_album() {
    let http = MockHttp::new();
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::ShowDetail {
            item_id: "alb-ill".into(),
        },
    );

    assert_eq!(
        test.core
            .app_mut()
            .gui_library_detail_for_test()
            .map(|item| item.id.as_str()),
        Some("alb-ill")
    );
}

/// The open-album command starts the album's track load when the cache does
/// not hold it (design D5): the tracks the tracklist paints arrive on the
/// drain.
#[test]
fn the_open_album_command_loads_the_albums_tracks() {
    let http = MockHttp::new();
    // One answer for the open's own track load; the pinned selection keeps
    // the launch restore's application off this destination, so no other
    // fetch runs.
    http.respond_repeated(200, TRACKS, 1);
    let (mut test, _cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::OpenAlbum {
            album_id: "alb-ill".into(),
        },
    );

    assert!(
        test.core.app_mut().album_tracks_loading_for_test("alb-ill"),
        "the album's track fetch is in flight"
    );
}

/// Play asks the Owner to replace its queue with the album's tracks (design
/// D8): the folder expands through the shared album path.
#[test]
fn the_play_command_asks_the_owner_to_play_the_album() {
    let http = MockHttp::new();
    // One answer for the play's own expansion; the pinned selection keeps
    // the launch restore's application off this destination, so no other
    // fetch runs.
    http.respond_repeated(200, TRACKS, 1);
    let (mut test, cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::Play {
            item_id: "alb-ill".into(),
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(ids, ["trk-1", "trk-2"]);
    assert_eq!(
        source,
        QueueSource::Collection {
            collection_type: "music".into()
        }
    );
}

/// Add to queue asks the Owner to append the album's tracks at the tail
/// (design D8).
#[test]
fn the_add_to_queue_command_asks_the_owner_to_append_the_album() {
    let http = MockHttp::new();
    // One answer for the enqueue's own expansion; the pinned selection keeps
    // the launch restore's application off this destination, so no other
    // fetch runs.
    http.respond_repeated(200, TRACKS, 1);
    let (mut test, cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::AddToQueue {
            item_id: "alb-ill".into(),
        },
    );

    assert_eq!(
        appended_ids(&cmd_rx),
        (None, vec!["trk-1".into(), "trk-2".into()])
    );
}

/// Add to queue applies the library playback policy's autoload rule to a
/// track (add-gui-library-panel): with autoload on, the album's cached
/// playable tracks enter in disc/track order starting at the selected track,
/// so the double-clicked track is the first of the block and the album
/// follows it. The tracks come from the same cache the tracklist paints —
/// injected here, so no fetch runs.
#[test]
fn the_add_to_queue_command_applies_the_autoload_policy_to_a_track() {
    let http = MockHttp::new();
    let (mut test, cmd_rx) = music_core(&http);
    test.core.app_mut().seed_gui_music_albums_for_test(
        0,
        "grp-hiphop",
        &[("alb-ill", "Licensed to Ill")],
    );
    test.core.app_mut().seed_gui_album_tracks_for_test(
        "alb-ill",
        &[
            ("trk-1", "Rhymin & Stealin", 1, 248),
            ("trk-2", "The New Style", 2, 275),
            ("trk-3", "She's Crafty", 3, 220),
        ],
    );
    test.core.app_mut().set_autoload_for_test(true);

    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    send_library(
        &mut test,
        GuiLibraryCommand::AddToQueue {
            item_id: "trk-2".into(),
        },
    );

    assert_eq!(
        appended_ids(&cmd_rx),
        (None, vec!["trk-2".into(), "trk-3".into()])
    );
}

/// With autoload off, the same add queues only the selected track — the
/// autoload rule's other half.
#[test]
fn the_add_to_queue_command_of_a_track_queues_only_it_while_autoload_is_off() {
    let http = MockHttp::new();
    let (mut test, cmd_rx) = music_core(&http);
    test.core.app_mut().seed_gui_music_albums_for_test(
        0,
        "grp-hiphop",
        &[("alb-ill", "Licensed to Ill")],
    );
    test.core.app_mut().seed_gui_album_tracks_for_test(
        "alb-ill",
        &[
            ("trk-1", "Rhymin & Stealin", 1, 248),
            ("trk-2", "The New Style", 2, 275),
            ("trk-3", "She's Crafty", 3, 220),
        ],
    );
    test.core.app_mut().set_autoload_for_test(false);

    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    send_library(
        &mut test,
        GuiLibraryCommand::AddToQueue {
            item_id: "trk-2".into(),
        },
    );

    assert_eq!(appended_ids(&cmd_rx), (None, vec!["trk-2".into()]));
}

/// Play over a populated queue replaces it and starts, with no confirm
/// pending (add-gui-library-panel): the replace-confirm is a TUI overlay the
/// GUI never paints, so the TUI's gate held the play silently and the Hero's
/// Play button did nothing over a populated queue. Regression for the live
/// finding: one queued item and a Play that never ran.
#[test]
fn the_play_command_over_a_populated_queue_replaces_it_without_a_confirm() {
    let http = MockHttp::new();
    http.respond(200, TRACKS);
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(1);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(&http));
    // The pinned selection keeps the launch restore's application off this
    // destination, so the mock's one answer serves the play's expansion.
    core.seed_gui_library_groups_for_test(0, &["grp-hiphop"]);
    core.seed_gui_music_albums_for_test(0, "grp-hiphop", &[("alb-ill", "Licensed to Ill")]);

    send_library(
        &mut test,
        GuiLibraryCommand::Play {
            item_id: "alb-ill".into(),
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(ids, ["trk-1", "trk-2"], "the queue was replaced");
    assert_eq!(
        source,
        QueueSource::Collection {
            collection_type: "music".into()
        }
    );
    assert!(
        test.core.app_mut().pending_overlay_is_none_for_test(),
        "no confirm is pending"
    );
}

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
/// Shuffle artist plays the artist's albums shuffled (design D8): one
/// replacement over every named album's tracks, with the shuffle source.
#[test]
fn the_shuffle_artist_command_plays_the_artists_albums_shuffled() {
    let http = MockHttp::new();
    // Two answers for the two albums' expansions; the pinned selection keeps
    // the launch restore's application off this destination, so no other
    // fetch runs.
    http.respond_repeated(200, TRACKS, 2);
    let (mut test, cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::ShuffleArtist {
            album_ids: vec!["alb-ill".into(), "alb-paul".into()],
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(ids.len(), 4, "both albums' tracks reached the Owner");
    assert_eq!(source, QueueSource::Shuffle);
}

/// Play next asks the Owner to admit the album's tracks next (design D8):
/// with no active slot the append has no anchor, so the tracks land at the
/// tail.
#[test]
fn the_play_next_command_asks_the_owner_to_admit_the_album_next() {
    let http = MockHttp::new();
    // One answer for the launch restore's first-group fetch (design D9) and
    // one for the admission's own expansion; the answers are identical, so
    // the order they arrive in does not matter.
    http.respond_repeated(200, TRACKS, 2);
    let (mut test, cmd_rx) = music_core(&http);

    send_library(
        &mut test,
        GuiLibraryCommand::PlayNext {
            item_id: "alb-ill".into(),
        },
    );

    assert_eq!(
        appended_ids(&cmd_rx),
        (None, vec!["trk-1".into(), "trk-2".into()])
    );
}

/// The item menu (design D8): the frame carries the picked album's entries,
/// and a chosen entry runs through the shared context-action executor for that
/// album. The GUI never arms the TUI's `pending_overlay`.
#[test]
fn the_item_menu_entries_ride_the_frame_and_run_their_action() {
    let http = MockHttp::new();
    http.respond(200, TRACKS);
    let (mut test, cmd_rx) = seeded_music_core(&http);
    send_library_frame(&mut test, GuiLibraryCommand::TakeFocus);
    send_library_frame(
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

    let menu = library_of(picked)
        .content
        .and_then(|content| content.music)
        .and_then(|music| music.hero)
        .expect("the picked album has a Hero")
        .menu;
    let labels: Vec<&str> = menu.iter().map(|entry| entry.label.as_str()).collect();
    assert_eq!(
        labels,
        ["Play All", "Shuffle", "Add to Queue"],
        "the frame carries the album's own entries"
    );

    send_library(
        &mut test,
        GuiLibraryCommand::MenuEntry {
            item_id: "alb-ill".into(),
            index: 0,
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(
        ids,
        ["trk-1", "trk-2"],
        "the Play All entry played the picked album"
    );
    assert_eq!(
        source,
        QueueSource::Collection {
            collection_type: "music".into()
        }
    );
    assert!(
        test.core.app_mut().pending_overlay_is_none_for_test(),
        "the GUI runs the entry without arming the TUI overlay"
    );
}

/// A core over the grouped Music library with Hip-Hop's album list already in
/// the GUI store, so a frame test reads the projection with no request and no
/// fetch behind it: the group is loaded before the selection names it, and the
/// stub's image cache stays off until a test enables it.
fn seeded_music_core(http: &MockHttp) -> (TestCore, Receiver<CtrlCmd>) {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(http));
    core.seed_gui_library_groups_for_test(0, &["grp-hiphop"]);
    core.seed_gui_music_albums_for_test(0, "grp-hiphop", &[("alb-ill", "Licensed to Ill")]);
    (test, cmd_rx)
}

/// Send one Library command and read the frame its step emitted.
fn send_library_frame(test: &mut TestCore, command: GuiLibraryCommand) -> GuiFrame {
    send_library(test, command);
    test.next_frame()
}

/// The Library section a frame carries (design D2).
fn library_of(frame: GuiFrame) -> GuiLibraryFrame {
    frame
        .library
        .expect("the slice carries the Library section")
}

/// The Hero's artwork value of a frame that carries the Library content.
fn hero_art(frame: GuiFrame) -> GuiLibraryArt {
    library_of(frame)
        .content
        .and_then(|content| content.music)
        .and_then(|music| music.art)
        .expect("the frame carries the Hero's art")
}

/// The Hero paints the bundled placeholder as a `data:image/webp` URL.
fn assert_placeholder(art: GuiLibraryArt) {
    let GuiLibraryArt::Placeholder(url) = art else {
        panic!("the Hero paints the placeholder, not {art:?}");
    };
    assert!(url.starts_with("data:image/webp;base64,"));
}

/// The Hero paints the cached art as a `data:image/jpeg` URL.
fn assert_image(art: GuiLibraryArt) {
    let GuiLibraryArt::Image(url) = art else {
        panic!("the Hero paints the cached art, not {art:?}");
    };
    assert!(url.starts_with("data:image/jpeg;base64,"));
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
