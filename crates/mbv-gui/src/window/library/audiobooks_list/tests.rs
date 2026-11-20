//! Headless tests of the Audiobooks list's row model (D16): the contracts of
//! `audiobooks-list.test.ts` — the letter headings over each run of the
//! catalog's author sort keys, the empty catalog, the rows' geometry over
//! the book rows only, and the A–Z rail over the author sort key with the
//! letters no row carries disabled. The model is plain data, so the geometry
//! is asserted without a layout.

use super::*;
use crate::bridge::frame::library::{GuiRowArtFrame, GuiRowMediaKindFrame};

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
        sort_letter,
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

/// The painted row's kind, as the TypeScript test's `kind` names it.
fn kind_of(row: &AudiobooksListRow) -> &'static str {
    match row {
        AudiobooksListRow::Heading { .. } => "heading",
        AudiobooksListRow::Item(_) => "item",
    }
}

/// The row's heading letter, `None` on every other kind.
fn heading_letter(row: &AudiobooksListRow) -> Option<char> {
    match row {
        AudiobooksListRow::Heading { letter, .. } => Some(*letter),
        AudiobooksListRow::Item(_) => None,
    }
}

// A letter heading paints over each run of author sort keys.
#[test]
fn a_letter_heading_paints_over_each_run_of_author_sort_keys() {
    let painted = build_audiobooks_rows(&catalog(), "");

    let kinds: Vec<&str> = painted.iter().map(kind_of).collect();
    assert_eq!(
        kinds,
        [
            "heading", "item", "item", "heading", "item", "heading", "item"
        ]
    );
    let heights: Vec<f32> = painted.iter().map(GeometryRow::height).collect();
    assert_eq!(
        heights,
        [
            AUIB_HEADING_ROW_HEIGHT,
            AUIB_ROW_HEIGHT,
            AUIB_ROW_HEIGHT,
            AUIB_HEADING_ROW_HEIGHT,
            AUIB_ROW_HEIGHT,
            AUIB_HEADING_ROW_HEIGHT,
            AUIB_ROW_HEIGHT,
        ]
    );
    assert_eq!(painted[0].top(), 0.0);
    assert_eq!(painted[1].top(), AUIB_HEADING_ROW_HEIGHT);
    let letters: Vec<Option<char>> = painted.iter().map(heading_letter).collect();
    assert_eq!(
        letters,
        [Some('A'), None, None, Some('B'), None, Some('#'), None]
    );
}

// An empty catalog paints no row.
#[test]
fn an_empty_catalog_paints_no_row() {
    assert_eq!(build_audiobooks_rows(&[], ""), Vec::new());
}

// The rail holds the first book under each letter and disables the rest.
#[test]
fn the_rail_holds_the_first_book_under_each_letter_and_disables_the_rest() {
    let rail = audiobooks_letter_rail(&build_audiobooks_rows(&catalog(), ""));
    let letter = |letter: char| {
        rail.iter()
            .find(|entry| entry.letter == letter)
            .and_then(|entry| entry.row_id.clone())
    };

    assert_eq!(letter('A'), Some("b1".to_owned()));
    assert_eq!(letter('B'), Some("b3".to_owned()));
    assert_eq!(letter('C'), None);
}
