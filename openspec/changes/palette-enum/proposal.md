## Why

The TUI palette is 51 role consts (47 production, 4 test-only) over 29 actual colours, resolved through chains up to five hops deep (call site → Surface → Row → role → primitive → sometimes another primitive → `Rgb`). The 42 primitive consts include duplicate `Rgb` literals and primitive-to-primitive aliases (`SCROLLBAR = BG_GREEN_SOFT`) that defeat the edit-isolation the code comments claim, and the hand-maintained `docs/palette.json` can drift from the code with nothing to catch it. One palette with many assignments, enforced by the compiler, replaces convention with structure.

## What Changes

- **New closed `Palette` enum** (29 meaning-free variants): the sole owner of every `Rgb` literal in theme code, with `const fn color()`, an `ALL` list, and hex/name accessors.
- **Roles become assignments**: `pub const TEXT_X: Palette = Palette::Gold`. Role names and call-site meanings are unchanged; only the type changes. **BREAKING** (internal API: `src/app/palette.rs` re-export surface).
- **Surfaces resolve to `Palette`**: `Row.resting`, level fills, and `surface_colors` return `Palette`; conversion to ratatui `Color` happens at the paint boundary.
- **Delete** duplicate-literal primitives, primitive→primitive aliases, and the dozen "deliberately not X" comments, replaced by one palette rule: new colour = new palette entry, no inline literals in theme code.
- **Migrate stray production `Rgb` literals** in render components (`chrome_tabs.rs:168`, `media_list.rs:413,478`) to palette role references. Test-only `Rgb` literals (e.g. `library_hero_overlay.rs` buffer assertions) are known residual — not palette colours.
- **Role arrays** (`HERO_META_ROLES`, `HINT_PILL_FILLS`) become `[Palette; 3]`; call sites convert at use.
- **Frozen-test bridge**: retired `#[cfg(test)]` names survive only as thin `Color` shims over the enum (pending MSRV `const fn` spike; if it fails, unfreezing those tests is a rule change, not a code change).
- **Tests**: palette-value uniqueness over `ALL`; `docs/palette.json` asserted equal to the enum (kills the drift).
- **Explicitly out of scope**: raw `Color::` specials (Black/White/Reset — blend base, transparency, not palette colours) stay with a documented rationale; Surface/Level/FocusSource modeling is untouched.

## Capabilities

### New Capabilities

- None — pure refactor, no spec-level behavior changes.

### Modified Capabilities

- None. `ui-design-language` still holds: colours are named roles (req 1), raw primitives stay private to the theme (req 2), surfaces resolve through the one function (req 4). This change strengthens req 1's "change once, applies everywhere" by making sharing compiler-visible.

`skip_specs: true` is set in `.openspec.yaml` for this change.

## Impact

- `src/app/render/theme/` (primitives → palette enum, roles, surface table/resolver), `src/app/palette.rs` bridge.
- ~65 call-site files: mechanical `.fg(role)` → `.fg(role.color())` (or `Into`) churn; no visual change.
- `docs/palette.json` becomes test-verified output of the enum (variant-first: each variant carries name, hex, and assigned roles); `docs/palette.html` restructured to group by palette variant (replaces current role-first layout).
- Standing principle recorded: compiler enforcement preferred over tests/lints wherever possible.
