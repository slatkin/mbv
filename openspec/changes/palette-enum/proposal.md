## Why

The TUI's 42 primitive consts hold only 29 distinct `Rgb` literals, and nothing
in the code says which of the 42 is the *owner* of a given colour. A new colour
arrives as a 43rd const with a fresh literal; a colour edit requires knowing by
hand which other consts share the value. The set of colours the TUI uses is not
enumerable from the code at all, so the hand-maintained `docs/palette.json` can
drift from it with nothing to catch it.

One closed enum owning one literal per colour fixes both: colour identity
becomes a named thing the compiler knows, and `Palette::ALL` makes the set
enumerable so the docs drift-guard becomes a test.

**Not a problem** (corrected 2026-09-19, superseding the original framing): the
duplicate literals and the "deliberately not X" comments are not a defect. They
are the edit-isolation mechanism two prior accepted changes installed on purpose
(`now-playing-media-type-titles` D2, `unify-surface-colour-neutral` 4.2):
`PLAYBACK_TITLE` is a primitive of its own precisely so a brand-aqua edit cannot
move the now-playing title. That intent is preserved by this change, not removed.

## What Changes

- **New closed `Palette` enum** (29 meaning-free variants): the sole owner of
  every palette `Rgb` literal in theme code, with `const fn color()`, an `ALL`
  list, and hex/name accessors. *(Already landed: `9b740dfb`.)*
- **Roles derive from the palette, keeping their `Color` type**:
  `pub const TEXT_X: Color = Palette::Gold.color();`. `const fn color()` makes
  this a compile-time constant, so the role tier gains a named colour identity
  with no type change and no call-site churn. Not breaking: the
  `src/app/palette.rs` re-export surface is unchanged.
- **Surface rows and level fills derive the same way**; `Row.resting`,
  `focused_fill`, and `surface_colors` keep their `Color` types, so
  `PopupDimBackdrop`'s raw `Color::Black` blend base needs no `Option` or
  wrapper.
- **Delete the two primitive→primitive aliases** (`SCROLLBAR = BG_GREEN_SOFT`,
  `PILL_SELECTOR_OVERFLOW_FG = BG_GREEN`): each becomes a direct
  `Palette::Green2.color()` / `Palette::Green1.color()` assignment. Delete
  `primitives.rs` once every const references the enum instead.
- **Keep the "deliberately not X" comments.** Two roles at one variant means
  "these share a value today, and each is independently editable" — exactly what
  the comments record. Deleting them would leave `ACCENT = Palette::Aqua` and
  `PLAYBACK_TITLE_FG = Palette::Aqua` reading as a deliberate *sameness*
  contract, which is the opposite of the recorded intent.
- **Migrate the one stray production `Rgb` literal**: `chrome_tabs.rs:168`
  (`#495156`) → `palette::PILL_FG`. Test-only literals
  (`library_hero_overlay.rs`, `media_list.rs:413,478`) are known residual — they
  are buffer assertions, not palette colours.
- **Tests**: palette-value uniqueness over `ALL`; `docs/palette.json` asserted
  equal to the enum's hexes (kills the drift). *(Already landed: `9b740dfb`.)*
- **Explicitly out of scope**: raw `Color::` specials (Black/White/Reset — blend
  base, transparency) stay with a documented rationale; Surface/Level/FocusSource
  modeling untouched; no change to any role's or surface row's type.

## Capabilities

### New Capabilities

- None — pure refactor, no spec-level behavior changes.

### Modified Capabilities

- None. `ui-design-language` req 1 (roles have one definition) and req 2 (raw
  primitives private to the theme) both still hold, with the `Palette` enum
  taking the primitive tier's place: a variant's colour changes, and only the
  roles referencing that variant expose the change.

`skip_specs: true` is set in `.openspec.yaml` for this change.

## Impact

- `src/app/render/theme/` only: `palette.rs` (landed), `mod.rs` roles,
  `surface.rs` level fills, `surface_table.rs` rows, `primitives.rs` deleted.
- One call-site file: `src/app/render/components/chrome_tabs.rs` (the stray
  literal). No other consumer changes; no frozen test file is touched.
- `docs/palette.json` gains a test-enforced hex set. Its role→colour listing and
  `docs/palette.html` stay hand-maintained (no generator exists and the repo
  forbids adding one).
- Standing principle recorded: compiler enforcement preferred over tests/lints
  wherever the enforcement is real and its cost is proportionate.
