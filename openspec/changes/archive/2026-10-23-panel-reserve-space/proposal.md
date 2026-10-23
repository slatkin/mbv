# Proposal

## Why

Today the pinned panel always reserves its strip beside the tiled windows at the collapsed
width, and the spec forbids a setting for it ("Panel width decides pushing or covering … with
no setting for either"). Some users want the panel to draw over the tiled windows instead, so
no window moves when the panel shows. pinwin already supports both modes (`push` and
`cover`), so mbv can offer the choice without any pinwin change.

## What Changes

- A new `[panel] reserve` boolean in `config.toml` (default `true`). `true` keeps the
  current behavior: the collapsed panel pushes and reserves its strip. `false` makes the
  collapsed panel cover: it draws over the tiled windows and reserves nothing. The expanded
  width always covers, unchanged.
- A new `Reserve space` toggle row on the F2 Panel page. While pinned at the collapsed width
  it sends one `pinwin set-reserve` request like the other live rows. Enabling takes effect
  immediately: pinwin reserves the strip at the collapsed width. Disabling switches the panel
  to covering but cannot release the strip a push already reserved — pinwin's `cover` holds
  the last pushed gap by design — so a neutral toast says the reserved space is released on
  the next `mbv --pin` launch (slatkin/pinwin#59 tracks live release). While pinned at the
  expanded width the row only saves: pushing there would reserve the full expanded strip and
  contradict "expanded always covers", so the saved value takes effect on the next collapse.
- Every `set-size` request that names the collapsed width (`x` collapse, the F2 `Width` row)
  carries `--reserve cover` instead of `push` while `reserve` is off, so no request
  re-reserves the strip.

## Capabilities

### New Capabilities

### Modified Capabilities

- `pinned-launch`: the `Panel layout settings` requirement gains the `reserve` key and its
  F2 row; the `Panel width decides pushing or covering` requirement changes from "no setting
  for either" to a `reserve` setting that picks the collapsed mode (expanded always covers).

## Impact

- `crates/mbv-config`: `PanelConfig.reserve` field, parse, save, default `true`.
- `crates/mbv-ui-model`: `SettingKey::PanelReserve` row on the Panel page.
- `src/pin.rs`: `run_argv` picks `push` or `cover` from `reserve`; `pinned_width_argv` maps
  collapsed to the configured mode; a new `PinnedSettingRequest::Reserve` variant builds one
  `set-reserve` argv.
- `src/app/dispatch/settings.rs`: dispatch the new row; on an accepted disable while pinned,
  flash the next-launch note.
- pinwin: no change required. slatkin/pinwin#59 tracks releasing a held strip live; when it
  lands, only the `set-reserve` argv changes.
- pinwin version floor stays 0.4.1: `set-reserve` exists since 0.4.1.
