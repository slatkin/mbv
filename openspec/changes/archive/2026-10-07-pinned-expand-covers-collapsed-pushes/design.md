# Design

## Context

See proposal.md for motivation. `layout_from_config` (`src/pin.rs`) builds every pinwin `Layout`, for the start path and for each `apply_layout`. It reads `config.cover` and calls `.covering()` on both widths. pinwin already supports the push-then-cover excursion: a covering target on the same side with the same gutters leaves the held reservation alone (issue #898, verified upstream). No pinwin change is needed.

## Goals / Non-Goals

**Goals:**
- Collapsed pushes and expanded covers, decided by width alone, in the one function that builds layouts.
- Delete the `cover` setting so no config can contradict the rule.

**Non-Goals:**
- A per-width or tri-state setting. The product owner wants one fixed mode.
- A migration warning for an old `cover` key. It was unused and the parser already ignores unknown keys.

## Decisions

**Branch on `PinnedWidth` inside `layout_from_config`.** `Collapsed` returns the plain layout, `Expanded` returns `layout.covering()`. The `width` argument is already there, so no signature changes and no caller changes. Alternative: build both layouts in config and pick one at apply time. Rejected: it spreads one rule over two places.

**Remove the field instead of defaulting it.** A bool that no code reads is a lie in the type. Removing `PanelConfig.cover` forces parse, save and the tests to change in one compile pass, so nothing keeps a dead key alive.

**Leave unknown keys alone.** `parse.rs` reads named keys only, so an old `cover = ...` line is ignored with no new code. The next F2 save rewrites `[panel]` without it.

## Risks / Trade-offs

- [A user with `cover = true` loses full-overlay at the collapsed width] → Accepted by the product owner; the setting was never used.
- [Expanded layout arriving while hidden] → Hidden state follows the width, as the delta spec states; pinwin already handles hiding a covering panel without moving tiles.
- [The excursion only holds if side and gutters match between widths] → Both layouts come from the same config values, so they always match.
