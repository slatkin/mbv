# Proposal

## Why

The pinned panel has one width (`[panel] cols`). A narrow panel suits glancing at playback, a
wide one suits browsing, and today switching means stepping `Cols` in F2 one column (or ten)
at a time. A single key that flips between two saved widths makes both usable.

## What Changes

- New `[panel] cols_expanded` setting (same range as `cols`, default 80), with an `Expanded cols`
  row on the F2 Panel page.
- New configurable keybind action `pinned_width_toggle` (default `Ctrl+e`, Global section) that
  flips the running pinned panel between `cols` (collapsed) and `cols_expanded` (expanded).
  Expanding retiles: the reservation grows with the width, as any `cols` change does today.
- The active width is runtime state only; every pinned launch starts collapsed.
- While pinned, editing `Cols` or `Expanded cols` in F2 switches the panel to that width so the
  edited value is the one pinwin validates; side and gutter edits apply at the active width.
- No pinwin change, no CLI flag, no IPC. A WM global key (CLI flag) is a possible follow-up.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pinned-launch`: `Panel layout settings` gains `cols_expanded` and its F2 row; new
  requirement `Pinned panel width toggle`.

## Impact

- `crates/mbv-config` (`PanelConfig`, parse, save), `crates/mbv-ui-model` (Panel setting key and
  row, `changed_panel_config`), `crates/mbv-keybinds` (registry entry), `src/app/input/key_policy.rs`
  (policy entry → `Command`), `src/app/dispatch/` (toggle + width-aware settings apply), `src/pin.rs`
  (layout built from config plus active width), `CONTEXT.md` (new term), `README.md` and `dist/config.toml` (`[panel]` key docs).
- No protocol, daemon, or pinwin changes.
