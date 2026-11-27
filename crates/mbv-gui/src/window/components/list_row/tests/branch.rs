//! Painted-structure tests of the branch row part (`gui-tree-row-roles`
//! design D10): the Branch swatch follows the row's sibling, a branch that
//! opens paints `aria-expanded` and the chevron of its open state, and a
//! branch that opens nothing paints no mark and no `aria-expanded`.

use crate::window::components::list_row::BranchRow;

use super::*;

/// The Branch swatch span, by its swatch hook.
const SWATCH: &str = "[data-tree-swatch]";

/// One branch row the fixture paints.
#[derive(Clone, Copy, PartialEq)]
struct BranchSpec {
    sibling: usize,
    open: Option<bool>,
}

/// The root: paints one branch row.
fn branch_app(spec: BranchSpec) -> Element {
    let BranchSpec { sibling, open } = spec;
    rsx! {
        BranchRow {
            attr: "test",
            kind: "item".to_owned(),
            id: "row-1".to_owned(),
            title: "Sabrosito".to_owned(),
            sibling,
            facts: String::new(),
            open,
            selected: false,
            on_press: Callback::new(|()| {}),
            on_play: Callback::new(|()| {}),
        }
    }
}

/// Paint one branch row and let its first render land.
fn branch_built(spec: BranchSpec) -> Fixture {
    let mut doc = DioxusDocument::new(
        VirtualDom::new_with_props(|spec: BranchSpec| branch_app(spec), spec),
        config(),
    );
    doc.initial_build();
    let mut fixture = Fixture { doc };
    fixture.poll_until_idle();
    fixture
}

/// Contract (`gui-tree-row-roles` design D10, D4): a branch row paints the
/// Branch swatch its sibling picks — the hook carries the swatch number and
/// the square loads the sibling's swatch token.
#[rstest]
#[case(0, "1", "var(--canvas-tree-swatch-1)")]
#[case(4, "5", "var(--canvas-tree-swatch-5)")]
#[case(5, "2", "var(--canvas-tree-swatch-2)")]
fn a_branch_paints_the_swatch_its_sibling_picks(
    #[case] sibling: usize,
    #[case] mark: &str,
    #[case] token: &str,
) {
    let fixture = branch_built(BranchSpec {
        sibling,
        open: Some(false),
    });

    assert_eq!(
        fixture.attr_of(SWATCH, "data-tree-swatch").as_deref(),
        Some(mark),
        "the hook carries the swatch number"
    );
    assert!(
        fixture
            .style_of(SWATCH)
            .contains(&format!("background: {token}")),
        "the swatch loads the sibling's token: {}",
        fixture.style_of(SWATCH)
    );
}

/// Contract (`gui-tree-row-roles` design D10, D5): a branch that opens paints
/// its `aria-expanded` and the chevron mark of its open state — down when
/// open, right when closed.
#[rstest]
#[case(Some(true), "true", "svg.lucide-chevron-down")]
#[case(Some(false), "false", "svg.lucide-chevron-right")]
fn a_branch_that_opens_paints_aria_expanded_and_its_mark(
    #[case] open: Option<bool>,
    #[case] expanded: &str,
    #[case] glyph: &str,
) {
    let fixture = branch_built(BranchSpec { sibling: 0, open });

    assert_eq!(
        fixture.attr_of(ROW, "aria-expanded").as_deref(),
        Some(expanded),
        "the row carries its open state"
    );
    assert!(fixture.presents(glyph), "the state mark paints {glyph}");
}

/// Contract (`gui-tree-row-roles` design D10, D5): a branch that opens
/// nothing paints no chevron and no `aria-expanded`.
#[test]
fn a_branch_that_opens_nothing_paints_no_mark() {
    let fixture = branch_built(BranchSpec {
        sibling: 0,
        open: None,
    });

    assert!(!fixture.presents("svg.lucide-chevron-down"));
    assert!(!fixture.presents("svg.lucide-chevron-right"));
    assert_eq!(fixture.attr_of(ROW, "aria-expanded"), None);
}
