//! The shared whole-rows capacity rule, the port of the former report's
//! `wholeRows` arithmetic.

/// Contract: the whole 30 px rows a list
/// box fits — whole rows only, a partial row counts none.
#[test]
fn the_capacity_counts_whole_30px_rows_only() {
    assert_eq!(super::super::whole_rows(60.0), 2);
    assert_eq!(super::super::whole_rows(59.0), 1);
    assert_eq!(super::super::whole_rows(0.0), 0);
}
