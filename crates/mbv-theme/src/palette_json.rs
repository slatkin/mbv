//! Keeps `docs/palette.json` a live reference (design D6). Every test run
//! regenerates its mechanical fields from the theme's enums — the slots, each
//! role's slot and value, the role-set members, and each surface's resting and
//! focused slot and fill — and rewrites the file when they changed. The
//! hand-written `uses` prose and the `specials` section are carried over.
//! This is a generator, not a guard: it never fails because the file was
//! stale. It walks `Slot::ALL`, `Role::ALL`, `HERO_META_ROLES`, `HINT_CHIPS`
//! and `Surface::ALL`; there is no source-text parsing.

use std::collections::BTreeMap;

use ratatui::style::Color;
use serde_json::{Value, json};

use super::slot::{Slot, active};
use super::surface::Surface;
use super::surface_resolve::surface_colors;
use super::surface_table::slots;
use super::{HERO_META_ROLES, HINT_CHIPS, Role};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/palette.json");

/// The files that hold the theme's tables, recorded for the reader.
const SOURCES: [&str; 5] = [
    "crates/mbv-theme/src/slot.rs",
    "crates/mbv-theme/src/role.rs",
    "crates/mbv-theme/src/surface.rs",
    "crates/mbv-theme/src/surface_table.rs",
    "crates/mbv-theme/src/surface_resolve.rs",
];

/// The `#rrggbb` spelling of a theme colour.
fn hex(color: Color) -> String {
    let [r, g, b] = rgb(color);
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// The `[r, g, b]` spelling of a theme colour.
fn rgb(color: Color) -> [u8; 3] {
    match color {
        Color::Rgb(r, g, b) => [r, g, b],
        other => panic!("theme colour is not an Rgb colour: {other:?}"),
    }
}

/// The existing `uses` prose of one array section, keyed by name.
fn prose(section: &Value) -> BTreeMap<String, String> {
    section
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            Some((
                entry["name"].as_str()?.to_string(),
                entry["uses"].as_str()?.to_string(),
            ))
        })
        .collect()
}

/// A carried-over `uses`, or an empty one (named on stderr) for a new entry.
fn uses(prose: &BTreeMap<String, String>, kind: &str, name: &str) -> String {
    prose.get(name).cloned().unwrap_or_else(|| {
        eprintln!("docs/palette.json: new {kind} {name} — add its `uses` prose");
        String::new()
    })
}

/// The whole document rebuilt from the theme, keeping `old`'s prose.
fn regenerate(old: &Value) -> Value {
    let role_prose = prose(&old["roles"]);
    let surface_prose = prose(&old["surfaces"]);
    let role_set_uses = |name: &str| match old["role_sets"][name]["uses"].as_str() {
        Some(text) => text.to_string(),
        None => uses(&BTreeMap::new(), "role set", name),
    };

    let slot_entries: Vec<Value> = Slot::ALL
        .iter()
        .map(|&slot| {
            let color = active().get(slot);
            json!({
                "hex": hex(color),
                "name": format!("{slot:?}"),
                "rgb": rgb(color),
            })
        })
        .collect();

    let roles: Vec<Value> = Role::ALL
        .iter()
        .map(|&role| {
            let color = role.color();
            json!({
                "hex": hex(color),
                "name": format!("{role:?}"),
                "rgb": rgb(color),
                "slot": format!("{:?}", role.slot()),
                "uses": uses(&role_prose, "role", &format!("{role:?}")),
            })
        })
        .collect();

    let role_sets = json!({
        "HERO_META_ROLES": {
            "members": HERO_META_ROLES
                .iter()
                .map(|&role| format!("{role:?}"))
                .collect::<Vec<_>>(),
            "uses": role_set_uses("HERO_META_ROLES"),
        },
        "HINT_CHIPS": {
            "members": HINT_CHIPS
                .iter()
                .map(|&surface| hex(surface_colors(surface, false).fill))
                .collect::<Vec<_>>(),
            "uses": role_set_uses("HINT_CHIPS"),
        },
    });

    let mut surface_entries: Vec<Value> = Surface::ALL
        .iter()
        .map(|&surface| {
            let (resting, focused) = slots(surface);
            json!({
                "focused": hex(surface_colors(surface, true).fill),
                "focusedSlot": format!("{focused:?}"),
                "name": format!("{surface:?}"),
                "resting": hex(surface_colors(surface, false).fill),
                "restingSlot": format!("{resting:?}"),
                "uses": uses(&surface_prose, "surface", &format!("{surface:?}")),
            })
        })
        .collect();
    surface_entries.sort_by_key(|entry| entry["name"].as_str().unwrap_or_default().to_string());

    json!({
        "role_sets": role_sets,
        "roles": roles,
        "slots": slot_entries,
        "source": SOURCES,
        "specials": old["specials"],
        "surfaces": surface_entries,
    })
}

fn write_if_changed(old_text: &str, new_text: &str) {
    if old_text != new_text {
        std::fs::write(PATH, new_text).expect("write docs/palette.json");
    }
}

/// Contract: `docs/palette.json`'s mechanical fields match this crate after
/// any test run (the file is the maintainer's live colour reference).
#[test]
fn docs_palette_json_is_regenerated_from_the_theme() {
    let old_text = std::fs::read_to_string(PATH).expect("read docs/palette.json");
    let old: Value = serde_json::from_str(&old_text).expect("docs/palette.json is valid JSON");
    let new_text = serde_json::to_string(&regenerate(&old)).expect("serialize palette.json");
    write_if_changed(&old_text, &new_text);
}
