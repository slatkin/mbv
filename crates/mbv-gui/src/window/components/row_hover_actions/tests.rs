//! Headless tests of the rows' hover controls (gui-row-hover-actions design
//! D1, D2): the part paints exactly the controls its row offers, in the fixed
//! order Play, Add to queue, Shuffle, Expand/collapse. The reveal and the
//! press behaviour are the stylesheet's and the browser's, so no test checks
//! them here. No test opens a window, sleeps, or reaches the core.

use blitz_dom::Document as _;
use blitz_dom::LocalName;
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::RowHoverActions;

/// The controls a row offers, as `(play, queue, shuffle, toggle)`: one flag
/// per callback, so a case names the controls its row has rather than a prop
/// list per case.
type Offer = (bool, bool, bool, bool);

/// The callback an offered control gets, `None` for a control the row does
/// not offer.
fn callback(offered: bool) -> Option<Callback<()>> {
    offered.then(|| Callback::new(|()| {}))
}

/// The part over one row's offer, mounted once per case, with the row's own
/// name when the case carries one.
fn painted(offer: Offer) -> DioxusDocument {
    named(offer, None)
}

/// The part over one row's offer and that row's name, mounted once per case.
fn named(offer: Offer, row_name: Option<&str>) -> DioxusDocument {
    let row_name = row_name.map(str::to_owned);
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(
            move |(offer, row_name): (Offer, Option<String>)| {
                let (play, queue, shuffle, toggle) = offer;
                rsx! {
                    RowHoverActions {
                        on_play: callback(play),
                        on_queue: callback(queue),
                        on_shuffle: callback(shuffle),
                        on_toggle: callback(toggle),
                        open: false,
                        row_name: row_name,
                    }
                }
            },
            (offer, row_name),
        ),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// The `data-row-action` value of every painted control, in paint order.
fn actions(doc: &DioxusDocument) -> Vec<String> {
    let dom = doc.inner();
    dom.query_selector_all("[data-row-action]")
        .expect("the selector is valid")
        .into_iter()
        .filter_map(|id| {
            dom.get_node(id)
                .expect("the node is in the document")
                .attr(LocalName::from("data-row-action"))
                .map(str::to_owned)
        })
        .collect()
}

/// The accessible name of the control the `data-row-action` value names.
fn label_of(doc: &DioxusDocument, action: &str) -> String {
    let dom = doc.inner();
    let selector = format!("[data-row-action=\"{action}\"]");
    let node = dom
        .query_selector(&selector)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{action} paints"));
    dom.get_node(node)
        .expect("the node is in the document")
        .attr(LocalName::from("aria-label"))
        .unwrap_or_default()
        .to_owned()
}

/// Contract (design D1, D2): only the controls the row offers paint, and they
/// paint in the fixed order Play, Add to queue, Shuffle, Expand/collapse.
#[rstest]
#[case((true, true, true, true), &["play", "queue", "shuffle", "toggle"])]
#[case((false, true, false, true), &["queue", "toggle"])]
#[case((false, false, false, false), &[])]
fn only_the_offered_controls_paint_in_the_fixed_order(
    #[case] offer: Offer,
    #[case] expected: &[&str],
) {
    let doc = painted(offer);

    assert_eq!(actions(&doc), expected);
}

/// Contract (gui-row-hover-actions review, unit 4): a list of rows must not
/// expose `N` identical Play buttons, so a row name composes the Play
/// control's accessible name as `Play {row_name}`; a row that passes no name
/// keeps the plain `Play`.
#[rstest]
#[case(Some("Rhymin & Stealin"), "Play Rhymin & Stealin")]
#[case(None, "Play")]
fn the_play_control_names_its_row(#[case] row_name: Option<&str>, #[case] expected: &str) {
    let doc = named((true, false, false, false), row_name);

    assert_eq!(label_of(&doc, "play"), expected);
}
