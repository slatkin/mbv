//! The Library panel's contracts (add-gui-library-panel rows 3.1, 3.4): every
//! command family reaches the `App` effect it names, and the Library section's
//! content revision and Hero artwork reach the frame from the image cache. The
//! Emby answers come from the in-memory mock transport, so no request leaves
//! the process, and no live Owner, Service, or webview runs (design D13).
//!
//! This module holds the panel's shared support (the stubs, the command
//! sender, the Owner-command readers, and the frame readers) and declares one
//! child module per test family: the destination and its focus handoff
//! (`destination`), the stored selection and detail (`selection`), the queue
//! commands (`playback`), the item menus (`menu`), and the frame's content
//! (`frame`). The child files resolve under `tests/core_thread/library/` (one
//! binary per crate; no `mod.rs`).

use std::sync::mpsc::Receiver;

use mbv::{ClientTickHost, stub_app_with_queue_and_cmd_rx};
use mbv_ctrl::CtrlCmd;
use mbv_gui::bridge::frame::GuiFrame;
use mbv_gui::bridge::frame::library::{GuiLibraryArt, GuiLibraryFrame};
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::QueueSource;

use crate::{TestCore, test_core_with, test_emby_client};

mod destination;
mod emby;
mod frame;
mod menu;
mod playback;
mod selection;
mod shows;

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
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");
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
/// A core over the grouped Music library with Hip-Hop's album list already in
/// the GUI store, so a frame test reads the projection with no request and no
/// fetch behind it: the group is loaded before the selection names it, and the
/// stub's image cache stays off until a test enables it.
fn seeded_music_core(http: &MockHttp) -> (TestCore, Receiver<CtrlCmd>) {
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(0);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(http));
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");
    core.seed_gui_music_albums_for_test(
        0,
        "grp-hiphop",
        &[
            ("alb-ill", "Licensed to Ill"),
            ("alb-paul", "Paul's Boutique"),
        ],
    );
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
