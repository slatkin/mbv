# Proposal

## Why

niri draws no focus ring on layer-shell surfaces, so the pinned panel gives no sign that it holds
the keyboard. The `pinwin` crate's focus accent — a stroke around the whole panel window, drawn
while the panel holds keyboard focus — is already in the pinned revision (`0d9be0b`, the Rust
crate mbv consumes): `pinwin::layout::Accent` and `Startup.accent: Option<Accent>`. mbv passes
`None`, so the panel still shows no sign of focus and the user cannot configure the accent. This
change adopts it.

## What Changes

- `src/pin.rs` builds `Startup.accent` (`Option<pinwin::layout::Accent>`, `None` means off) from
  the new `[panel]` keys at launch, instead of the hardcoded `None` it passes today.
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

- `pinned-launch`: `Panel layout settings` gains the three accent keys and F2 rows. New
  requirement `Focus accent applies at launch`.

## Impact

- Crates and files that change:
  - `crates/mbv-config`: `PanelConfig`, parse, save.
  - `crates/mbv-ui-model`: three setting keys and rows, `changed_panel_config`.
  - `src/pin.rs`: builds the accent from config at start.
  - `src/app/dispatch/settings.rs`: an accent edit is saved and toasted without a live apply.
  - Docs: `README.md`, `dist/config.toml`, `CONTEXT.md`.
- No protocol, daemon, pinwin source or pin changes: pin `0d9be0b` already ships the accent.
- `panel-expand-toggle` is already merged.
