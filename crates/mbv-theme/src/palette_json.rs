//! Keeps `docs/palette.json` a live reference. Every test run regenerates its
//! mechanical fields from this crate — palette variants, each role's variant
//! and value, role-set members, and each surface's level and fills — and
//! rewrites the file when they changed. The hand-written `uses` prose and the
//! `specials` section are carried over. This is a generator, not a guard: it
//! never fails because the file was stale.

use std::collections::BTreeMap;

use ratatui::style::Color;
use serde_json::{json, Value};

use crate::{surface_colors, surface_table, Surface};

const PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/palette.json");

const SOURCES: [&str; 5] = [
    "crates/mbv-theme/src/palette.rs",
    "crates/mbv-theme/src/lib.rs",
    "crates/mbv-theme/src/surface.rs",
    "crates/mbv-theme/src/surface_table.rs",
    "crates/mbv-theme/src/surface_resolve.rs",
];

/// Every `Palette::X => Color::Rgb(..)` arm in `palette.rs`, in declaration
/// order. Read from the source so no second variant list exists to forget.
fn variants() -> Vec<(String, [u8; 3])> {
    include_str!("palette.rs")
        .lines()
        .filter_map(|line| {
            let (name, rgb) = line
                .trim()
                .strip_prefix("Palette::")?
                .split_once(" => Color::Rgb(")?;
            let bytes: Vec<u8> = rgb
                .trim_end_matches("),")
                .split(", ")
                .map(|b| u8::from_str_radix(b.trim_start_matches("0x"), 16).ok())
                .collect::<Option<_>>()?;
            Some((name.to_string(), bytes.try_into().ok()?))
        })
        .collect()
}

/// Every `pub const NAME: Color = Palette::X.color();` role in `lib.rs`, as
/// `(name, variant)` in declaration order.
fn roles() -> Vec<(String, String)> {
    include_str!("lib.rs")
        .lines()
        .filter_map(|line| {
            let (name, rest) = line
                .strip_prefix("pub const ")?
                .split_once(": Color = Palette::")?;
            Some((name.to_string(), rest.split('.').next()?.to_string()))
        })
        .collect()
}

/// Every `pub const NAME: [Color; N] = [..];` role set in `lib.rs`. A member
/// naming a role stays a role name; a `Palette::X.color()` member becomes
/// its hex.
fn role_sets(hexes: &BTreeMap<String, String>) -> Vec<(String, Vec<String>)> {
    let src = include_str!("lib.rs");
    src.match_indices("pub const ")
        .filter_map(|(at, marker)| {
            let rest = &src[at + marker.len()..];
            let (name, _) = rest.lines().next()?.split_once(": [Color; ")?;
            let (_, body) = rest.split_once("= [")?;
            let (body, _) = body.split_once("];")?;
            let members = body
                .split(',')
                .map(str::trim)
                .filter(|member| !member.is_empty())
                .map(|member| match member.strip_prefix("Palette::") {
                    Some(variant) => hexes[variant.trim_end_matches(".color()")].clone(),
                    None => member.to_string(),
                })
                .collect();
            Some((name.to_string(), members))
        })
        .collect()
}

/// The `#rrggbb` spelling. The popup backdrop's `Color::Black` blend base is
/// the one non-`Rgb` value a surface can resolve to.
fn hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        Color::Black => "#000000".to_string(),
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

fn surface_entry(surface: Surface, prose: &BTreeMap<String, String>) -> Value {
    let row = surface_table::row(surface);
    let name = format!("{surface:?}");
    let mut entry = json!({
        "focused": hex(surface_colors(surface, true).fill),
        "level": format!("{:?}", row.level),
        "resting": hex(surface_colors(surface, false).fill),
        "uses": uses(prose, "surface", &name),
        "name": name,
    });
    if row.soft {
        entry["soft"] = Value::Bool(true);
    }
    entry
}

/// The whole document rebuilt from the theme, keeping `old`'s prose.
fn regenerate(old: &Value) -> Value {
    let variants = variants();
    let rgbs: BTreeMap<String, [u8; 3]> = variants.iter().cloned().collect();
    let hexes: BTreeMap<String, String> = variants
        .iter()
        .map(|(name, [r, g, b])| (name.clone(), hex(Color::Rgb(*r, *g, *b))))
        .collect();

    let role_prose = prose(&old["roles"]);
    let roles: Vec<Value> = roles()
        .into_iter()
        .map(|(name, variant)| {
            json!({
                "hex": hexes[&variant],
                "rgb": rgbs[&variant],
                "uses": uses(&role_prose, "role", &name),
                "name": name,
                "variant": variant,
            })
        })
        .collect();

    let role_sets: serde_json::Map<String, Value> = role_sets(&hexes)
        .into_iter()
        .map(|(name, members)| {
            let prose = old["role_sets"][&name]["uses"]
                .as_str()
                .map_or_else(|| uses(&BTreeMap::new(), "role set", &name), str::to_string);
            (name, json!({ "members": members, "uses": prose }))
        })
        .collect();

    let surface_prose = prose(&old["surfaces"]);
    let mut surfaces: Vec<Value> = Surface::ALL
        .iter()
        .map(|&surface| surface_entry(surface, &surface_prose))
        .collect();
    surfaces.sort_by_key(|entry| entry["name"].as_str().unwrap_or_default().to_string());

    let variants: Vec<Value> = variants
        .into_iter()
        .map(|(name, rgb)| json!({ "hex": hexes[&name], "name": name, "rgb": rgb }))
        .collect();

    json!({
        "role_sets": role_sets,
        "roles": roles,
        "source": SOURCES,
        "specials": old["specials"],
        "surfaces": surfaces,
        "variants": variants,
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
