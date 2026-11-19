//! Tests of the theme layer (D16): the theme rule carries the role colours
//! and chrome fills to the `:root` properties, an absent theme writes
//! nothing, and a transport glyph's colour gates on availability. Pure
//! string builders, so no document harness runs.

use std::collections::BTreeMap;

use rstest::rstest;

use super::{EfToken, ef_var, glyph_color, role_property, theme_style_rule};
use crate::bridge::frame::theme::{GuiChrome, GuiRgb, GuiRole, GuiTheme};

/// The stylesheet the app loads, read so a palette name the Rust side writes
/// must exist there.
const APP_CSS: &str = include_str!("../../../assets/app.css");

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
            queue_panel: GuiRgb {
                r: 96,
                g: 97,
                b: 98,
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
/// their `--mbv-` properties in the `rgb(r, g, b)` form the window write
/// used.
#[test]
fn the_theme_rule_writes_the_role_and_chrome_colours() {
    let rule = theme_style_rule(Some(&fixture_theme()));

    assert!(rule.starts_with(":root"), "{rule}");
    assert!(rule.contains("--mbv-textPrimary: rgb(1, 2, 3)"), "{rule}");
    assert!(rule.contains("--mbv-accent: rgb(4, 5, 6)"), "{rule}");
    assert!(rule.contains("--mbv-background: rgb(10, 11, 12)"), "{rule}");
    assert!(rule.contains("--mbv-border: rgb(70, 71, 72)"), "{rule}");
    assert!(rule.contains("--mbv-input: rgb(80, 81, 82)"), "{rule}");
}

/// The `--mbv-` property names the rule declares, in declaration order.
fn declared_properties(rule: &str) -> Vec<String> {
    rule.split(';')
        .filter_map(|declaration| {
            let declaration = declaration.rsplit('{').next()?;
            let (name, _) = declaration.split_once(':')?;
            let name = name.trim();
            name.starts_with("--mbv-").then(|| name.to_owned())
        })
        .collect()
}

/// The rule's `--mbv-` property set is exactly the one the reference
/// build defined: every role, plus the eight chrome fills whose concept has a
/// one-word name. Its chrome DTO had no `rename_all`, so the six two-word
/// fills reached the webview as `--mbv-queue_column` and the like while every
/// consumer read the camelCase name — those six properties were never
/// defined, and the fills they name stayed transparent (see `write_chrome`).
#[test]
fn the_theme_rule_declares_the_reference_property_set() {
    let rule = theme_style_rule(Some(&GuiTheme::current()));
    let mut expected: Vec<String> = GuiRole::ALL
        .iter()
        .map(|&role| role_property(role))
        .collect();
    expected.extend(
        [
            "background",
            "card",
            "popover",
            "muted",
            "secondary",
            "sidebar",
            "border",
            "input",
        ]
        .map(|name| format!("--mbv-{name}")),
    );
    expected.sort();
    let mut declared = declared_properties(&rule);
    declared.sort();
    assert_eq!(declared, expected, "{rule}");
}

/// An absent theme writes nothing, so `app.css`'s fallbacks stay and the
/// chrome paints before the first theme.
#[test]
fn an_absent_theme_writes_nothing() {
    assert_eq!(theme_style_rule(None), "");
}

/// Contract (design D1 of `gui-queue-panel-redesign`): every `--ef-*` token the
/// typed accessor names is a property `assets/app.css` defines, so a painter
/// cannot name a token that compiles and paints nothing.
#[test]
fn every_ef_token_names_a_stylesheet_property() {
    for token in EfToken::ALL {
        assert!(
            APP_CSS.contains(&format!("--ef-{}:", token.name())),
            "{} is not a property in app.css",
            token.name()
        );
        assert_eq!(ef_var(token), format!("var(--ef-{})", token.name()));
    }
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
