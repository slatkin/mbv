//! The row model moved with `MediaSemanticState` from the render painter
//! (extract-client-core row 4.3): the assertions are unchanged from their
//! `mbv-components` home.

use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::media_list::{
    MediaKind, MediaListRow, zebra_group_member_striped, zebra_group_start, zebra_striped,
};

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

/// An ordinary playable item row, the row kind the zebra walk fills.
fn item_row<'a>() -> MediaListRow<&'a str> {
    MediaListRow::Item {
        target: "row",
        primary: String::new(),
        secondary: None,
        trailing: None,
        duration: None,
        kind: MediaKind::Media,
        semantic_state: MediaSemanticState::Ordinary,
    }
}

/// The one zebra rule both painters read (`mbv-render`'s wide list and the
/// GUI queue frame): a grouped list alternates within each group from the
/// group's first member — the row below its heading, whose heading and
/// spacer sit outside the fill and never flip the running state — and an
/// ungrouped list flips per item row from the caller's viewport-offset
/// seed.
#[test]
fn zebra_striped_walks_the_shared_rule() {
    let rows = [
        item_row(),
        MediaListRow::Heading {
            text: "album".to_owned(),
        },
        item_row(),
        MediaListRow::Spacer,
        item_row(),
        item_row(),
    ];
    let at = |index: usize| &rows[index];

    // The grouped walk: the group's members start below the heading, and
    // every second member reverts to the surface fill. The heading and the
    // spacer report nothing and leave the running fill untouched.
    assert_eq!(
        zebra_group_start(&at, 0),
        0,
        "an item above every heading opens the first group at the list's top"
    );
    assert_eq!(
        zebra_group_start(&at, 2),
        2,
        "a group's members start at the row below its heading"
    );
    assert!(
        zebra_group_member_striped(2, 2),
        "the group's first member takes the fill"
    );
    assert!(
        !zebra_group_member_striped(5, 2),
        "the group's second member after another reverts to the surface fill"
    );
    let mut striped = false;
    assert!(zebra_striped(&rows[0], 0, at, true, &mut striped));
    assert!(!zebra_striped(&rows[1], 1, at, true, &mut striped));
    assert!(zebra_striped(&rows[2], 2, at, true, &mut striped));
    assert!(!zebra_striped(&rows[3], 3, at, true, &mut striped));
    assert!(zebra_striped(&rows[4], 4, at, true, &mut striped));
    assert!(!zebra_striped(&rows[5], 5, at, true, &mut striped));
    assert!(!striped, "only item rows flip the running fill");

    // The ungrouped walk: the running state flips per item row, seeded by
    // the caller's viewport offset.
    let mut striped = false;
    assert!(!zebra_striped(&rows[0], 0, at, false, &mut striped));
    assert!(zebra_striped(&rows[2], 2, at, false, &mut striped));
    assert!(!zebra_striped(&rows[4], 4, at, false, &mut striped));
}
