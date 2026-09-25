//! The closed palette of UI colours
//! (openspec/changes/archive/2026-09-19-palette-enum).
//!
//! One meaning-free variant per distinct colour, one `Rgb` literal per
//! variant, owned here and nowhere else in theme code. Roles (`theme.rs`)
//! and surface rows are compiler-visible assignments over this enum; raw
//! `Color::` specials (Black/White/Reset) are ratatui mechanics and stay
//! outside the palette (design.md, "Raw `Color::` specials stay outside
//! the palette").
//!
//! Additive unit: the roles (`theme.rs`) and the surface tier consume
//! `color()`; `ALL`, `name()` and `hex()` are test-only (the uniqueness and
//! `docs/palette.json` drift tests below) and are `#[cfg(test)]`-gated —
//! structurally, with no production dead-code suppression.
//!
//! Known residual `Color::Rgb` literals outside this enum (task 4.1), none
//! of them palette colours: `components/backdrop.rs`'s dim arithmetic, which
//! halves whatever cell colour it finds, and buffer assertions in
//! `components/library_hero_overlay.rs` and `components/media_list.rs`
//! (overlay/chrome and zebra-row background expectations pinned by hex;
//! `media_list.rs:413,478` at the time of `palette-enum`). These stay as
//! literals; do not migrate them to roles.

use ratatui::style::Color;

/// The 19 approved palette variants, in name-table order (hue families,
/// dark-to-light within each family; see
/// `openspec/changes/archive/2026-09-19-palette-enum/name-table.md`).
///
/// `Copy` is required: const-context indexing out of `ALL` moves the value
/// (see the palette table drift tests below).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(in crate::app) enum Palette {
    Grey1,
    Grey2,
    Grey3,
    Green1,
    Green2,
    Green3,
    Green,
    Iris,
    Aqua,
    Ink,
    Slate,
    Storm,
    Foam,
    Mauve,
    Red,
    Orange,
    Yellow,
    Cream,
    White,
}

/// Every variant exactly once, in declaration order. The single source for
/// the uniqueness tests and the later `docs/palette.json` viewer. Test-only:
/// production code names variants directly, never through `ALL`.
#[cfg(test)]
pub(in crate::app) const ALL: [Palette; 19] = [
    Palette::Grey1,
    Palette::Grey2,
    Palette::Grey3,
    Palette::Green1,
    Palette::Green2,
    Palette::Green3,
    Palette::Green,
    Palette::Iris,
    Palette::Aqua,
    Palette::Ink,
    Palette::Slate,
    Palette::Storm,
    Palette::Foam,
    Palette::Mauve,
    Palette::Red,
    Palette::Orange,
    Palette::Yellow,
    Palette::Cream,
    Palette::White,
];

impl Palette {
    /// The variant's colour. The only place a palette `Color::Rgb(...)`
    /// literal lives in theme code.
    pub(in crate::app) const fn color(self) -> Color {
        match self {
            Palette::Grey1 => Color::Rgb(0x1a, 0x1a, 0x1a),
            Palette::Grey2 => Color::Rgb(0x9e, 0x9e, 0x9e),
            Palette::Grey3 => Color::Rgb(0xe6, 0xe6, 0xe6),
            Palette::Green1 => Color::Rgb(0x2e, 0x38, 0x3c),
            Palette::Green2 => Color::Rgb(0x37, 0x41, 0x45),
            Palette::Green3 => Color::Rgb(0x6c, 0x76, 0x6c),
            Palette::Green => Color::Rgb(0x93, 0xb2, 0x59),
            Palette::Iris => Color::Rgb(0xa7, 0xc0, 0x80),
            Palette::Aqua => Color::Rgb(0x35, 0xa7, 0x7c),
            Palette::Ink => Color::Rgb(0x1e, 0x23, 0x26),
            Palette::Slate => Color::Rgb(0x27, 0x2e, 0x33),
            Palette::Storm => Color::Rgb(0x2b, 0x32, 0x38),
            Palette::Foam => Color::Rgb(0x3a, 0x94, 0xc5),
            Palette::Mauve => Color::Rgb(0xd6, 0x99, 0xb6),
            Palette::Red => Color::Rgb(0xe5, 0x7e, 0x80),
            Palette::Orange => Color::Rgb(0xe5, 0x98, 0x75),
            Palette::Yellow => Color::Rgb(0xdb, 0xbc, 0x7f),
            Palette::Cream => Color::Rgb(0xfa, 0xed, 0xcd),
            Palette::White => Color::Rgb(0xfd, 0xf6, 0xe3),
        }
    }

    /// The variant's PascalCase name (matches the name table and the
    /// `name` field `docs/palette.json` gains in row 4.2). Test-only.
    #[cfg(test)]
    pub(in crate::app) const fn name(self) -> &'static str {
        match self {
            Palette::Grey1 => "Grey1",
            Palette::Grey2 => "Grey2",
            Palette::Grey3 => "Grey3",
            Palette::Green1 => "Green1",
            Palette::Green2 => "Green2",
            Palette::Green3 => "Green3",
            Palette::Green => "Green",
            Palette::Iris => "Iris",
            Palette::Aqua => "Aqua",
            Palette::Ink => "Ink",
            Palette::Slate => "Slate",
            Palette::Storm => "Storm",
            Palette::Foam => "Foam",
            Palette::Mauve => "Mauve",
            Palette::Red => "Red",
            Palette::Orange => "Orange",
            Palette::Yellow => "Yellow",
            Palette::Cream => "Cream",
            Palette::White => "White",
        }
    }

    /// The variant's colour as lowercase `"#rrggbb"`, formatted from
    /// `color()` so the two presentations cannot disagree. Test-only: the
    /// hex form is how the name table and `docs/palette.json` spell a
    /// variant.
    #[cfg(test)]
    pub(in crate::app) fn hex(self) -> String {
        match self.color() {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            other => panic!(
                "palette variant {} is not an Rgb colour: {other:?}",
                self.name()
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{surface_colors, surface_table, Surface};
    use super::*;
    use std::collections::{BTreeMap, HashSet};

    /// Embedded so the drift guard has no cwd/filesystem dependence.
    const PALETTE_JSON: &str = include_str!("../../../../docs/palette.json");

    /// Every variant appears in `ALL` exactly once, and no two variants
    /// share a `color()` — which `hex()` is now formatted from, so the
    /// colour check covers both presentations.
    #[test]
    fn all_lists_every_variant_once_with_distinct_values() {
        assert_eq!(ALL.len(), 19, "ALL must list exactly the 19 variants");
        let distinct: HashSet<Palette> = ALL.into_iter().collect();
        assert_eq!(
            distinct.len(),
            ALL.len(),
            "ALL contains a duplicate variant"
        );
        for (i, a) in ALL.into_iter().enumerate() {
            for b in ALL.into_iter().skip(i + 1) {
                assert_ne!(
                    a.color(),
                    b.color(),
                    "{:?} and {:?} share a Color value",
                    a.name(),
                    b.name()
                );
            }
        }
    }

    /// The green-family entries in the palette RGB table intentionally shift
    /// one step darker together: Green1 takes the new dark value and Green2
    /// takes Green1's former value.
    #[test]
    fn palette_rgb_table_tracks_the_green_shift() {
        assert_eq!(Palette::Green1.color(), Color::Rgb(0x2e, 0x38, 0x3c));
        assert_eq!(Palette::Green2.color(), Color::Rgb(0x37, 0x41, 0x45));
    }

    #[test]
    fn palette_rgb_table_tracks_the_yellow_decision() {
        assert_eq!(Palette::Yellow.color(), Color::Rgb(0xdb, 0xbc, 0x7f));
    }

    /// Collects every `#rrggbb` string and every `[r, g, b]` triple from a
    /// JSON value, at any nesting depth, into a normalized lowercase set.
    /// Structural on purpose: the collector does not care whether the hexes
    /// hang off a `roles` array or a variant-first map, so row 4.2's
    /// variant-first restructure of `docs/palette.json` needs no test
    /// change beyond what the root already covers.
    fn collect_json_hexes(value: &serde_json::Value, out: &mut HashSet<String>) {
        match value {
            serde_json::Value::String(s) => {
                if s.len() == 7
                    && s.starts_with('#')
                    && s[1..].bytes().all(|b| b.is_ascii_hexdigit())
                {
                    out.insert(s.to_ascii_lowercase());
                }
            }
            serde_json::Value::Array(items) => {
                if let Some(rgb) = rgb_triple(items) {
                    out.insert(format!("#{:02x}{:02x}{:02x}", rgb[0], rgb[1], rgb[2]));
                }
                for item in items {
                    collect_json_hexes(item, out);
                }
            }
            serde_json::Value::Object(map) => {
                for v in map.values() {
                    collect_json_hexes(v, out);
                }
            }
            _ => {}
        }
    }

    fn rgb_triple(items: &[serde_json::Value]) -> Option<[u8; 3]> {
        if items.len() != 3 {
            return None;
        }
        let mut rgb = [0u8; 3];
        for (slot, item) in rgb.iter_mut().zip(items) {
            *slot = item.as_u64()?.try_into().ok()?;
        }
        Some(rgb)
    }

    /// Every production role const in `theme.rs` as `(name, variant
    /// name)`, in declaration order. Test-only aliases are not roles and do
    /// not belong in the docs.
    ///
    /// Parsed from the source rather than listed here so the guard needs no
    /// second inventory to keep in sync: a new role const is checked the
    /// moment it is written.
    fn code_roles() -> Vec<(String, String)> {
        let mut roles = Vec::new();
        let mut cfg_test = false;
        for line in include_str!("../theme.rs").lines() {
            let line = line.trim();
            if line.starts_with("#[cfg(test)]") {
                cfg_test = true;
            } else if line.starts_with("#[") {
                continue;
            } else {
                if let Some(rest) = line.strip_prefix("pub const ") {
                    if let Some((name, value)) = rest.split_once(": Color = Palette::") {
                        if !cfg_test {
                            let variant = value.split('.').next().unwrap_or_default();
                            roles.push((name.to_string(), variant.to_string()));
                        }
                    }
                }
                cfg_test = false;
            }
        }
        roles
    }

    /// The `#rrggbb` form `docs/palette.json` spells a colour with. The one
    /// non-`Rgb` value a surface row can carry is the popup dim backdrop's
    /// `Color::Black` blend base, which the docs record as `#000000` (it is
    /// not a palette colour, which is why the variant drift guard skips the
    /// `surfaces` subtree).
    fn hex(color: Color) -> String {
        match color {
            Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
            Color::Black => "#000000".to_string(),
            other => panic!("surface/role colour is not an Rgb colour: {other:?}"),
        }
    }

    fn json_field(value: &serde_json::Value, field: &str) -> String {
        value[field]
            .as_str()
            .unwrap_or_else(|| panic!("palette.json entry {value} has no string `{field}`"))
            .to_string()
    }

    /// Each variant's `(hex, rgb)` in the docs' spelling, keyed by name.
    fn variant_fields() -> BTreeMap<String, (String, [u8; 3])> {
        ALL.into_iter()
            .map(|variant| {
                let Color::Rgb(r, g, b) = variant.color() else {
                    panic!("palette variant {:?} is not an Rgb colour", variant.name());
                };
                (variant.name().to_string(), (variant.hex(), [r, g, b]))
            })
            .collect()
    }

    /// The `uses` prose of every entry in one docs array, keyed by name.
    fn prose_by_name(json: &serde_json::Value, array: &str) -> BTreeMap<String, String> {
        json[array]
            .as_array()
            .unwrap_or_else(|| panic!("docs/palette.json {array} must be an array"))
            .iter()
            .map(|entry| (json_field(entry, "name"), json_field(entry, "uses")))
            .collect()
    }

    /// Drift guard: `docs/palette.json`'s `roles` array must list exactly the
    /// production role consts, each naming its own variant and that variant's
    /// hex. The prose (`uses`) stays hand-written; the mechanical fields are
    /// pinned here.
    #[test]
    fn docs_palette_json_lists_every_role_with_its_variant_and_hex() {
        if updating() {
            return;
        }
        let json: serde_json::Value =
            serde_json::from_str(PALETTE_JSON).expect("docs/palette.json must be valid JSON");
        let variant_hexes = variant_fields();
        let expected: BTreeMap<String, (String, String)> = code_roles()
            .into_iter()
            .map(|(name, variant)| {
                let (hex, _) = variant_hexes
                    .get(&variant)
                    .unwrap_or_else(|| panic!("role {name} names unknown variant {variant}"));
                (name, (variant, hex.clone()))
            })
            .collect();
        let actual: BTreeMap<String, (String, String)> = json["roles"]
            .as_array()
            .expect("docs/palette.json roles must be an array")
            .iter()
            .map(|role| {
                let name = json_field(role, "name");
                let variant = json_field(role, "variant");
                let hex = json_field(role, "hex");
                (name, (variant, hex))
            })
            .collect();
        assert_eq!(
            actual, expected,
            "docs/palette.json roles drifted from the role consts in theme.rs"
        );
    }

    /// Drift guard: `docs/palette.json`'s `surfaces` array must list exactly
    /// the closed `Surface` set, each with the level and the focused/resting
    /// fills the resolver actually paints (the `soft` flag too, when set).
    #[test]
    fn docs_palette_json_lists_every_surface_row() {
        if updating() {
            return;
        }
        let json: serde_json::Value =
            serde_json::from_str(PALETTE_JSON).expect("docs/palette.json must be valid JSON");
        let expected: BTreeMap<String, (String, String, String, bool)> = Surface::ALL
            .iter()
            .map(|&surface| {
                let row = surface_table::row(surface);
                (
                    format!("{surface:?}"),
                    (
                        format!("{:?}", row.level),
                        hex(surface_colors(surface, true).fill),
                        hex(surface_colors(surface, false).fill),
                        row.soft,
                    ),
                )
            })
            .collect();
        let actual: BTreeMap<String, (String, String, String, bool)> = json["surfaces"]
            .as_array()
            .expect("docs/palette.json surfaces must be an array")
            .iter()
            .map(|surface| {
                let name = json_field(surface, "name");
                (
                    name,
                    (
                        json_field(surface, "level"),
                        json_field(surface, "focused"),
                        json_field(surface, "resting"),
                        surface["soft"].as_bool().unwrap_or(false),
                    ),
                )
            })
            .collect();
        assert_eq!(
            actual, expected,
            "docs/palette.json surfaces drifted from the surface table"
        );
    }

    /// Drift guard: the distinct colour values in `docs/palette.json` must
    /// be exactly the palette's 19 `hex()` values. The `surfaces` and
    /// `specials` subtrees are deliberately excluded because they contain
    /// surface-specific and raw `Color::` values, respectively
    /// (design.md, "Raw `Color::` specials stay outside the palette").
    /// Row 4.2 retargets the JSON to the variant-first structure; the
    /// structural collector above already tolerates that restructure.
    #[test]
    fn docs_palette_json_matches_the_palette() {
        let json: serde_json::Value =
            serde_json::from_str(PALETTE_JSON).expect("docs/palette.json must be valid JSON");
        let mut json_hexes = HashSet::new();
        if let serde_json::Value::Object(map) = &json {
            for (key, value) in map {
                if key == "surfaces" || key == "specials" {
                    continue;
                }
                collect_json_hexes(value, &mut json_hexes);
            }
        }
        let palette_hexes: HashSet<String> = ALL.into_iter().map(|p| p.hex()).collect();
        assert_eq!(
            palette_hexes, json_hexes,
            "docs/palette.json colours drifted from the Palette enum"
        );
    }

    /// `true` while the update mode below is rewriting the docs. The guards
    /// step aside then: the embedded copy they read is the one being replaced,
    /// so a rewrite would otherwise fail its own run.
    fn updating() -> bool {
        std::env::var_os("PALETTE_JSON_UPDATE").is_some()
    }

    /// `PALETTE_JSON_UPDATE=1 cargo nextest run -p mbv palette_json` rewrites
    /// `docs/palette.json`'s mechanical fields from this tree: every variant
    /// row, every role's `hex`/`rgb`/`variant`, and every surface's
    /// `level`/`focused`/`resting`/`soft`. The hand-written `uses` prose is
    /// carried over by name, and the two arrays keep the file's own order
    /// (roles in const order, surfaces alphabetically).
    ///
    /// Entries this tree has and the file does not are added with empty prose
    /// and named on stderr; entries the file has and this tree does not are
    /// dropped and named too. The prose is the human's half — the two guards
    /// above stay the proof, this test only writes.
    #[test]
    fn palette_json_regenerates_the_mechanical_fields_when_asked() {
        if !updating() {
            return;
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/palette.json");
        let mut json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read docs/palette.json"))
                .expect("docs/palette.json must be valid JSON");
        let fields = variant_fields();

        json["variants"] = serde_json::Value::Array(
            ALL.into_iter()
                .map(|variant| {
                    let (hex, rgb) = &fields[variant.name()];
                    serde_json::json!({"hex": hex, "name": variant.name(), "rgb": rgb})
                })
                .collect(),
        );

        let mut role_prose = prose_by_name(&json, "roles");
        json["roles"] = serde_json::Value::Array(
            code_roles()
                .into_iter()
                .map(|(name, variant)| {
                    let (hex, rgb) = &fields[variant.as_str()];
                    let uses = role_prose.remove(&name).unwrap_or_else(|| {
                        eprintln!("palette.json: new role {name} — fill in its `uses` prose");
                        String::new()
                    });
                    serde_json::json!({
                        "hex": hex, "name": name, "rgb": rgb, "uses": uses, "variant": variant
                    })
                })
                .collect(),
        );
        for orphan in role_prose.keys() {
            eprintln!("palette.json: role {orphan} is no longer a const — dropped");
        }

        let mut surface_prose = prose_by_name(&json, "surfaces");
        let mut surfaces: Vec<serde_json::Value> = Surface::ALL
            .iter()
            .map(|&surface| {
                let row = surface_table::row(surface);
                let name = format!("{surface:?}");
                let uses = surface_prose.remove(&name).unwrap_or_else(|| {
                    eprintln!("palette.json: new surface {name} — fill in its `uses` prose");
                    String::new()
                });
                // Built field by field so `soft` lands in the file's
                // alphabetical key order rather than at the end.
                let mut entry = serde_json::Map::new();
                entry.insert(
                    "focused".into(),
                    serde_json::json!(hex(surface_colors(surface, true).fill)),
                );
                entry.insert(
                    "level".into(),
                    serde_json::json!(format!("{:?}", row.level)),
                );
                entry.insert("name".into(), serde_json::json!(name));
                entry.insert(
                    "resting".into(),
                    serde_json::json!(hex(surface_colors(surface, false).fill)),
                );
                if row.soft {
                    entry.insert("soft".into(), serde_json::Value::Bool(true));
                }
                entry.insert("uses".into(), serde_json::json!(uses));
                serde_json::Value::Object(entry)
            })
            .collect();
        for orphan in surface_prose.keys() {
            eprintln!("palette.json: surface {orphan} is no longer a row — dropped");
        }
        surfaces.sort_by_key(|surface| json_field(surface, "name"));
        json["surfaces"] = serde_json::Value::Array(surfaces);

        std::fs::write(
            path,
            serde_json::to_string(&json).expect("serialize docs/palette.json"),
        )
        .expect("write docs/palette.json");
        eprintln!("docs/palette.json regenerated from the theme");
    }
}
