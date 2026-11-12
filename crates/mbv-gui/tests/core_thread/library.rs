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
//! commands (`playback`), the item menus (`menu`), the Search field
//! (`search`), and the frame's content (`frame`). The child files resolve
//! under `tests/core_thread/library/` (one binary per crate; no `mod.rs`).

use std::sync::mpsc::Receiver;

use crate::Held;
use mbv::{ClientTickHost, stub_app_with_queue_and_cmd_rx};
use mbv_ctrl::CtrlCmd;
use mbv_gui::art::{GuiArtRef, PLACEHOLDER_PATH, art_url};
use mbv_gui::bridge::frame::library::{
    GuiDestinationList, GuiLibraryDetail, GuiLibraryList, GuiMusicDetail, GuiMusicList,
};
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_gui::core_thread::CoreInput;
use mbv_net::mock_http::MockHttp;
use mbv_queue::QueueSource;

use crate::{TestCore, test_core_with, test_emby_client};

mod audiobooks;
mod destination;
mod emby;
mod feeds;
mod frame;
mod menu;
mod playback;
mod podcasts;
mod search;
mod selection;
mod shows;

/// The bound on one pump round's wait for the next library completion
/// (design D13): a safety net that turns a fetch pipeline that stops
/// answering into a failure. A passing pump never spends it.
const LIB_EVENT_BOUND: std::time::Duration = std::time::Duration::from_secs(30);

/// Pump GUI ticks until the newest frame satisfies `want`, waiting for the
/// next library completion instead of sleeping between steps (design D13):
/// each round steps, checks the newest emitted frame, then blocks until one
/// more fetch completion lands on the lib channel — the event the next
/// step's drain applies. The wait blocks for the outcome, so the fetch
/// workers never starve behind a spinning test and a passing pump spends no
/// real time; the bound only fires when the pipeline has stopped, and the
/// panic then names `what`.
fn pump_until(test: &mut TestCore, want: impl Fn(&Held) -> bool, what: &str) {
    loop {
        assert!(test.core.step(), "the core thread never quits mid-drain");
        if want(&test.held()) {
            return;
        }
        assert!(
            test.core
                .app_mut()
                .drain_lib_event_blocking_for_test(LIB_EVENT_BOUND),
            "no library completion landed while waiting for {what}"
        );
        // The next step drains the requeued completion, which marks its sections
        // (design D3 rule 2), and the wake makes that step run at once.
        test.waker.wake();
    }
}

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
fn send_library_frame(test: &mut TestCore, command: GuiLibraryCommand) -> Held {
    send_library(test, command);
    test.held()
}

/// The Library list section a frame carries (design D8).
fn list_of(frame: &Held) -> Option<GuiLibraryList> {
    frame.library_list().cloned()
}

/// The Library detail section a frame carries (design D8).
fn detail_of(frame: &Held) -> Option<GuiLibraryDetail> {
    frame.library_detail().cloned()
}

/// The Music list part a frame carries, `None` while another destination is
/// active.
fn music_list(frame: &Held) -> Option<GuiMusicList> {
    let GuiDestinationList::Music(list) = list_of(frame)?.destination else {
        return None;
    };
    Some(list)
}

/// The Music detail part a frame carries, `None` while another destination is
/// active.
fn music_detail(frame: &Held) -> Option<GuiMusicDetail> {
    let GuiLibraryDetail::Music(detail) = detail_of(frame)? else {
        return None;
    };
    Some(detail)
}

/// The Hero's artwork value of a frame that carries the Music detail.
fn hero_art(frame: &Held) -> GuiArtRef {
    music_detail(frame)
        .and_then(|detail| detail.art)
        .expect("the frame carries the Hero's art")
}

/// The Hero paints the bundled placeholder's `mbv-art` reference (design D7).
fn assert_placeholder(art: GuiArtRef) {
    let GuiArtRef::Placeholder { src } = art else {
        panic!("the Hero paints the placeholder, not {art:?}");
    };
    assert_eq!(src, art_url(PLACEHOLDER_PATH));
}

/// The Hero paints the cached art's `mbv-art` card reference (design D7).
fn assert_image(art: GuiArtRef) {
    let GuiArtRef::Image { src } = art else {
        panic!("the Hero paints the cached art, not {art:?}");
    };
    assert!(src.starts_with("mbv-art://localhost/card/"), "{src}");
}
