# Name table: 29 palette variants (task 1.2)

One row per **distinct colour**. Naming rules from design.md: keep existing
colour-words (`Gold`, `Aqua`, `Red`, `Orange`, `Purple`, `Foam`); number only the
tight clusters (`Grey1`–`Grey6` achromatic greys, `Green1`–`Green3` dark
green-greys); the three dark blue-grey slates are `Slate`/`Storm`/`Flint`; the
visually distinct greens get plain colour names; plain meaning-free hue names
everywhere else. Ordered by hue family, dark-to-light within each family.

## Derivation (auditable 42 → 29)

`src/app/render/theme/primitives.rs` declares **42 consts**: 40 `Rgb(...)` literals
plus 2 primitive→primitive aliases (`PILL_SELECTOR_OVERFLOW_FG = BG_GREEN`,
`SCROLLBAR = BG_GREEN_SOFT`). Duplicate literals collapse:

| value | consts sharing it | collapse |
| --- | --- | --- |
| `#1e2326` | `DARK_BG`, `PILL_SELECTOR_ROW_BG`, `PILL_SELECTOR_BG`, `PILL_SELECTOR_SELECTED_FG` | −3 |
| `#2d353b` | `LIBRARY_SIDE_BG`, `SELECTED_ROW_BAR`, `ARTWORK_PLACEHOLDER` (test-only) | −2 |
| `#48584e` | `SOFT_CONTENT_BODY_BG`, `BG_GREEN_SOFT` (+ alias `SCROLLBAR`) | −1 −1 alias |
| `#3f3f3f` | `OVERLAY`, `ARTWORK_LOADING_PLACEHOLDER` | −1 |
| `#3c4841` | `SURFACE_FOCUSED_BG`, `BG_GREEN` (+ alias `PILL_SELECTOR_OVERFLOW_FG`) | −1 −1 alias |
| `#3a94c5` | `FOAM`, `PILL_SELECTOR_SELECTED_BG` | −1 |
| `#35a77c` | `AQUA`, `PLAYBACK_TITLE` | −1 |
| `#dbbc7f` | `YELLOW`, `PLAYBACK_CONTEXT` | −1 |

13 removals: 42 − 13 = **29 distinct colours**. Cross-check: `docs/palette.json`
`roles[].rgb` contains exactly these 29 hexes (verified by extracting and
deduplicating both sets), and every hex below appears there.

Roles in `src/app/render/theme/mod.rs` (51 consts) are assignments over these
primitives and introduce **no new values** — each role's colour is the row of its
backing primitive, so the primitive side of the table covers every role
assignment transitively. Value-alias markings: *(value-alias of)* = duplicate
`Rgb` literal, *(alias)* = primitive→primitive alias.

## The table

| old primitive(s) | new PascalCase variant | hex | one-word description |
| --- | --- | --- | --- |
| `BASE` | `Grey1` | `#1a1a1a` | near-black |
| `OVERLAY`, `ARTWORK_LOADING_PLACEHOLDER` (value-alias of `OVERLAY`) | `Grey2` | `#3f3f3f` | dark grey |
| `FOCUSED` | `Grey3` | `#535353` | mid grey |
| `MUTED` | `Grey4` | `#6c6c6c` | dim grey |
| `SUBTLE` | `Grey5` | `#9e9e9e` | light grey |
| `TEXT` | `Grey6` | `#e6e6e6` | pale grey |
| `SURFACE_FOCUSED_BG`, `BG_GREEN` (value-alias), `PILL_SELECTOR_OVERFLOW_FG` (alias of `BG_GREEN`) | `Green1` | `#3c4841` | dark green-grey |
| `SOFT_CONTENT_BODY_BG`, `BG_GREEN_SOFT` (value-alias), `SCROLLBAR` (alias of `BG_GREEN_SOFT`) | `Green2` | `#48584e` | mid green-grey |
| `MUTED_GREEN` | `Green3` | `#6c766c` | light green-grey |
| `PLAYBACK_META_FG` | `Sage` | `#859289` | muted sage |
| `GREEN` | `Green` | `#93b259` | bright green |
| `PLAYBACK_CONTENT_FG` | `Mint` | `#83c092` | soft mint |
| `IRIS` | `Iris` | `#a7c080` | pale sage |
| `PLAYED_ROW` | `Fog` | `#bec5b2` | pale sage-grey |
| `AQUA`, `PLAYBACK_TITLE` (value-alias of `AQUA`) | `Aqua` | `#35a77c` | emerald aqua |
| `DARK_BG`, `PILL_SELECTOR_ROW_BG` (value-alias), `PILL_SELECTOR_BG` (value-alias), `PILL_SELECTOR_SELECTED_FG` (value-alias) | `Ink` | `#1e2326` | near-black blue |
| `LIBRARY_SIDE_BG`, `SELECTED_ROW_BAR` (value-alias), `ARTWORK_PLACEHOLDER` (value-alias, test-only) | `Slate` | `#2d353b` | dark slate |
| `PLAYBACK_PANEL_BG` | `Storm` | `#333c43` | mid slate |
| `PANEL_BG` | `Flint` | `#3c424a` | light slate |
| `PILL_SELECTOR_FG` | `Ash` | `#495156` | dark ash-blue |
| `SEEK_TRACK` | `Steel` | `#46545f` | steel blue |
| `FOAM`, `PILL_SELECTOR_SELECTED_BG` (value-alias of `FOAM`) | `Foam` | `#3a94c5` | bright blue |
| `PURPLE` | `Purple` | `#d699b6` | muted purple |
| `RED` | `Red` | `#e57e80` | muted red |
| `ORANGE` | `Orange` | `#e59875` | warm orange |
| `GOLD` | `Gold` | `#dea000` | deep gold |
| `YELLOW`, `PLAYBACK_CONTEXT` (value-alias of `YELLOW`) | `Yellow` | `#dbbc7f` | muted gold |
| `SOFT_WHITE` | `Cream` | `#faedcd` | pale cream |
| `WHITE` | `White` | `#fdf6e3` | warm white |

## Notes for review

- The design.md keep-list names six colour-words (`Gold`, `Aqua`, `Red`, `Orange`,
  `Purple`, `Foam`) — all kept verbatim. Three further primitives already carry
  plain colour-word names and are kept under the same rule: `YELLOW` → `Yellow`,
  `WHITE` → `White`, `GREEN` → `Green`, `IRIS` → `Iris`. Amend these rows if the
  intent was to rename them.
- `SOFT_WHITE` → `Cream` (one-word plain shade name; `SoftWhite` is the
  two-word alternative). `PLAYBACK_CONTENT_FG` `#83c092` → `Mint`,
  `PLAYBACK_META_FG` `#859289` → `Sage`, `PLAYED_ROW` `#bec5b2` → `Fog` (the
  three greenish primitives whose old names leak meaning).
- New hue names for the blue-grey primitives outside the named slate triple:
  `DARK_BG` `#1e2326` → `Ink`, `PILL_SELECTOR_FG` `#495156` → `Ash`,
  `SEEK_TRACK` `#46545f` → `Steel`. `#495156` and `#46545f` are not achromatic,
  so they cannot join `Grey1`–`Grey6`, and design.md fixes the slate cluster at
  exactly three.
- No variant name is a Rust keyword, primitive type, or `std` type name.
- Ratatui raw specials (`Color::Black`/`White`/`Reset`) are outside the palette
  (design decision) and have no rows here.
