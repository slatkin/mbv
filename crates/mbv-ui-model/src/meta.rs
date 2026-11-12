//! Typed segments for one Hero meta row (design D14).
//!
//! A row that joins values of several data types is built as segments where
//! it is built, never split later in a window. The TUI joins the segment text
//! back into one line. The GUI paints each segment in its kind's colour.

/// The data type of one meta segment. Colour carries the type in the GUI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaKind {
    /// Words, names, and the separators between segments.
    Plain,
    /// A runtime or a duration.
    Time,
    /// A year, a year range, or a release date.
    Date,
    /// A number of items.
    Count,
    /// A section label.
    Label,
    /// A parent or series title.
    Secondary,
    /// A role or a character.
    Role,
}

/// One typed run of text inside a meta row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaSegment {
    pub text: String,
    pub kind: MetaKind,
}

impl MetaSegment {
    #[must_use]
    pub fn new(text: impl Into<String>, kind: MetaKind) -> Self {
        Self {
            text: text.into(),
            kind,
        }
    }

    /// A `Plain` segment (words, names, separators).
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, MetaKind::Plain)
    }
}

/// Joins non-empty segments into one row, with `separator` as a `Plain`
/// segment between each pair. Empty segments are skipped.
#[must_use]
pub fn join_segments(
    segments: impl IntoIterator<Item = MetaSegment>,
    separator: &str,
) -> Vec<MetaSegment> {
    let mut row = Vec::new();
    for segment in segments.into_iter().filter(|s| !s.text.is_empty()) {
        if !row.is_empty() {
            row.push(MetaSegment::plain(separator));
        }
        row.push(segment);
    }
    row
}

/// The text of one meta row, as the TUI paints it.
#[must_use]
pub fn meta_row_text(row: &[MetaSegment]) -> String {
    row.iter().map(|segment| segment.text.as_str()).collect()
}

/// The text of every meta row, in order (see [`meta_row_text`]).
#[must_use]
pub fn meta_rows_text(rows: &[Vec<MetaSegment>]) -> Vec<String> {
    rows.iter().map(|row| meta_row_text(row)).collect()
}
