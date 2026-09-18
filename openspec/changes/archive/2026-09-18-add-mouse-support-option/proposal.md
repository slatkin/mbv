## Why

Mouse support (ADR 0024) is always on: `init_terminal` unconditionally enables
crossterm mouse capture, so there is no way to turn it off. Users on terminals
where captured mouse bytes interfere with native selection/scrollback (or who
simply do not use the pointer) have no escape hatch. The feature should be
gated behind a configuration option, default on.

## What Changes

- Add `mouse_support` bool setting (default `true`) under the existing
  `[display]` config section: parsed from `config.toml`, persisted on save.
- Gate `EnableMouseCapture` in `init_terminal` on the setting; when off, the
  terminal never sends mouse events and every downstream mouse layer
  (subscriptions, fold, hit resolution) idles untouched. `restore_terminal`
  keeps its unconditional `DisableMouseCapture`.
- Add a `MouseSupport` row to the F2 Settings panel's Display section. The
  toggle flips the config value and applies mouse capture immediately in the
  live session (no restart); persistence goes through the existing debounced
  settings save.
- Extract one shared `set_mouse_capture` helper used by `init_terminal`,
  `restore_terminal`, and the settings toggle, so capture sequences exist in
  exactly one place.
- Add a requirement to the `mouse-input` spec covering the toggle's observable
  behavior.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `mouse-input`: new requirement — mouse capture follows the `mouse_support`
  setting (default on); off means capture is never enabled and no mouse events
  are delivered; the F2 toggle applies immediately and persists.

## Impact

- `crates/mbv-core/src`: `config_types_paths.rs` (field + default),
  `config_parse.rs` (parse under `[display]`), `config_save.rs` (save entry).
- `src/app/mod.rs`: `init_terminal` signature (takes the flag),
  `set_mouse_capture` helper.
- `src/app/render/components/settings.rs` + `src/app/types_settings.rs`:
  `SettingKey::MouseSupport`, Display-section row, toggle arm applying live
  capture via the helper.
- Tests: config parse/save round-trip; settings toggle arm. Existing
  `tests_tick_integration_mouse*.rs` suites are unaffected (they inject
  events through the tick harness, never through a real terminal).
- Known cosmetic edge, accepted: a target mid-hover when mouse is toggled off
  keeps its hover appearance until the next repaint; no event arrives to
  clear it.
