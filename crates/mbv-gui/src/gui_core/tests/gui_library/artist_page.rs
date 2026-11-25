//! Tests of the artist page's commands and its Music ensure step
//! (gui-music-artist-page design D2, D3, D4, D8): `ShowArtist` stores the
//! page pick and starts the album track loads, `PlayArtistTrack` builds the
//! sequence that spans the page's albums in order and starts at the track,
//! and the destination's ensure pass requests the page's uncached albums
//! only while a page shows. The queue replacement is observed on the Owner's
//! command channel, so no test reads an optimistic Client write.

use super::*;
use rstest::rstest;

/// The item ids and `start_idx` of the most recent `UnifiedQueueReplace`
/// sent to the Owner. Drains the command channel; call once per assertion
/// point.
fn sent_replacement(
    cmd_rx: &std::sync::mpsc::Receiver<mbv_ctrl::CtrlCmd>,
) -> Option<(Vec<String>, Option<usize>)> {
    let mut sent = None;
    for command in cmd_rx.try_iter() {
        if let mbv_ctrl::CtrlCmd::UnifiedQueueReplace {
            items, start_idx, ..
        } = command
        {
            sent = Some((
                items.iter().map(|item| item.id().to_string()).collect(),
                start_idx,
            ));
        }
    }
    sent
}

/// `ShowArtist` stores the library's page pick and starts the track loads of
/// the artist's albums once, and none for a cached album
/// (gui-music-artist-page design D1, D3): the loads are the core's bounded,
/// deduplicated fan-out, so a repeat of the command starts nothing the first
/// one already armed, and an album the cache holds never enters it.
#[test]
fn show_artist_stores_the_page_pick_and_starts_the_uncached_album_loads() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        Vec::new(),
    );
    app.seed_album_track_items_for_test(
        "alb-ill",
        vec![cached_track("trk-1", "Rhymin & Stealin", 1, 248)],
    );
    let artist_key = gui_music_projection(&mut app, 0).artists[0]
        .artist_key
        .clone();

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowArtist {
            lib_idx: 0,
            artist_key: artist_key.clone(),
        },
    );
    // The repeat proves the loads start once: every album the first command
    // armed is loading or cached, so the second command arms nothing again.
    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::ShowArtist {
            lib_idx: 0,
            artist_key: artist_key.clone(),
        },
    );

    assert_eq!(
        crate::gui_core::test_support::stub_gui_state(|gui| gui
            .music_artist_pages
            .get(&0)
            .cloned()),
        Some(artist_key),
        "the library's page pick is the pressed artist"
    );
    assert!(
        app.album_tracks_loading("alb-paul"),
        "the uncached album's track load started"
    );
    assert!(
        !app.album_tracks_loading("alb-ill"),
        "the cached album starts no load"
    );
}

/// `PlayArtistTrack` builds the sequence that spans the page's albums in
/// order and starts at the track (gui-music-artist-page design D4, D8; the
/// spec scenario "A track is played from the artist page"): the queue
/// replacement holds every album's cached tracks concatenated in the albums'
/// order — the picked track, the rest of its album, and the next album's
/// tracks in order behind it — and playback starts at the picked track's
/// position.
#[test]
fn play_artist_track_queues_the_spanning_sequence_from_the_picked_track() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(Vec::new());
    app.seed_album_track_items_for_test(
        "alb-ill",
        vec![cached_track("trk-1", "Rhymin & Stealin", 1, 248)],
    );
    app.seed_album_track_items_for_test(
        "alb-paul",
        vec![
            cached_track("trk-2", "Shake Your Rump", 1, 195),
            cached_track("trk-3", "Egg Man", 2, 178),
        ],
    );
    app.seed_album_track_items_for_test(
        "alb-check",
        vec![
            cached_track("trk-4", "The Sounds of Science", 1, 240),
            cached_track("trk-5", "3-Minute Rule", 2, 210),
        ],
    );

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::PlayArtistTrack {
            album_ids: vec![
                "alb-ill".to_string(),
                "alb-paul".to_string(),
                "alb-check".to_string(),
            ],
            track_id: "trk-2".to_string(),
        },
    );

    let (ids, start) = sent_replacement(&cmd_rx).expect("the play replaced the queue");
    assert_eq!(
        ids,
        ["trk-1", "trk-2", "trk-3", "trk-4", "trk-5"],
        "the sequence spans the albums in the page's order"
    );
    assert_eq!(start, Some(1), "playback starts at the picked track");
    assert_eq!(
        ids[1..],
        ["trk-2", "trk-3", "trk-4", "trk-5"],
        "the picked track, the rest of its album, and the next album's tracks, in order"
    );
}

/// `PlayArtistTrack` skips an album the cache does not hold and still plays
/// (gui-music-artist-page design D4; correction of the P1 early-return):
/// the not-yet-loaded album adds nothing to the sequence, and playback
/// starts at the picked track among the cached albums' tracks.
#[test]
fn play_artist_track_skips_an_uncached_album_and_still_plays() {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let (mut app, cmd_rx) = make_local_daemon_app_stub_with_cmd_rx(Vec::new());
    app.seed_album_track_items_for_test(
        "alb-ill",
        vec![cached_track("trk-1", "Rhymin & Stealin", 1, 248)],
    );
    app.seed_album_track_items_for_test(
        "alb-check",
        vec![cached_track("trk-4", "The Sounds of Science", 1, 240)],
    );

    apply_gui_library_command(
        &mut app,
        GuiLibraryCommand::PlayArtistTrack {
            album_ids: vec![
                "alb-ill".to_string(),
                "alb-paul".to_string(),
                "alb-check".to_string(),
            ],
            track_id: "trk-4".to_string(),
        },
    );

    let (ids, start) = sent_replacement(&cmd_rx).expect("the play replaced the queue");
    assert_eq!(
        ids,
        ["trk-1", "trk-4"],
        "the uncached album adds nothing to the sequence"
    );
    assert_eq!(start, Some(1), "playback starts at the picked track");
}

/// The Music destination's ensure pass requests the shown artist page's
/// uncached albums, and nothing while no page shows (gui-music-artist-page
/// design D3): the dependents step is the page's loader, so a page a
/// collection change widened picks up its new albums, and an album the cache
/// already holds enters no load.
#[rstest]
#[case::with_a_page(true)]
#[case::without_a_page(false)]
fn the_ensure_pass_loads_the_pages_uncached_albums_only_while_a_page_shows(
    #[case] with_page: bool,
) {
    let _guard = mbv_client_core::config::TestStateDirGuard::new();
    let mut app = app_with_stored_groups(
        vec![
            stored_album("alb-ill", "Licensed to Ill", "Beastie Boys", 1986),
            stored_album("alb-paul", "Paul's Boutique", "Beastie Boys", 1989),
        ],
        Vec::new(),
    );
    app.seed_album_track_items_for_test(
        "alb-ill",
        vec![cached_track("trk-1", "Rhymin & Stealin", 1, 248)],
    );
    if with_page {
        let artist_key = gui_music_projection(&mut app, 0).artists[0]
            .artist_key
            .clone();
        crate::gui_core::test_support::stub_gui_state(|gui| {
            gui.music_artist_pages.insert(0, artist_key);
        });
    }

    crate::gui_core::test_support::ensure_gui_destination(
        &mut app,
        GuiDestinationKind::Music { lib_idx: 0 },
    );

    assert_eq!(
        app.album_tracks_loading("alb-paul"),
        with_page,
        "the page's uncached album loads exactly while a page shows"
    );
    assert!(
        !app.album_tracks_loading("alb-ill"),
        "the cached album enters no load"
    );
}
