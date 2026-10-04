# Tasks

## Prerequisites

`panel-expand-toggle` is merged to `main` (pinwin pin at `c64af49`, whose `PinwinStartup`
already carries the accent and whose strip is already drawn when enabled).

## 1. Accent at the crate boundary

- [ ] 1.1 In `crates/mbv-pinwin/src/ffi.rs`, add `const _: () = assert!(size_of::<PinwinStartup>() == 44);` (design D2). The `PinwinAccent` mirror and the `accent` field landed with `panel-expand-toggle`, so no pin bump is needed. Verify `cargo check -p mbv-pinwin`.
- [ ] 1.2 In `crates/mbv-pinwin`, add public `Rgb { r, g, b }` and `Accent { color: Rgb, width: NonZeroU16 }` beside `Layout` (`layout.rs`), with a `to_abi(Option<Accent>) -> ffi::PinwinAccent` that maps `None` to `{enabled 0, width 1}` (design D1). `Panel::start` takes a fourth parameter `accent: Option<Accent>`. Fix the call in `src/pin.rs::start_panel` by passing `None` for now (2.2 wires config). Verify `cargo check -p mbv-pinwin -p mbv`.

## 2. Config

- [ ] 2.1 In `crates/mbv-config/src/panel.rs`, add `PanelAccentColor([u8; 3])` with `parse(&str) -> Option<Self>` (accepts `#RRGGBB` or `RRGGBB`), `Display` as `#rrggbb`, and `DEFAULT_PANEL_ACCENT_COLOR = #dabc7f`. Add `accent: bool` (default `true`), `accent_color: PanelAccentColor` and `accent_width: NonZeroU16` (default 1) to `PanelConfig`. Parse them in `parse.rs` with the existing per-key fallback warning, and save them in `save.rs` after the gutters. Extend `panel_values_fall_back_per_key_and_a_valid_section_round_trips` (`crates/mbv-config/src/tests/settings.rs`) with a malformed colour, width 0, and a valid round trip of all three keys. Verify `cargo nextest run -p mbv-config`.
- [ ] 2.2 In `src/pin.rs`, build `Option<Accent>` from the three config fields (`None` when `accent` is false) and pass it from `start_panel`. Verify `cargo check -p mbv`.

## 3. F2 rows

- [ ] 3.1 In `crates/mbv-ui-model/src/settings.rs`, add `SettingKey::PanelAccent` (Boolean), `PanelAccentColor` (Text) and `PanelAccentWidth` (Stepper, 1..=65535). Add the rows `Accent`, `Accent color` and `Accent width` after `Gutter right` on the Panel page, including `panel_cursor_to_key`. Add arms in `changed_panel_config`: the colour cycles through `#dabc7f, #7fc8da, #a3be8c, #bf616a, #ffffff`, with the configured value first when it is not in that list (design D4). Fix every exhaustive match the compiler flags. Add one `#[case]` test owning the cycle contract: default → next list entry, and a custom colour stays reachable. Verify `cargo nextest run -p mbv-ui-model` and `cargo check --workspace`.
- [ ] 3.2 In `src/app/dispatch/settings.rs`, `apply_panel_setting` must save accent keys without calling `pin::apply_layout`. While `pinned_panel` is `Some`, show a `Neutral` toast "Accent changes apply on the next mbv --pin launch". Verify `cargo clippy --workspace --all-targets -- -D warnings`.

## 4. Docs

- [ ] 4.1 Add the three keys to the `[panel]` table in `README.md` and to `[panel]` in `dist/config.toml`, with their defaults. In `CONTEXT.md`, add **Focus accent** after **Pinned panel width**: a strip on the pinned panel's workspace-facing edge while it holds keyboard focus, fixed at launch. _Avoid_: focus ring, border, highlight. Verify by reading.

## 5. Integration

- [ ] 5.1 Integration check (manual, user): `mbv --pin` on niri. Clicking into the panel shows the `#dabc7f` strip on the workspace-facing edge, and clicking a tile removes it. `accent = false` shows no strip after a relaunch. Editing an accent row in F2 while pinned shows the toast, and the new value applies after a relaunch. Run `cargo fmt --all -- --check` and `cargo nextest run --workspace`; both must pass.
