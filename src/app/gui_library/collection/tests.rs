//! Tests of the collection-selection core (gui-collection-dropdown design
//! D1, D4): the spec's "The selection is All or checked collections"
//! transitions as one table, and the date comparison's undated-last rule in
//! both orders.

use std::cmp::Ordering;

use rstest::rstest;

use super::{GuiCollectionSet, GuiDateOrder, compare_gui_date_order};

/// One selection-table operation (design D1): a press on a collection, a
/// press on All, or the catalog's retention over the present keys.
enum Op {
    Toggle(&'static str),
    PressAll,
    RetainPresent(&'static [&'static str]),
}

/// Apply `op` to `set`, as the store and the dropdown command do.
fn apply(set: GuiCollectionSet, op: &Op) -> GuiCollectionSet {
    match op {
        Op::Toggle(key) => set.toggle(key),
        Op::PressAll => GuiCollectionSet::All,
        Op::RetainPresent(present) => {
            let present: Vec<String> = present.iter().map(|key| (*key).to_string()).collect();
            set.retain_present(&present)
        }
    }
}

/// A checked set over `keys`, as the selection holds them.
fn checked(keys: &[&str]) -> GuiCollectionSet {
    GuiCollectionSet::Checked(keys.iter().map(|key| (*key).to_string()).collect())
}

/// The spec's selection transitions (gui-collection-dropdown, "The selection
/// is All or checked collections"): check first, check second, uncheck the
/// last, press All, retain past a vanished collection, and retain to empty.
#[rstest]
#[case::check_first_clears_all(
    GuiCollectionSet::All,
    Op::Toggle("tifo"),
    checked(&["tifo"]),
)]
#[case::check_second(
    checked(&["tifo"]),
    Op::Toggle("offmenu"),
    checked(&["tifo", "offmenu"]),
)]
#[case::unchecking_the_last_collection_returns_to_all(
    checked(&["tifo"]),
    Op::Toggle("tifo"),
    GuiCollectionSet::All,
)]
#[case::pressing_all_clears_every_check(
    checked(&["tifo", "offmenu"]),
    Op::PressAll,
    GuiCollectionSet::All,
)]
#[case::a_vanished_collection_drops(
    checked(&["soul", "jazz"]),
    Op::RetainPresent(&["jazz"]),
    checked(&["jazz"]),
)]
#[case::no_surviving_collection_returns_to_all(
    checked(&["soul", "jazz"]),
    Op::RetainPresent(&["blues"]),
    GuiCollectionSet::All,
)]
fn the_selection_transitions_follow_the_spec(
    #[case] start: GuiCollectionSet,
    #[case] op: Op,
    #[case] expected: GuiCollectionSet,
) {
    assert_eq!(apply(start, &op), expected);
}

/// The undated row is last in Newest order: the later date sorts first and
/// the undated row sorts after every dated one (design D4).
#[test]
fn the_undated_row_is_last_in_newest_order() {
    assert_eq!(
        compare_gui_date_order(Some(200), Some(100), GuiDateOrder::Newest),
        Ordering::Less,
        "the later date sorts first"
    );
    assert_eq!(
        compare_gui_date_order(Some(100), None, GuiDateOrder::Newest),
        Ordering::Less,
        "a dated row sorts before an undated one"
    );
    assert_eq!(
        compare_gui_date_order(None, None, GuiDateOrder::Newest),
        Ordering::Equal
    );
}

/// The undated row is last in Oldest order too: the earlier date sorts first,
/// and the undated row still sorts after every dated one (design D4).
#[test]
fn the_undated_row_is_last_in_oldest_order() {
    assert_eq!(
        compare_gui_date_order(Some(100), Some(200), GuiDateOrder::Oldest),
        Ordering::Less,
        "the earlier date sorts first"
    );
    assert_eq!(
        compare_gui_date_order(Some(100), None, GuiDateOrder::Oldest),
        Ordering::Less,
        "a dated row still sorts before an undated one"
    );
    assert_eq!(
        compare_gui_date_order(None, None, GuiDateOrder::Oldest),
        Ordering::Equal
    );
}
