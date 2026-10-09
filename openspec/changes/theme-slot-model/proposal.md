# Proposal

## Why

mbv will soon be themeable, but its colour model cannot be swapped as a set. Two authorities
decide fills: 57 `pub const` roles in `mbv-theme/src/lib.rs` (about 40 of whose uses are
background fills) and the `Surface` table with its `Level`/`FocusSource`/`soft`/
`RESTING_DEVIATIONS` machinery. Some painters take one half of a fill from each
(`hero_composition.rs:190`). The 19 palette variants carry hue names that are already wrong
(`Iris` is sage green, `Green1`/`Green2` are background steps), and roles are compile-time
`Color` constants, so a second theme would need every call site touched. This change
reshapes the model so a theme is one defined slot set. No colour changes.

## What Changes

- The 19 `Palette` variants become 19 `Slot`s named by job: a background ladder (`BgDim`,
  `Bg0`–`Bg3`), a foreground ladder (`OnAccent`, `FgFaint`, `FgMuted`, `Fg`, `FgWarm`,
  `FgBright`) and accent hues (`Red`, `Orange`, `Yellow`, `Green`, `GreenDeep`, `Aqua`,
  `Blue`, `Purple`). A `Theme` value holds one RGB per slot; `Theme::DEFAULT` holds
  today's exact values.
- Roles and surfaces resolve through the active theme at paint time. `active()` returns
  `&Theme::DEFAULT`; selecting another theme is out of scope.
- **BREAKING (internal API)**: the 57 `pub const ROLE: Color` constants become a closed
  `Role` enum resolved with `.color()`. The two role arrays (`HERO_META_ROLES`,
  `HINT_PILL_FILLS`) become functions or slices of identities. About 420 references
  across `mbv-render`, `mbv-components` and the `mbv` binary change mechanically.
- One authority per kind: every background fill is a `Surface`, and every foreground
  (text, indicator, scrollbar, underline) is a `Role`. Background-role uses move into the
  Surface table.
- The Surface table becomes one `(resting, focused)` slot pair per surface. `Level`,
  `FocusSource`, the `soft` flag, `RESTING_DEVIATIONS` and the resolver's `debug_assert`
  are deleted.
- Fill identities that name the same concept merge: `WORKSPACE_FOCUSED_FILL` folds into
  `Surface::MainContentBox`, the context-menu selected row and `SELECTED_ROW_BG` become
  one `Surface::SelectedRow`, and the three fixed list stripes (playlists, settings,
  sessions) become one `Surface::ListStripe`. Text roles move 1:1.
- `media_list/row.rs` stops deciding "on the selection bar" by comparing colour values.
- The `docs/palette.json` generator iterates the slot, role and surface enums instead of
  parsing theme source text.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `ui-design-system`: the closed palette requirement becomes a closed slot set plus a
  theme value, with roles and surfaces as fixed mappings over slots.
- `ui-design-language`: the surface table drops nesting levels; fills resolve only
  through surfaces and foregrounds only through roles; one concept takes one identity.

## Impact

- `crates/mbv-theme`: rewritten internals (`palette.rs`, `lib.rs`, `surface*.rs`,
  `palette_json.rs`).
- `crates/mbv-render` (about 37 files), `crates/mbv-components` (about 12 files) and the
  `mbv` binary's projections and shell (about 10 files): mechanical call-site edits.
- `docs/palette.json`: names change; every resolved hex stays the same.
- Out of scope: `Color::White`/`Black`/`Reset` specials, the pinwin `[panel] accent_color`
  default, mpv Lua overlay colours, theme selection or config, and pruning text roles.
