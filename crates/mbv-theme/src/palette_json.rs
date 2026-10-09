//! TEMPORARY migration bridge (theme-slot-model group 1; task 1.5 replaces
//! this module).
//!
//! The legacy generator rebuilt `docs/palette.json` by parsing this crate's
//! source text — `palette.rs` and the `Level`/`Row` surface machinery, both of
//! which the slot model deleted. Until task 1.5 rebuilds the generator over
//! `Slot::ALL`/`Role::ALL`/`HINT_CHIPS`/`Surface::ALL` and rewrites the file's
//! keys, this test pins the legacy file's recorded values against the new
//! model without writing anything: the slot, role and surface tables must
//! resolve exactly the colours the legacy palette recorded.

use std::collections::{BTreeMap, BTreeSet};

use ratatui::style::Color;
use serde_json::Value;

use super::slot::{Slot, active};
use super::{HERO_META_ROLES, HINT_CHIPS, Role, Surface, surface_colors};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/palette.json");

/// The fill roles design D5 folds into surfaces. The legacy file records them
/// as roles; the new model does not, so this is the closed set of legacy role
/// names with no `Role` variant.
const FILL_ROLES: [&str; 15] = [
    "HERO_CREDITS_STRIPE",
    "PILL_BG",
    "PILL_ROW_BG",
    "PILL_SELECTED_BG",
    "PLAYLIST_STRIPE_BG",
    "SELECTED_ROW_BG",
    "SESSIONS_STRIPE_BG",
    "SETTINGS_STRIPE_BG",
    "SURFACE_BACKDROP",
    "SURFACE_CHROME",
    "SURFACE_FOCUSED",
    "SURFACE_RESTING",
    "SURFACE_SIDEBAR",
    "WORKSPACE_FOCUSED_FILL",
    "WORKSPACE_FOCUSED_STRIPE",
];

/// The new surfaces whose fills design D5 pins to the legacy identity they
/// replaced: `(surface, resting legacy role, focused legacy role)`.
const REPLACED_BY_ROLES: [(Surface, &str, &str); 7] = [
    (Surface::SelectedRow, "SELECTED_ROW_BG", "SELECTED_ROW_BG"),
    (
        Surface::ListStripe,
        "PLAYLIST_STRIPE_BG",
        "PLAYLIST_STRIPE_BG",
    ),
    (
        Surface::WorkspaceStripe,
        "SURFACE_RESTING",
        "WORKSPACE_FOCUSED_STRIPE",
    ),
    (
        Surface::CreditsStripe,
        "HERO_CREDITS_STRIPE",
        "HERO_CREDITS_STRIPE",
    ),
    (Surface::PopupBorder, "SURFACE_RESTING", "SURFACE_RESTING"),
    (Surface::ModalButton, "SURFACE_CHROME", "SURFACE_CHROME"),
    (
        Surface::TransportRow,
        "SURFACE_BACKDROP",
        "SURFACE_BACKDROP",
    ),
];

/// The `#rrggbb` spelling of a theme colour.
fn hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        other => panic!("theme colour is not an Rgb colour: {other:?}"),
    }
}

/// The legacy `SCREAMING_CASE` constant name of a role's `UpperCamelCase` name.
fn screaming(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 4);
    for (index, ch) in name.char_indices() {
        if index > 0 && ch.is_ascii_uppercase() {
            out.push('_');
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}

/// The legacy file as `(value, document)`.
fn read_legacy() -> Value {
    let text = std::fs::read_to_string(PATH).expect("read docs/palette.json");
    serde_json::from_str(&text).expect("docs/palette.json is valid JSON")
}

/// The legacy `variants` hexes, sorted (slot names differ; values must not).
fn check_slots(doc: &Value) {
    let mut slot_hexes: Vec<String> = Slot::ALL
        .iter()
        .map(|&slot| hex(active().get(slot)))
        .collect();
    slot_hexes.sort();
    let mut variant_hexes: Vec<String> = doc["variants"]
        .as_array()
        .expect("legacy variants array")
        .iter()
        .map(|variant| variant["hex"].as_str().expect("variant hex").to_string())
        .collect();
    variant_hexes.sort();
    assert_eq!(
        slot_hexes, variant_hexes,
        "slot values must match the legacy variant hexes"
    );
}

/// Every surviving role keeps its legacy hex; the legacy roles outside
/// `Role` are exactly design D5's fill roles.
fn check_roles(doc: &Value) {
    let legacy_roles = legacy_role_hexes(doc);
    let mut survivor_names = BTreeSet::new();
    for &role in Role::ALL {
        let name = screaming(&format!("{role:?}"));
        let value = hex(role.color());
        assert_eq!(
            legacy_roles.get(&name).map(String::as_str),
            Some(value.as_str()),
            "role {name} changed value"
        );
        survivor_names.insert(name);
    }
    let uncovered: BTreeSet<String> = legacy_roles
        .keys()
        .filter(|name| !survivor_names.contains(*name))
        .cloned()
        .collect();
    let fills: BTreeSet<String> = FILL_ROLES.iter().map(|name| (*name).to_string()).collect();
    assert_eq!(
        uncovered, fills,
        "legacy roles outside Role must be exactly design D5's fill roles"
    );
}

/// Pre-existing surfaces keep their resting/focused hexes; the new surfaces'
/// fills equal the legacy identities design D5 replaced them with.
fn check_surfaces(doc: &Value) {
    let legacy_surfaces: BTreeMap<String, (String, String)> = doc["surfaces"]
        .as_array()
        .expect("legacy surfaces array")
        .iter()
        .map(|surface| {
            let name = surface["name"].as_str().expect("surface name").to_string();
            let resting = surface["resting"]
                .as_str()
                .expect("surface resting")
                .to_string();
            let focused = surface["focused"]
                .as_str()
                .expect("surface focused")
                .to_string();
            (name, (resting, focused))
        })
        .collect();
    let legacy_roles = legacy_role_hexes(doc);
    for &surface in Surface::ALL {
        let name = format!("{surface:?}");
        if let Some((resting, focused)) = legacy_surfaces.get(&name) {
            assert_eq!(
                hex(surface_colors(surface, false).fill),
                *resting,
                "{name} resting changed"
            );
            assert_eq!(
                hex(surface_colors(surface, true).fill),
                *focused,
                "{name} focused changed"
            );
        }
    }
    for (surface, resting_from, focused_from) in REPLACED_BY_ROLES {
        let name = format!("{surface:?}");
        assert_eq!(
            hex(surface_colors(surface, false).fill),
            legacy_roles[resting_from],
            "{name} resting changed"
        );
        assert_eq!(
            hex(surface_colors(surface, true).fill),
            legacy_roles[focused_from],
            "{name} focused changed"
        );
    }
}

/// The legacy role hexes by constant name.
fn legacy_role_hexes(doc: &Value) -> BTreeMap<String, String> {
    doc["roles"]
        .as_array()
        .expect("legacy roles array")
        .iter()
        .map(|role| {
            (
                role["name"].as_str().expect("role name").to_string(),
                role["hex"].as_str().expect("role hex").to_string(),
            )
        })
        .collect()
}

/// The role sets keep their members: `HERO_META_ROLES` by role name,
/// `HINT_CHIPS` by resolved hex against the legacy `HINT_PILL_FILLS`.
fn check_role_sets(doc: &Value) {
    let meta: Vec<String> = HERO_META_ROLES
        .iter()
        .map(|role| screaming(&format!("{role:?}")))
        .collect();
    let legacy_meta: Vec<String> = doc["role_sets"]["HERO_META_ROLES"]["members"]
        .as_array()
        .expect("legacy HERO_META_ROLES members")
        .iter()
        .map(|member| member.as_str().expect("member name").to_string())
        .collect();
    assert_eq!(meta, legacy_meta, "HERO_META_ROLES members changed");

    let chip_hexes: Vec<String> = HINT_CHIPS
        .iter()
        .map(|&surface| hex(surface_colors(surface, false).fill))
        .collect();
    let legacy_chips: Vec<String> = doc["role_sets"]["HINT_PILL_FILLS"]["members"]
        .as_array()
        .expect("legacy HINT_PILL_FILLS members")
        .iter()
        .map(|member| member.as_str().expect("member hex").to_string())
        .collect();
    assert_eq!(chip_hexes, legacy_chips, "HINT_CHIPS fills changed");
}

/// Contract: during the slot migration the new model resolves every colour
/// the legacy `docs/palette.json` recorded to the same hex. Temporary —
/// task 1.5 replaces this module with the enum-driven regenerator.
#[test]
fn legacy_palette_json_values_survive_the_slot_migration() {
    let doc = read_legacy();
    check_slots(&doc);
    check_roles(&doc);
    check_surfaces(&doc);
    check_role_sets(&doc);
}
