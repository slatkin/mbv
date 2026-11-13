//! Tests of the theme layer (D16): the theme rule carries the role colours
//! and chrome fills to the `:root` properties, an absent theme writes
//! nothing, and a transport glyph's colour gates on availability. Pure
//! string builders, so no document harness runs.

use std::collections::BTreeMap;

use rstest::rstest;

use super::{glyph_color, theme_style_rule};
use crate::bridge::frame::theme::{GuiChrome, GuiRgb, GuiRole, GuiTheme};

/// A theme with two roles and a full chrome fixture.
fn fixture_theme() -> GuiTheme {
    let mut roles = BTreeMap::new();
    roles.insert(GuiRole::TextPrimary, GuiRgb { r: 1, g: 2, b: 3 });
    roles.insert(GuiRole::Accent, GuiRgb { r: 4, g: 5, b: 6 });
    GuiTheme {
        roles,
        chrome: GuiChrome {
            background: GuiRgb {
                r: 10,
                g: 11,
                b: 12,
            },
            card: GuiRgb {
                r: 20,
                g: 21,
                b: 22,
            },
            popover: GuiRgb {
                r: 30,
                g: 31,
                b: 32,
            },
            muted: GuiRgb {
                r: 40,
                g: 41,
                b: 42,
            },
            secondary: GuiRgb {
                r: 50,
                g: 51,
                b: 52,
            },
            sidebar: GuiRgb {
                r: 60,
                g: 61,
                b: 62,
            },
            border: GuiRgb {
                r: 70,
                g: 71,
                b: 72,
            },
            input: GuiRgb {
                r: 80,
                g: 81,
                b: 82,
            },
            queue_column: GuiRgb {
                r: 90,
                g: 91,
                b: 92,
            },
            queue_column_focused: GuiRgb {
                r: 93,
                g: 94,
                b: 95,
            },
            queue_panel: GuiRgb {
                r: 96,
                g: 97,
                b: 98,
            },
            queue_panel_focused: GuiRgb {
                r: 99,
                g: 100,
                b: 101,
            },
            queue_playback_band: GuiRgb {
                r: 102,
                g: 103,
                b: 104,
            },
            queue_footer_band: GuiRgb {
                r: 105,
                g: 106,
                b: 107,
            },
        },
    }
}

/// The theme rule is a `:root` rule, and the roles and chrome fills land on
/// their `--mbv-` properties in the `rgb(r, g, b)` form the webview write
/// used.
#[test]
fn the_theme_rule_writes_the_role_and_chrome_colours() {
    let rule = theme_style_rule(Some(&fixture_theme()));

    assert!(rule.starts_with(":root"), "{rule}");
    assert!(rule.contains("--mbv-textPrimary: rgb(1, 2, 3)"), "{rule}");
    assert!(rule.contains("--mbv-accent: rgb(4, 5, 6)"), "{rule}");
    assert!(rule.contains("--mbv-background: rgb(10, 11, 12)"), "{rule}");
    assert!(rule.contains("--mbv-border: rgb(70, 71, 72)"), "{rule}");
    assert!(
        rule.contains("--mbv-queueColumn: rgb(90, 91, 92)"),
        "{rule}"
    );
}

/// An absent theme writes nothing, so `app.css`'s fallbacks stay and the
/// chrome paints before the first theme.
#[test]
fn an_absent_theme_writes_nothing() {
    assert_eq!(theme_style_rule(None), "");
}

/// A transport glyph paints its role while actionable and the muted role
/// when not.
#[rstest]
#[case::actionable(true, "var(--mbv-statusAvailable)")]
#[case::not_actionable(false, "var(--mbv-textMuted)")]
fn a_transport_glyph_gates_its_colour_on_availability(
    #[case] available: bool,
    #[case] expected: &str,
) {
    assert_eq!(glyph_color(available, GuiRole::StatusAvailable), expected);
}
