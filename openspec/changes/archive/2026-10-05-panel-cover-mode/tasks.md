# Tasks

## 1. Config

- [x] 1.1 In `crates/mbv-config/src/panel.rs`, add `cover: bool` to `PanelConfig` (last field,
      default `false`), documented as pinwin's covering mode. In `crates/mbv-config/src/parse.rs`,
      parse it with `panel_bool(panel, "cover", defaults.cover)`. In
      `crates/mbv-config/src/save.rs`, write it as a boolean after `accent_width`. Extend
      `panel_values_fall_back_per_key_and_a_valid_section_round_trips`
      (`crates/mbv-config/src/tests/settings.rs`) with a malformed `cover` that falls back to
      `false` and a `cover: true` round trip. Verify `cargo nextest run -p mbv-config`.

## 2. Pin wiring

- [x] 2.1 In `src/pin.rs::layout_from_config`, apply `.covering()` on the built `Layout` when
      `config.cover` is set. Extend the layout test with a `#[case]` table asserting
      `Layout::coverage()` follows `cover` (default pushes, `cover = true` covers). Verify
      `cargo check -p mbv`.

## 3. Docs

- [x] 3.1 Add `cover` to the `[panel]` section in `dist/config.toml` with its default and a
      one-line effect note. In `CONTEXT.md`, add the term **Panel covering** after **Focus
      accent**. _Avoid_: cover mode, overlay, always on top. Verify by reading.

## 4. Integration

- [x] 4.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D
      warnings` and `cargo nextest run -p mbv-config -p mbv`. Integration check (manual, user):
      with `cover = true`, `mbv --pin` shows the panel over tiled windows and niri reserves no
      strip; with the key removed, the panel pushes again after a relaunch.
