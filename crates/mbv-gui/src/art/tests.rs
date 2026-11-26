//! The `mbv-art` path grammar's contract (design D7 of
//! `gui-change-driven-sections`, extended by D4 of `gui-hero-design-polish`):
//! the three accepted forms parse to their keys, and a shape outside them —
//! another prefix, a missing segment, or an undecodable escape — is
//! malformed.

use rstest::rstest;

use super::backdrop::{BackdropRequest, BackdropStyle};
use super::{ArtPath, parse_path};

/// The one case table: each row is a distinct outcome of the grammar.
#[rstest]
#[case::card_decodes_its_key("card/a%3Ab", Some(ArtPath::Card("a:b".to_owned())))]
#[case::backdrop_decodes_its_key("backdrop/v/1200/a%3Ab", Some(ArtPath::Backdrop(BackdropRequest { key: "a:b".to_owned(), width: 1200, style: BackdropStyle::Video })))]
#[case::placeholder_is_its_own_path("placeholder", Some(ArtPath::Placeholder))]
#[case::another_prefix_is_malformed("bogus/answer", None)]
#[case::a_missing_segment_is_malformed("card", None)]
#[case::an_undecodable_escape_is_malformed("card/%zz", None)]
fn a_request_path_parses_to_one_of_the_three_forms(
    #[case] path: &str,
    #[case] expected: Option<ArtPath>,
) {
    assert_eq!(parse_path(path), expected);
}
