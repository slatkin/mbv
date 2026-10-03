# Proposal

## Why

niri draws no focus ring on layer-shell surfaces, so the pinned panel gives no sign that it holds
the keyboard. pinwin `ef778e4` (validation tests and spec fold in `dc46c61`) adds a focus accent: a strip on the panel's workspace-facing edge,
drawn while the panel has keyboard focus. That commit also grows the `PinwinStartup` ABI struct.
mbv mirrors `PinwinStartup` in Rust, so moving the pin to `ef778e4` without changing the mirror is
undefined behaviour. mbv has to adopt the accent and let the user configure it.

## What Changes

- Bump the pinwin pin from `664e195` to `dc46c61`. Mirror `PinwinAccent` in
  `crates/mbv-pinwin/src/ffi.rs` and add it to `PinwinStartup` after `keyboard_mode`.
  `Panel::start` takes a typed accent: `Option<Accent>`, where `None` means off.
- New `[panel]` keys `accent` (bool, default `true`), `accent_color` (`"#RRGGBB"` or `"RRGGBB"`,
  default `"#dabc7f"`, niri's focus-ring colour) and `accent_width` (pixels 1 through 65535,
  default 1). An out-of-range or malformed value falls back to its default with a logged warning,
  as the other `[panel]` keys do.
- The F2 Panel page gains three rows:
  - `Accent` toggles on and off.
  - `Accent width` steps by 1, or by 10 with Shift.
  - `Accent color` cycles through a short fixed list of colours plus the configured value.
- The accent is fixed when `pinwin_start` runs, because pinwin has no runtime accent API. A
  change to an accent row is saved, takes effect on the next `mbv --pin` launch, and is announced
  with a neutral toast while pinned.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `pinwin-panel`: `Keyboard focus by clicking` gains the focus accent and its scenarios, mirroring
  pinwin's archived `add-focus-accent`.
- `pinned-launch`: `Panel layout settings` gains the three accent keys and F2 rows. New
  requirement `Focus accent applies at launch`.

## Impact

- Crates and files that change:
  - `crates/mbv-pinwin`: `build.zig.zon` pin, `ffi.rs` mirror, `Panel::start` signature, accent type.
  - `crates/mbv-config`: `PanelConfig`, parse, save.
  - `crates/mbv-ui-model`: three setting keys and rows, `changed_panel_config`.
  - `src/pin.rs`: passes the accent from config at start.
  - `src/app/dispatch/settings.rs`: an accent edit is saved and toasted without a live apply.
  - Docs: `README.md`, `dist/config.toml`, `CONTEXT.md`.
- No protocol, daemon or pinwin source changes.
- Depends on `panel-expand-toggle`, which moves the pin to `664e195`. Implement this change only
  after that change merges.
