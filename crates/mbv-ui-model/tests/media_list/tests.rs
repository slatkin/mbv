//! The row model moved with `MediaSemanticState` from the render painter
//! (add-gui-front-end row 4.3): the assertions are unchanged from their
//! `mbv-components` home.

use mbv_ui_model::media_list::MediaSemanticState;

/// The one canonical state derivation every list uses: `played` wins, a
/// positive resume position yields `Active` with the bounded percentage (no
/// percentage when the runtime is unknown), and no position is `Ordinary`.
#[test]
fn from_progress_is_the_one_state_derivation() {
    assert_eq!(
        MediaSemanticState::from_progress(true, 50, 100),
        MediaSemanticState::active(Some(50)),
        "a resume position yields Active even when played"
    );
    assert_eq!(
        MediaSemanticState::from_progress(true, 0, 100),
        MediaSemanticState::Played,
        "played without a resume position yields Played"
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 25, 100),
        MediaSemanticState::active(Some(25))
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 50, 0),
        MediaSemanticState::active(None),
        "an unknown runtime keeps Active without a percentage"
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 0, 100),
        MediaSemanticState::Ordinary
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 150, 100),
        MediaSemanticState::active(Some(100)),
        "the percentage is bounded at 100"
    );
}

/// The item-level derivation: music (track, album, artist) never carries its
/// stored played/resume facts into a row — music is fire-and-forget, so the
/// row stays `Ordinary`. Every other item keeps the canonical derivation.
#[test]
fn item_level_derivation_makes_music_rows_ordinary() {
    let played_music = |item_type: &str| {
        let mut item = mbv_emby_model::test_support::make_item("Music", item_type);
        item.played = true;
        item.runtime_ticks = 1000;
        item.playback_position_ticks = 500;
        item
    };
    for item_type in ["Audio", "MusicAlbum", "MusicArtist"] {
        let item = played_music(item_type);
        assert_eq!(
            MediaSemanticState::from_emby(&item),
            MediaSemanticState::Ordinary,
            "a {item_type} row ignores both its played flag and its resume position"
        );
        assert_eq!(
            MediaSemanticState::from_queue_item(&mbv_queue::QueueItem::Emby(Box::new(item))),
            MediaSemanticState::Ordinary,
            "the queue's music row is ordinary too"
        );
    }

    let mut film = mbv_emby_model::test_support::make_item("The Film", "Movie");
    film.played = true;
    assert_eq!(
        MediaSemanticState::from_emby(&film),
        MediaSemanticState::Played,
        "non-music keeps the canonical derivation"
    );
    film.played = false;
    film.runtime_ticks = 1000;
    film.playback_position_ticks = 500;
    assert_eq!(
        MediaSemanticState::from_emby(&film),
        MediaSemanticState::active(Some(50))
    );
    assert_eq!(
        MediaSemanticState::from_queue_item(&mbv_queue::QueueItem::Emby(Box::new(film))),
        MediaSemanticState::active(Some(50))
    );
}
