//! Tests of the rail and swatch parts (`gui-tree-row-roles` design D10): the
//! swatch wrap rule and the stylesheet contract for every token the module
//! names. Pure string builders, so no document harness runs.

use rstest::rstest;

use super::{RAIL_GROUP_TOKEN, RAIL_OPEN_TOKEN, SWATCH_TOKENS, swatch_var};

/// The stylesheet the app loads, read so a canvas name the Rust side writes
/// must exist there.
const APP_CSS: &str = include_str!("../../../../assets/app.css");

/// Contract (design D4): a branch row's swatch follows the painted siblings
/// 1 to 5, then the cycle restarts at swatch 2, so orange is never followed
/// by red.
#[rstest]
#[case::first(0, "var(--canvas-tree-swatch-1)")]
#[case::fifth(4, "var(--canvas-tree-swatch-5)")]
#[case::restarts_at_blue(5, "var(--canvas-tree-swatch-2)")]
#[case::wraps_to_blue_again(9, "var(--canvas-tree-swatch-2)")]
fn a_sibling_picks_its_swatch_by_the_wrap_rule(#[case] sibling: usize, #[case] expected: &str) {
    assert_eq!(swatch_var(sibling), expected);
}

/// Contract (design D10, the pattern of
/// `every_ef_token_names_a_stylesheet_property`): every canvas token this
/// module names is a property `assets/app.css` defines, so a rail or swatch
/// cannot name a token that compiles and paints nothing.
#[test]
fn every_tree_rail_token_names_a_stylesheet_property() {
    for name in [RAIL_OPEN_TOKEN, RAIL_GROUP_TOKEN]
        .into_iter()
        .chain(SWATCH_TOKENS)
    {
        assert!(
            APP_CSS.contains(&format!("--canvas-{name}:")),
            "--canvas-{name} is not a property in app.css"
        );
    }
}
