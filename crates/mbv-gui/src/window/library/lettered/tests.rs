//! Headless tests of the lettered destinations' one row model (design D4):
//! the contracts of `emby-list.test.ts` and `audiobooks-list.test.ts` — the
//! letter headings over a name-sorted list with its A–Z rail, and the flat
//! date-sorted rows with no heading and no rail (gui-flat-list-sort design
//! D8). A contract the destinations share runs over both adapters; the
//! feed-group mode's date gutter is Emby's alone. The model is plain data, so
//! the geometry is asserted without a layout.

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

/// One book row of the catalog: its rail letter is the heading it paints
/// under.
fn book(id: &str, title: &str, sort_letter: char) -> GuiAudiobooksRowFrame {
    GuiAudiobooksRowFrame {
        art: GuiRowArtFrame::NoSource,
        media: GuiRowMediaKindFrame::Audio,
        id: id.to_owned(),
        title: title.to_owned(),
        subtitle: None,
        duration: String::new(),
        sort_letter: Some(sort_letter),
    }
}

/// The catalog fixture: two books under `A`, one under `B`, and one whose
/// author sort key leads with a digit, which paints under `#`.
fn catalog() -> Vec<GuiAudiobooksRowFrame> {
    vec![
        book("b1", "Aftermath", 'A'),
        book("b2", "Audition", 'A'),
        book("b3", "Blindsight", 'B'),
        book("b4", "1984", '#'),
    ]
}

/// The painted row's kind.
fn kind_of<Row>(row: &LetteredRow<LetteredItemRow<Row>>) -> &'static str {
    match row {
        LetteredRow::Heading(_) => "heading",
        LetteredRow::Item(_) => "item",
    }
}

/// The row's heading letter, `None` on every other kind.
fn heading_letter<Row>(row: &LetteredRow<LetteredItemRow<Row>>) -> Option<char> {
    match row {
        LetteredRow::Heading(heading) => Some(heading.letter),
        LetteredRow::Item(_) => None,
    }
}

/// The appends the shared assertions for a name-sorted list: one heading per
/// run of the same rail letter, the headings' 24 px band above each run, and
/// the item rows at the shared row height.
fn assert_letters_over_runs<Row: CarriedRow>(painted: &[LetteredRow<LetteredItemRow<Row>>]) {
    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        [
            "heading", "item", "item", "heading", "item", "heading", "item"
        ]
    );
    let letters: Vec<Option<char>> = painted.iter().map(heading_letter).collect();
    assert_eq!(
        letters,
        [Some('A'), None, None, Some('B'), None, Some('#'), None]
    );
    assert_eq!(painted[0].top(), 0.0);
    assert_eq!(painted[1].top(), LETTER_ROW_HEIGHT);
    let heights: Vec<f32> = painted
        .iter()
        .map(|row| match row {
            LetteredRow::Heading(heading) => heading.height,
            LetteredRow::Item(item) => item.height,
        })
        .collect();
    assert_eq!(
        heights,
        [
            LETTER_ROW_HEIGHT,
            LIBRARY_ROW_HEIGHT,
            LIBRARY_ROW_HEIGHT,
            LETTER_ROW_HEIGHT,
            LIBRARY_ROW_HEIGHT,
            LETTER_ROW_HEIGHT,
            LIBRARY_ROW_HEIGHT,
        ]
    );
}

/// The rail entry of one letter, with the first row under it.
fn rail_row<Row: CarriedRow>(
    rows: &[LetteredRow<LetteredItemRow<Row>>],
    letter: char,
) -> Option<String> {
    lettered_rail(rows)
        .iter()
        .find(|entry| entry.letter == letter)
        .and_then(|entry| entry.row_id.clone())
}

// A name-sorted list paints a letter heading over each run of rows, and the
// rail holds the first row under each letter.
#[test]
fn a_name_sorted_list_paints_a_letter_heading_over_each_run_of_rows() {
    assert_letters_over_runs(&build_rows::<EmbyList>(&movies(), ""));
    assert_letters_over_runs(&build_rows::<AudiobooksList>(&catalog(), ""));
}

// The rail holds the first row under each letter and disables the rest.
#[test]
fn the_rail_holds_the_first_row_under_each_letter_and_disables_the_rest() {
    let emby = build_rows::<EmbyList>(&movies(), "");
    let books = build_rows::<AudiobooksList>(&catalog(), "");

    assert_eq!(rail_row(&emby, 'A'), Some("m1".to_owned()));
    assert_eq!(rail_row(&emby, 'B'), Some("m3".to_owned()));
    assert_eq!(rail_row(&emby, 'C'), None);
    assert_eq!(rail_row(&books, 'A'), Some("b1".to_owned()));
    assert_eq!(rail_row(&books, 'B'), Some("b3".to_owned()));
    assert_eq!(rail_row(&books, 'C'), None);
}

// An empty catalog paints no row.
#[test]
fn an_empty_catalog_paints_no_row() {
    assert_eq!(build_rows::<AudiobooksList>(&[], ""), Vec::new());
}

// A date-sorted list's rows carry no letter, so they paint flat with their
// date and no heading.
#[test]
fn a_date_sorted_list_paints_flat_rows_with_their_date_and_no_heading() {
    let painted = build_rows::<EmbyList>(
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
    assert_eq!(painted[1].top(), LIBRARY_ROW_HEIGHT);
}

// A date-sorted list's rows carry no letter, so every rail letter is
// disabled and no rail paints.
#[test]
fn a_date_sorted_lists_rows_carry_no_letter_so_every_rail_letter_is_disabled() {
    let rows = build_rows::<EmbyList>(&[feed("f1", "Family", "1998")], "");
    let rail = lettered_rail(&rows);

    assert!(!carries_letters(&rows));

    assert!(rail.iter().all(|letter| letter.row_id.is_none()));
}
