## 1. Config and settings model

- [ ] 1.1 `crates/mbv-config`: add `cols_expanded: u16` to `PanelConfig` with `DEFAULT_PANEL_COLS_EXPANDED = 80`; parse it like `cols` (same range, same per-key fallback warning) and save it next to `cols`. Extend `panel_values_fall_back_per_key_and_a_valid_section_round_trips` (`crates/mbv-config/src/tests/settings.rs`) with a malformed and a valid `cols_expanded`; verify `cargo nextest run -p mbv-config`.
- [ ] 1.2 `crates/mbv-ui-model/src/settings.rs`: add `SettingKey::PanelColsExpanded`, an `Expanded cols` row after `Cols` on the Panel page (and in `panel_cursor_to_key`), and its step arm in `changed_panel_config` (same clamp as `PanelCols`). Fix every exhaustive match the compiler flags (`crates/mbv-components`, `crates/mbv-render`, `src/app`) with the same handling as `PanelCols`; verify `cargo check --workspace`.

## 2. Width-aware pinned layout

- [ ] 2.1 `src/pin.rs`: add `pub(crate) enum PinnedWidth { Collapsed, Expanded }` (`Default` = `Collapsed`, plus a `toggled()` method); `layout_from_config(config, width)` selects `cols` or `cols_expanded`; `start` passes `Collapsed`; `apply_layout(panel, config, width)`. Verify `cargo check -p mbv`.
- [ ] 2.2 `App` (`src/app/state/app_struct.rs`, `construct.rs`): add `pinned_width: PinnedWidth` next to `pinned_panel`. In `apply_panel_setting` (`src/app/dispatch/settings.rs`) apply at `Collapsed` for `PanelCols`, `Expanded` for `PanelColsExpanded`, otherwise the current `pinned_width`; on `Ok` also store that width (design D4). Verify `cargo check -p mbv`.

## 3. Toggle keybind

- [ ] 3.1 `crates/mbv-keybinds/src/registry.rs`: add `pinned_width_toggle` (section `Global`, default `Ctrl+e`, gate `NoBlockingOverlay`, policy `pinned_width_toggle`, rebindable, prefix-addressable). `src/app/input/key_policy.rs`: matching `KEY_POLICY` entry (global, `NoBlockingOverlay`), `KeyPolicyBinding::PinnedWidthToggle` → `Command::TogglePinnedWidth`. Verify existing registry/policy consistency tests pass: `cargo nextest run -p mbv-keybinds` and `cargo nextest run -p mbv -E 'test(routing_matrix)'`.
- [ ] 3.2 `src/app/dispatch/action.rs`: dispatch `Command::TogglePinnedWidth` to an `App` method: no `pinned_panel` → `Neutral` toast "Width toggle needs a pinned launch (mbv --pin)"; otherwise `pin::apply_layout` at `pinned_width.toggled()`, `Ok` → store it, `Err(reason)` → `Warning` toast with `reason`, width unchanged. Add no test: the pinwin handle cannot be mocked and the not-pinned branch is a one-line toast. Verify `cargo clippy --workspace --all-targets -- -D warnings`.

## 4. Docs and specs

- [ ] 4.1 `CONTEXT.md`: add **Pinned panel width** after **Pinned panel** (collapsed = `cols`, expanded = `cols_expanded`, toggled by `pinned_width_toggle`, never saved; _Avoid_: panel mode, expand mode, panel size). Verify by reading the entry.
- [ ] 4.2 Integration check (manual, user): `mbv --pin` on niri — `Ctrl+e` expands and tiles reflow, again collapses; stepping `Expanded cols` in F2 while collapsed expands the panel; relaunch opens collapsed; `Ctrl+e` in a plain terminal shows the toast. `cargo fmt --all -- --check` and `cargo nextest run --workspace` pass.
