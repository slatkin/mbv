//! The Library panel command's contracts (add-gui-library-panel row 3.1): every
//! command family reaches the `App` effect it names — the destination, the
//! stored selection or detail, the picked album's track load, and the mocked
//! Owner request a play, an add, or a shuffle sends. The Emby answers come from
//! the in-memory mock transport, so no request leaves the process, and no live
//! Owner, Service, or webview runs (design D13).

use std::sync::mpsc::Receiver;

use mbv::{ClientTickHost, Config, stub_app_with_queue_and_cmd_rx};
use mbv_ctrl::CtrlCmd;
use mbv_emby::EmbyClient;
use mbv_gui::bridge::frame::focus::{GuiPanel, GuiSurface};
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::QueueSource;
use mbv_ui_model::tab_selection::TabSelection;
use rstest::rstest;

use crate::{TestCore, test_core_with};

/// A URL that never leaves the process: the mock transport ignores it, but it
/// must remain an IP literal so the default resolver never attempts DNS.
const TEST_URL: &str = "http://127.0.0.1:1";

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
    let client = EmbyClient::new(Config {
        server_url: TEST_URL.into(),
        ..Config::default()
    })
    .with_test_agent(http.agent());
    test.core.app_mut().seed_gui_music_library_for_test(client);
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
    http.respond(200, TRACKS);
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
    http.respond(200, TRACKS);
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
    http.respond(200, TRACKS);
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

/// Shuffle artist plays the artist's albums shuffled (design D8): one
/// replacement over every named album's tracks, with the shuffle source.
#[test]
fn the_shuffle_artist_command_plays_the_artists_albums_shuffled() {
    let http = MockHttp::new();
    http.respond(200, TRACKS);
    http.respond(200, TRACKS);
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

/// The commands rows 3.5 and 3.6 own run nothing yet (add-gui-library-panel
/// rows 3.1, 3.5, 3.6): a stale frame's play-next or menu-entry press sends no
/// Owner request and reaches no Emby endpoint, so the interim no-op is
/// deliberate until those rows replace it.
#[rstest]
#[case::play_next(GuiLibraryCommand::PlayNext { item_id: "alb-ill".into() })]
#[case::menu_entry(GuiLibraryCommand::MenuEntry { item_id: "alb-ill".into(), index: 0 })]
fn the_interim_library_commands_run_nothing(#[case] command: GuiLibraryCommand) {
    let http = MockHttp::new();
    let (mut test, cmd_rx) = music_core(&http);

    send_library(&mut test, command);

    assert!(cmd_rx.try_iter().next().is_none(), "no Owner request");
    assert_eq!(http.request_count(), 0, "no Emby request");
}
