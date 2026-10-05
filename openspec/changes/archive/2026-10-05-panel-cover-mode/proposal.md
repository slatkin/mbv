# Proposal

## Why

The pinned panel docks beside tiled windows: the compositor reserves a strip for it and tiled
windows move aside. Pinwin tag `0.2.2` (the revision mbv pins, commit `b910591`) adds an opt-in
per-layout covering mode: the panel draws over the tiled windows instead, and the compositor
reserves nothing. Pushing stays the default. mbv builds every layout with the default, so the
user cannot choose covering. This change exposes the choice as a `[panel]` config key.

## What Changes

- New `[panel]` key `cover` (boolean, default `false`). `false` keeps today's pushing behaviour;
  `true` makes `mbv --pin` build the panel layout in pinwin's covering mode, so the panel draws
  over tiled windows.
- `src/pin.rs` applies the choice where the layout is built (`layout_from_config`), for the start
  layout and every later apply. There is no runtime or compositor-side lever: pinwin takes the
  choice only at layout construction.
- No F2 row, no keybind, no pinwin change, no pin bump: the key is read from `config.toml` only,
  and pin tag `0.2.2` already ships `Layout::covering`.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pinned-launch`: `Panel layout settings` gains `cover`; the F2 Panel page holds one row per
  value except `cover`. New requirement `Panel covering mode`.

## Impact

- `crates/mbv-config`: `PanelConfig` field and default, parse, save, settings test.
- `src/pin.rs`: `layout_from_config` applies `.covering()` when `cover` is set; layout test
  extended.
- Docs: `dist/config.toml` (`[panel]` key), `CONTEXT.md` (new term).
- No protocol, daemon, UI-model, keybind, or pinwin source changes.
