//! Tests of the collection-selection core (gui-collection-dropdown design
//! D1, D4): the spec's "The selection is All or checked collections"
//! transitions as one table, and the date comparison's undated-last rule in
//! both orders.

use std::cmp::Ordering;

use rstest::rstest;

use super::{
    GuiCollectionNoun, GuiCollectionSet, GuiCollectionTarget, GuiCollections, GuiDateOrder,
    compare_gui_date_order,
};

/// One selection-table operation (design D1): a press on a collection, a
/// press on All, or the catalog's retention over the present keys.
enum Op {
    Toggle(&'static str),
    PressAll,
    RetainPresent(&'static [&'static str]),
    /// The retention over a catalog that may not have landed yet.
    Effective(Option<&'static [&'static str]>),
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
        Op::Effective(present) => {
            let present = present.map(|present| {
                present
                    .iter()
                    .map(|key| (*key).to_string())
                    .collect::<Vec<_>>()
            });
            set.effective(present.as_deref())
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
#[case::a_catalog_that_has_not_landed_keeps_the_set(
    checked(&["soul", "jazz"]),
    Op::Effective(None),
    checked(&["soul", "jazz"]),
)]
#[case::a_landed_catalog_retains(
    checked(&["soul", "jazz"]),
    Op::Effective(Some(&["soul"])),
    checked(&["soul"]),
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

/// All names itself (design D6): the location and the Music Hero header read
/// `All` whatever the catalog holds.
#[test]
fn selected_names_names_all() {
    let collections = GuiCollections::new(
        GuiCollectionTarget::Feeds,
        &GuiCollectionSet::All,
        GuiCollectionNoun::Subscriptions,
        [("feed-a".to_owned(), "Feed A".to_owned())],
        None,
    );
    assert_eq!(collections.selected_names(), ["All"]);
}

/// A checked set names its checked collections' display names in catalog
/// order (design D6): the one fact the Status bar's middle segment and the
/// Music Hero header read, whatever press order the set keeps.
#[test]
fn selected_names_names_the_checked_collections_in_catalog_order() {
    let collections = GuiCollections::new(
        GuiCollectionTarget::Feeds,
        &checked(&["feed-b", "feed-a"]),
        GuiCollectionNoun::Subscriptions,
        [
            ("feed-a".to_owned(), "Feed A".to_owned()),
            ("feed-b".to_owned(), "Feed B".to_owned()),
            ("feed-c".to_owned(), "Feed C".to_owned()),
        ],
        None,
    );
    assert_eq!(collections.selected_names(), ["Feed A", "Feed B"]);
}
