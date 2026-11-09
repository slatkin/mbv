//! The queue commands' contracts (add-gui-library-panel design D8): what the
//! Owner is asked for by a play, an add, a shuffle, a play-next, and an item
//! menu entry — the replacement or the append, its source, and its items.

use mbv::ClientTickHost;
use mbv::stub_app_with_queue_and_cmd_rx;
use mbv_gui::bridge::library::GuiLibraryCommand;
use mbv_net::mock_http::MockHttp;
use mbv_queue::QueueSource;

use crate::{test_core_with, test_emby_client};

use super::{TRACKS, appended_ids, music_core, replacement, send_library};

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
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");
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

/// The explicit Library shuffle replaces the queue directly (the
/// replace-confirm behaviour change): over a populated queue it replaces it
/// without confirming, exactly as the Library play does.
#[test]
fn the_shuffle_artist_command_replaces_a_populated_queue_without_a_confirm() {
    let http = MockHttp::new();
    http.respond_repeated(200, TRACKS, 2);
    let (app, cmd_rx) = stub_app_with_queue_and_cmd_rx(1);
    let mut test = test_core_with(app);
    let core = test.core.app_mut();
    core.seed_gui_music_library_for_test(test_emby_client(&http));
    core.seed_gui_library_groups_for_test(0, "grp-hiphop");

    send_library(
        &mut test,
        GuiLibraryCommand::ShuffleArtist {
            album_ids: vec!["alb-ill".into()],
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(
        ids.len(),
        2,
        "both albums' shuffled tracks reached the Owner"
    );
    assert_eq!(source, QueueSource::Shuffle);
    assert!(
        test.core.app_mut().pending_overlay_is_none_for_test(),
        "no confirm is pending"
    );
}

/// A menu entry executes for the item its id names, never for the item the
/// TUI's library cursor rests on (the id-addressed path, add-gui-library-panel
/// row 3.6): the album level's cursor rests on the first album while the
/// entry names a track of the second album, and the Owner receives that
/// track's own album's cached tracks.
#[test]
fn a_menu_entry_executes_for_its_named_item_not_the_cursor_item() {
    let http = MockHttp::new();
    let (mut test, cmd_rx) = music_core(&http);
    // The cursor's album keeps its own cached tracks; the entry names the
    // second album's track, so the two album expansions cannot be confused.
    test.core.app_mut().seed_gui_album_tracks_for_test(
        "alb-ill",
        &[
            ("ltrk-1", "Rhymin & Stealin", 1, 248),
            ("ltrk-2", "The New Style", 2, 275),
        ],
    );
    test.core.app_mut().seed_gui_album_tracks_for_test(
        "alb-paul",
        &[
            ("ptrk-3", "Shake Your Rump", 1, 195),
            ("ptrk-4", "Johnny Ryall", 2, 194),
        ],
    );
    test.core.app_mut().set_autoload_for_test(true);

    send_library(&mut test, GuiLibraryCommand::TakeFocus);
    send_library(
        &mut test,
        GuiLibraryCommand::MenuEntry {
            item_id: "ptrk-3".into(),
            index: 0,
        },
    );

    let (source, ids) = replacement(&cmd_rx);
    assert_eq!(ids, ["ptrk-3", "ptrk-4"], "the named track's album played");
    assert_eq!(
        source,
        QueueSource::Album,
        "the autoload play names the album source"
    );
}
