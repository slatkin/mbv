//! Letter grouping for [`WideMediaList`](super::WideMediaList): injects
//! `Heading`/`Spacer` rows into a sorted item list, matching the accepted
//! `render_letter_grouped_rows` contract (bucket derivation, sorted display
//! order, spacer-before-every-heading-except-first).
//!
//! Provider-neutral (design.md line 21): the caller projects each item to a
//! `(sort_str, MediaListRow::Item{..})` pair; no `EmbyItem` enters here.

use super::MediaListRow;
use crate::app::ui_util::{letter_bucket_label, natural_sort_key};

/// Sort `items` by `natural_sort_key(sort_str)`, then emit one
/// `MediaListRow::Heading` per non-empty bucket in bucket order, each preceded
/// by a `MediaListRow::Spacer` except the first. No trailing spacer.
///
/// Bucket mode follows `letter_bucket_label`: `total_count >= 250` yields
/// per-letter buckets, below that three-letter ranges. An active letter filter
/// forces per-letter mode (the reference does this by treating the total as
/// `usize::MAX`); the visible `items` are already the filtered slice.
pub fn letter_grouped_rows<Target>(
    mut items: Vec<(String, MediaListRow<Target>)>,
    total_count: usize,
    letter_filter_active: bool,
) -> Vec<MediaListRow<Target>> {
    items.sort_by_cached_key(|(sort_str, _)| natural_sort_key(sort_str));

    let bucket_total = if letter_filter_active {
        usize::MAX
    } else {
        total_count
    };

    let mut rows: Vec<MediaListRow<Target>> = Vec::with_capacity(items.len() + 8);
    let mut last_bucket: Option<String> = None;
    for (sort_str, row) in items {
        let bucket = letter_bucket_label(&sort_str, bucket_total);
        if last_bucket.as_deref() != Some(bucket.as_str()) {
            if last_bucket.is_some() {
                rows.push(MediaListRow::Spacer);
            }
            rows.push(MediaListRow::Heading {
                text: bucket.clone(),
            });
            last_bucket = Some(bucket);
        }
        rows.push(row);
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::components::media_list::{MediaListRow, MediaSemanticState};

    fn item(target: &str, primary: &str) -> MediaListRow<String> {
        MediaListRow::Item {
            target: target.into(),
            primary: primary.into(),
            trailing: None,
            duration: None,
            semantic_state: MediaSemanticState::Ordinary,
        }
    }

    /// `(sort_str, item)` pair; sort_str stands in for `effective_sort_str`
    /// (article-stripped where the fixture name has one).
    fn pair(sort_str: &str, target: &str) -> (String, MediaListRow<String>) {
        (sort_str.into(), item(target, target))
    }

    fn shape(rows: &[MediaListRow<String>]) -> Vec<String> {
        rows.iter()
            .map(|r| match r {
                MediaListRow::Heading { text } => format!("H:{text}"),
                MediaListRow::Spacer => "S".to_string(),
                MediaListRow::Item { target, .. } => format!("I:{target}"),
            })
            .collect()
    }

    fn headings(rows: &[MediaListRow<String>]) -> Vec<String> {
        rows.iter()
            .filter_map(|r| match r {
                MediaListRow::Heading { text } => Some(text.clone()),
                _ => None,
            })
            .collect()
    }

    // Generic-Emby/Movies-style fixture: leading articles, a digit title, an
    // accented title, and a symbol title.
    fn movies_fixture() -> Vec<(String, MediaListRow<String>)> {
        vec![
            pair("Apollo 13", "apollo13"),
            pair("Amelie", "amelie"), // "Amélie" article-free
            pair("Batman", "batman"), // "The Batman" -> article stripped
            pair("Zodiac", "zodiac"), // "The Zodiac"
            pair("300", "n300"),      // digit -> "#"
            pair("Élan", "elan"),     // accented first char -> "#"
            pair("Matrix", "matrix"), // "The Matrix"
        ]
    }

    #[test]
    fn ranges_below_250_and_natural_sort_and_hash_bucket() {
        // Digit titles sort before "A" and accented titles after "Z", so the
        // "#" bucket appears at both ends -- the reference only compares each
        // item's bucket against the previous one, so a non-contiguous bucket
        // emits a fresh heading. Faithful to `render_letter_grouped_rows`.
        let rows = letter_grouped_rows(movies_fixture(), 7, false);
        assert_eq!(
            shape(&rows),
            vec![
                "H:#",
                "I:n300",
                "S",
                "H:A\u{2013}C",
                "I:amelie",
                "I:apollo13",
                "I:batman",
                "S",
                "H:M\u{2013}O",
                "I:matrix",
                "S",
                "H:V\u{2013}Z",
                "I:zodiac",
                "S",
                "H:#",
                "I:elan",
            ]
        );
    }

    #[test]
    fn per_letter_buckets_at_250_boundary() {
        // 249 -> ranges, 250 -> per-letter.
        let fixture = || vec![pair("Alpha", "a"), pair("Bravo", "b")];
        assert_eq!(
            headings(&letter_grouped_rows(fixture(), 249, false)),
            vec!["A\u{2013}C"]
        );
        assert_eq!(
            headings(&letter_grouped_rows(fixture(), 250, false)),
            vec!["A", "B"]
        );
    }

    #[test]
    fn letter_filter_forces_per_letter_mode() {
        // Small filtered slice but filter active -> per-letter, not a range.
        let fixture = vec![pair("Alpha", "a"), pair("Anchor", "a2"), pair("Comet", "c")];
        assert_eq!(
            headings(&letter_grouped_rows(fixture, 3, true)),
            vec!["A", "C"]
        );
    }

    #[test]
    fn spacer_precedes_every_heading_except_the_first() {
        let rows = letter_grouped_rows(movies_fixture(), 7, false);
        for window in rows.windows(2) {
            if matches!(window[1], MediaListRow::Heading { .. }) {
                assert!(
                    matches!(window[0], MediaListRow::Spacer),
                    "non-first heading must be preceded by a spacer"
                );
            }
        }
        // First row is a heading with no preceding spacer.
        assert!(matches!(rows[0], MediaListRow::Heading { .. }));
        assert!(headings(&rows).len() >= 2);
        // No trailing spacer.
        assert!(!matches!(rows.last(), Some(MediaListRow::Spacer)));
    }
}
