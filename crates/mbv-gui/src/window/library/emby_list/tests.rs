//! Headless tests of the plain Emby-library list's row model (D16): the
//! contracts of `emby-list.test.ts` — the letter headings over a name-sorted
//! list with its A–Z rail, and the flat date-sorted rows with no heading and
//! no rail (gui-flat-list-sort design D8). The model is plain data, so the
//! geometry is asserted without a layout.

use super::*;
use crate::bridge::frame::library::{GuiEmbyRowKindFrame, GuiRowArtFrame, GuiRowMediaKindFrame};
use crate::window::library::list::{LETTER_ROW_HEIGHT, carries_letters, lettered_rail};

/// One plain-mode row: the name-sorted library's row under its rail letter.
fn plain(id: &str, title: &str, sort_letter: char) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: Some("1999".to_owned()),
        date: None,
        sort_letter: Some(sort_letter),
        kind: GuiEmbyRowKindFrame::Media,
    }
}

/// One feed-group row: the date-ordered list's row with its year in the
/// gutter.
fn feed(id: &str, title: &str, date: &str) -> GuiEmbyRowFrame {
    GuiEmbyRowFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Video,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        date: Some(date.to_owned()),
        sort_letter: None,
        kind: GuiEmbyRowKindFrame::Media,
    }
}

/// The plain fixture: two `A` movies, one `B` movie, and one with no letter,
/// which paints under `#`.
fn movies() -> Vec<GuiEmbyRowFrame> {
    vec![
        plain("m1", "Alien", 'A'),
        plain("m2", "Amélie", 'A'),
        plain("m3", "Brazil", 'B'),
        plain("m4", "2001", '#'),
    ]
}

/// The painted row's kind.
fn kind_of(row: &LetteredRow<EmbyItemRow>) -> &'static str {
    match row {
        LetteredRow::Heading(_) => "heading",
        LetteredRow::Item(_) => "item",
    }
}

// A name-sorted list paints a letter heading over each run of rows.
#[test]
fn a_name_sorted_list_paints_a_letter_heading_over_each_run_of_rows() {
    let painted = build_emby_rows(&movies(), "");

    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        [
            "heading", "item", "item", "heading", "item", "heading", "item"
        ]
    );
    assert_eq!(painted[0].top(), 0.0);
    assert_eq!(painted[1].top(), LETTER_ROW_HEIGHT);
    let heights: Vec<f32> = painted.iter().map(GeometryRow::height).collect();
    assert!(heights.contains(&EMBY_ROW_HEIGHT));
}

// A date-sorted list's rows carry no letter, so they paint flat with their
// date and no heading.
#[test]
fn a_date_sorted_list_paints_flat_rows_with_their_date_and_no_heading() {
    let painted = build_emby_rows(
        &[feed("f1", "Family", "1998"), feed("f2", "Fargo", "1996")],
        "",
    );

    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(kinds, ["item", "item"]);
    let dates: Vec<Option<&str>> = painted
        .iter()
        .map(|row| match row {
            LetteredRow::Item(item) => item.row.date.as_deref(),
            LetteredRow::Heading(_) => None,
        })
        .collect();
    assert_eq!(dates, [Some("1998"), Some("1996")]);
    assert_eq!(painted[1].top(), EMBY_ROW_HEIGHT);
}

// The rail holds the first row under each letter and disables the rest.
#[test]
fn the_rail_holds_the_first_row_under_each_letter_and_disables_the_rest() {
    let rail = lettered_rail(&build_emby_rows(&movies(), ""));
    let letter = |letter: char| {
        rail.iter()
            .find(|entry| entry.letter == letter)
            .and_then(|entry| entry.row_id.clone())
    };

    assert_eq!(letter('A'), Some("m1".to_owned()));
    assert_eq!(letter('B'), Some("m3".to_owned()));
    assert_eq!(letter('C'), None);
}

// A date-sorted list's rows carry no letter, so every rail letter is
// disabled and no rail paints.
#[test]
fn a_date_sorted_lists_rows_carry_no_letter_so_every_rail_letter_is_disabled() {
    let rows = build_emby_rows(&[feed("f1", "Family", "1998")], "");
    let rail = lettered_rail(&rows);

    assert!(!carries_letters(&rows));

    assert!(rail.iter().all(|letter| letter.row_id.is_none()));
}
