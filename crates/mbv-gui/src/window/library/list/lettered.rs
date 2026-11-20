//! The lettered-row builder the flat Library lists share (gui-flat-list-sort
//! design D8): one letter heading before each run of rows of the same sort
//! letter, after the filter, and flat rows with no heading when the rows carry
//! no letter. The plain and feed-group Emby list, the Audiobooks, Podcasts,
//! and Feeds lists, and the Shows New and Upcoming rows all build here, so the
//! headings and the rail stay one fact the core derives (design D6): the
//! window never re-decides it from the list kind.

use super::geometry::GeometryRow;
use super::{LETTER_ROW_HEIGHT, RailLetter, letter_rail_by};

/// A painted letter heading (design D7, D8): the 24 px band above the first
/// row of its sort letter. A heading is structural: no key acts on it.
#[derive(Debug, Clone, PartialEq)]
pub struct LetterHeading {
    /// The heading's stable key.
    pub id: String,
    /// The heading's top edge from the list's first row.
    pub top: f32,
    /// The heading's painted height.
    pub height: f32,
    /// The heading's rail letter.
    pub letter: char,
}

/// One painted row of a lettered flat list (gui-flat-list-sort design D8): a
/// letter heading, or an item row. A list whose rows carry no letter paints
/// item rows alone.
#[derive(Debug, Clone, PartialEq)]
pub enum LetteredRow<Row> {
    /// A letter heading above the first row of its letter.
    Heading(LetterHeading),
    /// An item row.
    Item(Row),
}

impl<Row: GeometryRow> GeometryRow for LetteredRow<Row> {
    fn id(&self) -> &str {
        match self {
            LetteredRow::Heading(heading) => &heading.id,
            LetteredRow::Item(row) => row.id(),
        }
    }

    fn pointer_row(&self) -> Option<&str> {
        match self {
            LetteredRow::Heading(_) => None,
            LetteredRow::Item(row) => row.pointer_row(),
        }
    }

    fn rows_in_view_id(&self) -> Option<String> {
        match self {
            LetteredRow::Heading(_) => None,
            LetteredRow::Item(row) => row.rows_in_view_id(),
        }
    }

    fn top(&self) -> f32 {
        match self {
            LetteredRow::Heading(heading) => heading.top,
            LetteredRow::Item(row) => row.top(),
        }
    }

    fn height(&self) -> f32 {
        match self {
            LetteredRow::Heading(heading) => heading.height,
            LetteredRow::Item(row) => row.height(),
        }
    }
}

/// The rail letter a list's item row carries (gui-flat-list-sort design D6,
/// D8): `Some` exactly while the list's active sort is a name sort, so a row
/// paints under a heading and the rail anchors it only then.
pub trait SortLetter {
    /// The row's rail letter, `None` under a date sort.
    fn sort_letter(&self) -> Option<char>;
}

/// The painted rows of a lettered flat list (gui-flat-list-sort design D8):
/// the carried rows `keeps` accepts, in their carried order, each `row_height`
/// tall, under one letter heading per run of rows whose `sort_letter_of` gives
/// the same letter. A list whose rows carry no letter builds flat rows with no
/// heading at all. The heading ids take `heading_prefix`, so each list's
/// headings keep their own hook.
#[must_use]
pub fn build_lettered_rows<Item, Row>(
    rows: &[Item],
    row_height: f32,
    heading_prefix: &str,
    keeps: impl Fn(&Item) -> bool,
    sort_letter_of: impl Fn(&Item) -> Option<char>,
    make_row: impl Fn(&Item, f32, f32) -> Row,
) -> Vec<LetteredRow<Row>> {
    let mut built: Vec<LetteredRow<Row>> = Vec::new();
    let mut top = 0.0_f32;
    let mut heading: Option<char> = None;
    for row in rows.iter().filter(|row| keeps(row)) {
        if let Some(letter) = sort_letter_of(row)
            && heading != Some(letter)
        {
            heading = Some(letter);
            built.push(LetteredRow::Heading(LetterHeading {
                id: format!("{heading_prefix}:{letter}"),
                top,
                height: LETTER_ROW_HEIGHT,
                letter,
            }));
            top += LETTER_ROW_HEIGHT;
        }
        built.push(LetteredRow::Item(make_row(row, top, row_height)));
        top += row_height;
    }
    built
}

/// Whether a painted lettered list carries any letter (gui-flat-list-sort
/// design D8): a list whose rows carry letters paints the rail, and a
/// date-sorted list paints none.
#[must_use]
pub fn carries_letters<Row: SortLetter>(rows: &[LetteredRow<Row>]) -> bool {
    rows.iter()
        .any(|row| matches!(row, LetteredRow::Item(item) if item.sort_letter().is_some()))
}

/// The A–Z rail over a lettered flat list's painted rows (design D7, D8):
/// each letter with the first item row under it, so a press scrolls there. A
/// heading is no anchor, so a letter with no row under it is disabled.
#[must_use]
pub fn lettered_rail<Row: SortLetter + GeometryRow>(rows: &[LetteredRow<Row>]) -> Vec<RailLetter> {
    letter_rail_by(rows, |row| match row {
        LetteredRow::Item(item) => item.sort_letter(),
        LetteredRow::Heading(_) => None,
    })
}
