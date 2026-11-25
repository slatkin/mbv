//! Headless tests of the shared play look (gui-played-row-marks design D1,
//! D5): the derivation a Library row's and a queue row's carried state map
//! to, and the mark's painted structure — the `data-play-mark` value, the
//! role, and the accessible label. No test opens a window, sleeps, or
//! reaches the core.

use blitz_dom::{Document as _, LocalName};
use dioxus_native::prelude::*;
use dioxus_native::{DioxusDocument, DocumentConfig};
use rstest::rstest;

use super::{PlayMark, RowPlay};
use crate::bridge::frame::library::GuiHomeRowStateFrame;
use crate::bridge::frame::queue::GuiSemanticState;

/// The mark slot the document paints.
const SLOT: &str = "[data-play-mark]";

/// The root: paints one play mark.
fn mark_app(play: Option<RowPlay>) -> Element {
    rsx! {
        PlayMark { play }
    }
}

/// A document painting one play mark over `play`.
fn built(play: Option<RowPlay>) -> DioxusDocument {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(mark_app, play),
        DocumentConfig::default(),
    );
    doc.initial_build();
    doc
}

/// The attribute of the slot the document paints.
fn slot_attr(doc: &DioxusDocument, attribute: &str) -> Option<String> {
    let dom = doc.inner();
    let id = dom
        .query_selector(SLOT)
        .expect("the selector is valid")
        .unwrap_or_else(|| panic!("{SLOT} paints"));
    dom.get_node(id)
        .expect("the node is in the document")
        .attr(LocalName::from(attribute))
        .map(str::to_owned)
}

/// Contract (design D1): a Library row's carried state derives to `Unplayed`
/// or `Played`, with a resume state unplayed.
#[rstest]
#[case(GuiHomeRowStateFrame::Ordinary, RowPlay::Unplayed)]
#[case(GuiHomeRowStateFrame::Played, RowPlay::Played)]
#[case(GuiHomeRowStateFrame::Active { progress: Some(40) }, RowPlay::Unplayed)]
fn a_rows_state_derives_its_play(#[case] state: GuiHomeRowStateFrame, #[case] play: RowPlay) {
    assert_eq!(RowPlay::of_row(&state), play);
}

/// Contract (design D1): a queue row's carried state derives the same way,
/// except the now-playing row, which carries no mark.
#[rstest]
#[case(GuiSemanticState::Ordinary, Some(RowPlay::Unplayed))]
#[case(GuiSemanticState::Played, Some(RowPlay::Played))]
#[case(GuiSemanticState::Active { progress: Some(40) }, Some(RowPlay::Unplayed))]
#[case(GuiSemanticState::NowPlaying { tracks_progress: true }, None)]
fn a_queue_rows_state_derives_its_play(
    #[case] state: GuiSemanticState,
    #[case] play: Option<RowPlay>,
) {
    assert_eq!(RowPlay::of_queue(&state), play);
}

/// Contract (design D1, `gui-client` spec "Rows mark their played state"):
/// the slot names the state in `data-play-mark` and carries the role and the
/// accessible label while the mark is meaningful — and neither on the empty
/// slot.
#[rstest]
#[case(Some(RowPlay::Unplayed), "unplayed", Some("img"), Some("Unplayed"))]
#[case(Some(RowPlay::Played), "played", Some("img"), Some("Played"))]
#[case(None, "none", None, None)]
fn the_mark_names_its_state(
    #[case] play: Option<RowPlay>,
    #[case] value: &str,
    #[case] role: Option<&str>,
    #[case] label: Option<&str>,
) {
    let doc = built(play);

    assert_eq!(slot_attr(&doc, "data-play-mark").as_deref(), Some(value));
    assert_eq!(slot_attr(&doc, "role").as_deref(), role);
    assert_eq!(slot_attr(&doc, "aria-label").as_deref(), label);
}
