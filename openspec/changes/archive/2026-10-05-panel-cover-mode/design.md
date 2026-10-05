# Design

## Context

Pinwin tag `0.2.2` (commit `b910591`) ships per-layout coverage: `pinwin::layout::Coverage`
(`Push`, the default, and `Cover`) and `Layout::covering(self) -> Self`, a const builder that
opts one layout into covering. `Surfaces` consumes `layout.coverage()` when it reserves the
tiled-window gap and tweens a coverage-only apply. mbv builds its layouts in
`src/pin.rs::layout_from_config` with `Layout::new(..)` alone, so every layout is `Push` today.

`PanelConfig` already carries one boolean knob wired end to end (`accent`): a typed field with a
default in `panel.rs`, a `panel_bool` parse with per-key fallback in `parse.rs`, a save in
`save.rs`, and a settings round-trip test.

## Goals / Non-Goals

**Goals:**
- A `[panel] cover` key that selects pinwin's covering mode for the pinned panel.

**Non-Goals:**
- An F2 row or a runtime toggle (the choice is made at layout construction; the user edits
  `config.toml` and restarts `mbv --pin`).
- Any pinwin source change or pin bump.
- A per-width (collapsed/expanded) split: one key covers both widths, like `side`.

## Decisions

**D1: Config shape mirrors `accent`.** `PanelConfig` gains `cover: bool`, default `false`
(pinwin's `Coverage::Push` is the default, so mbv's default must not change today's behaviour).
Parse with the existing `panel_bool` per-key fallback warning; save next to the other panel
keys so a round trip keeps the value.

**D2: The choice lives in `layout_from_config`.** Both `Panel::start` and
`Panel::apply_layout_animated` take their layout from `layout_from_config`, so applying
`.covering()` there keeps every built layout consistent with the config. `Layout::covering` is
a const builder on the layout value, so the wiring is:

```rust
let layout = Layout::new(..);
if config.cover { layout.covering() } else { layout }
```

Because there is no F2 row, the running panel never sees a changed value in practice: the shell
holds its loaded config in memory and a hand edit needs a relaunch.

**D3: Config-only, launch-fixed.** The spec keeps the F2 row rule ("one row per value") for the
rows that exist and states `cover` is read from `config.toml` only. A future F2 row or keybind
would reuse the same `layout_from_config` path and is out of scope here.

## Risks / Trade-offs

- [A covering panel hides content under it] → covering is opt-in per config, the default is
  unchanged, and the key is documented in `dist/config.toml` with its effect.
- [No F2 row means no discoverability in the UI] → accepted for this change; the key sits in the
  documented `[panel]` section beside the other launch-fixed keys.
