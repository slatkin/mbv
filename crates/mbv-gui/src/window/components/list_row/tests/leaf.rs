//! Painted-structure tests of the leaf row part (`gui-tree-row-roles` design
//! D10): the well segment names the well edge the row sits on, and the 1 px
//! separator paints only on a row that rests on the row above.

use crate::window::components::list_row::{LeafRow, WellEdge};

use super::*;

/// The well segment a leaf row paints, by its edge hook.
const WELL: &str = "[data-tree-well]";

/// One leaf row the fixture paints.
#[derive(Clone, Copy, PartialEq)]
struct LeafSpec {
    well: WellEdge,
}

/// The root: paints one leaf row.
fn leaf_app(spec: LeafSpec) -> Element {
    rsx! {
        LeafRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            title: "Sabrosito".to_owned(),
            well: spec.well,
            selected: false,
            on_press: Callback::new(|()| {}),
            on_play: Callback::new(|()| {}),
        }
    }
}

/// Paint one leaf row and let its first render land.
fn leaf_built(spec: LeafSpec) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(|spec: LeafSpec| leaf_app(spec), spec),
        config(),
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`gui-tree-row-roles` design D10, D3): a leaf row's well segment
/// carries its edge in `data-tree-well`, and the 1 px separator paints only
/// on a row that rests on the row above — the middle and last rows of a well.
#[rstest]
#[case(WellEdge::Only, "only", false)]
#[case(WellEdge::First, "first", false)]
#[case(WellEdge::Middle, "middle", true)]
#[case(WellEdge::Last, "last", true)]
fn a_leaf_names_its_well_edge_and_paints_the_separator_only_off_the_first_row(
    #[case] well: WellEdge,
    #[case] mark: &str,
    #[case] separator: bool,
) {
    let fixture = leaf_built(LeafSpec { well });

    assert_eq!(
        fixture.attr_of(WELL, "data-tree-well").as_deref(),
        Some(mark),
        "the segment carries its edge"
    );
    let button_style = fixture.style_of(ROW);
    assert_eq!(
        button_style.contains("border-top: 1px solid var(--canvas-tree-well-rule)"),
        separator,
        "the separator paints only off the first row: {button_style}"
    );
}
