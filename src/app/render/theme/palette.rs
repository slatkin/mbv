//! The closed palette of UI colours
//! (openspec/changes/archive/2026-09-19-palette-enum).
//!
//! One meaning-free variant per distinct colour, one `Rgb` literal per
//! variant, owned here and nowhere else in theme code. Roles (`mod.rs`)
//! and surface rows are compiler-visible assignments over this enum; raw
//! `Color::` specials (Black/White/Reset) are ratatui mechanics and stay
//! outside the palette (design.md, "Raw `Color::` specials stay outside
//! the palette").
//!
//! Additive unit: the roles (`mod.rs`) and the surface tier consume
//! `color()`; `ALL`, `name()` and `hex()` are test-only (the uniqueness and
//! `docs/palette.json` drift tests below) and are `#[cfg(test)]`-gated —
//! structurally, never `#[allow(dead_code)]`.
//!
//! Known residual `Color::Rgb` literals outside this enum (task 4.1), none
//! of them palette colours: `components/backdrop.rs`'s dim arithmetic, which
//! halves whatever cell colour it finds, and buffer assertions in
//! `components/library_hero_overlay.rs` and `components/media_list.rs`
//! (overlay/chrome and zebra-row background expectations pinned by hex;
//! `media_list.rs:413,478` at the time of `palette-enum`). These stay as
//! literals; do not migrate them to roles.

use ratatui::style::Color;

/// The 21 approved palette variants, in name-table order (hue families,
/// dark-to-light within each family; see
/// `openspec/changes/archive/2026-09-19-palette-enum/name-table.md`).
///
/// `Copy` is required: const-context indexing out of `ALL` moves the value
/// (design.md spike outcomes, task 1.1).
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub(in crate::app) enum Palette {
    Grey1,
    Grey2,
    Grey4,
    Grey5,
    Grey6,
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
    Purple,
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
pub(in crate::app) const ALL: [Palette; 21] = [
    Palette::Grey1,
    Palette::Grey2,
    Palette::Grey4,
    Palette::Grey5,
    Palette::Grey6,
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
    Palette::Purple,
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
            Palette::Grey2 => Color::Rgb(0x3f, 0x3f, 0x3f),
            Palette::Grey4 => Color::Rgb(0x6c, 0x6c, 0x6c),
            Palette::Grey5 => Color::Rgb(0x9e, 0x9e, 0x9e),
            Palette::Grey6 => Color::Rgb(0xe6, 0xe6, 0xe6),
            Palette::Green1 => Color::Rgb(0x3c, 0x48, 0x41),
            Palette::Green2 => Color::Rgb(0x48, 0x58, 0x4e),
            Palette::Green3 => Color::Rgb(0x6c, 0x76, 0x6c),
            Palette::Green => Color::Rgb(0x93, 0xb2, 0x59),
            Palette::Iris => Color::Rgb(0xa7, 0xc0, 0x80),
            Palette::Aqua => Color::Rgb(0x35, 0xa7, 0x7c),
            Palette::Ink => Color::Rgb(0x1e, 0x23, 0x26),
            Palette::Slate => Color::Rgb(0x2d, 0x35, 0x3b),
            Palette::Storm => Color::Rgb(0x33, 0x3c, 0x43),
            Palette::Foam => Color::Rgb(0x3a, 0x94, 0xc5),
            Palette::Purple => Color::Rgb(0xd6, 0x99, 0xb6),
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
            Palette::Grey4 => "Grey4",
            Palette::Grey5 => "Grey5",
            Palette::Grey6 => "Grey6",
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
            Palette::Purple => "Purple",
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
    use super::*;
    use std::collections::HashSet;

    /// Embedded so the drift guard has no cwd/filesystem dependence.
    const PALETTE_JSON: &str = include_str!("../../../../docs/palette.json");

    /// Every variant appears in `ALL` exactly once, and no two variants
    /// share a `color()` — which `hex()` is now formatted from, so the
    /// colour check covers both presentations.
    #[test]
    fn all_lists_every_variant_once_with_distinct_values() {
        assert_eq!(ALL.len(), 21, "ALL must list exactly the 21 variants");
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

    /// Drift guard: the distinct colour values in `docs/palette.json` must
    /// be exactly the palette's 21 `hex()` values. The `surfaces` and
    /// `specials` subtrees are deliberately excluded — `surfaces` contains
    /// the `PopupDimBackdrop` `#000000` dim blend base, which is not a
    /// palette colour, and `specials` names raw `Color::` mechanics
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
}
