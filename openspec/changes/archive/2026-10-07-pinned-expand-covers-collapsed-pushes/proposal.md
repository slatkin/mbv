# Proposal

## Why

Issue #898. With a Pinned panel docked on one side, expanding it moves the tiled windows. The single `[panel] cover` bool is applied to both widths, so only "push at both widths" and "cover at both widths" exist. The pattern pinwin is built for, a collapsed panel that reserves a strip and an expanded panel that draws over the tiles while that strip is held, cannot be expressed. The `cover` setting is also poorly named and unused, so it goes away.

## What Changes

- The collapsed Pinned panel always pushes: the compositor reserves the collapsed strip.
- The expanded Pinned panel always covers: it draws over the tiled windows and holds the collapsed strip. Expanding or collapsing moves no tiled window.
- **BREAKING**: remove the `[panel] cover` setting. A leftover `cover` key in `config.toml` is ignored. Full-overlay at the collapsed width is no longer available.
- Remove the `cover` field from `PanelConfig`, with its parse, its save, and its entry in `dist/config.toml`.
- Update the *Panel covering* term in `CONTEXT.md` to describe the fixed behavior.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `pinned-launch`: the `[panel]` key list loses `cover`; the "Panel covering mode" requirement becomes a fixed push-collapsed / cover-expanded rule; the width-toggle requirement states that tiled windows do not move.

## Impact

- `src/pin.rs`: `layout_from_config` and its test.
- `crates/mbv-config`: `panel.rs`, `parse.rs`, `save.rs`, `tests/settings.rs`.
- `dist/config.toml`, `CONTEXT.md`.
- `openspec/specs/pinned-launch/spec.md` (via the delta spec).
- No change to the pinwin dependency.
