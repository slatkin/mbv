//! The row model moved with `MediaSemanticState` from the render painter
//! (extract-client-core row 4.3): the assertions are unchanged from their
//! `mbv-components` home.

use mbv_ui_model::media_list::MediaSemanticState;
use mbv_ui_model::media_list::{
    ActiveProgress, MediaKind, MediaListRow, zebra_group_member_striped, zebra_group_start,
    zebra_striped,
};
use rstest::rstest;

/// The one canonical state derivation every list uses: `played` wins, a
/// positive resume position yields `Active` with the bounded percentage (no
/// percentage when the runtime is unknown), and no position is `Ordinary`.
#[test]
fn from_progress_is_the_one_state_derivation() {
    assert_eq!(
        MediaSemanticState::from_progress(true, 50, 100),
        MediaSemanticState::Active {
            progress: Some(ActiveProgress::new(50)),
        },
        "a resume position yields Active even when played"
    );
    assert_eq!(
        MediaSemanticState::from_progress(true, 0, 100),
        MediaSemanticState::Played,
        "played without a resume position yields Played"
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 25, 100),
        MediaSemanticState::Active {
            progress: Some(ActiveProgress::new(25)),
        }
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 50, 0),
        MediaSemanticState::Active { progress: None },
        "an unknown runtime keeps Active without a percentage"
    );
    assert_eq!(
        MediaSemanticState::from_progress(false, 0, 100),
        MediaSemanticState::Ordinary
    );
}

/// The one progress constructor: `ActiveProgress::of_ticks` floors the played
/// fraction, bounds it at 100, and names no percentage for a non-positive
/// position or duration (#995).
#[rstest]
#[case(0, 100, None)]
#[case(20, 0, None)]
#[case(1, 3, Some(ActiveProgress::new(33)))]
#[case(150, 100, Some(ActiveProgress::new(100)))]
#[case(i64::MAX, 1, Some(ActiveProgress::new(100)))]
fn of_ticks_floors_and_bounds_the_percentage(
    #[case] position_ticks: i64,
    #[case] duration_ticks: i64,
    #[case] expected: Option<ActiveProgress>,
) {
    assert_eq!(
        ActiveProgress::of_ticks(position_ticks, duration_ticks),
        expected
    );
}

/// The podcast callers convert their seconds to ticks before the division, so
/// an exact boundary keeps the true percentage: 20.4 s of 60 s is 34, where
/// the f64 division floors to 33 (#995).
#[test]
fn of_ticks_corrects_the_podcast_seconds_boundary() {
    let position_ticks = mbv_emby_model::seconds_to_ticks(20.4);
    let duration_ticks = mbv_emby_model::seconds_to_ticks(60.0);
    assert_eq!(
        ActiveProgress::of_ticks(position_ticks, duration_ticks).map(ActiveProgress::percent),
        Some(34)
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
        MediaSemanticState::Active {
            progress: Some(ActiveProgress::new(50)),
        }
    );
    assert_eq!(
        MediaSemanticState::from_queue_item(&mbv_queue::QueueItem::Emby(Box::new(film))),
        MediaSemanticState::Active {
            progress: Some(ActiveProgress::new(50)),
        }
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
