# Tasks

## 1. Remove the `cover` setting from config

- [ ] 1.1 Delete the `cover` field and its default from `PanelConfig` (`crates/mbv-config/src/panel.rs`), its parse line (`parse.rs`, the `cover: panel_bool(...)` entry) and its save line (`save.rs`, the `panel.insert("cover", ...)` entry). Verify: `cargo check -p mbv-config` reports errors only in `tests/settings.rs`.
- [ ] 1.2 In `crates/mbv-config/src/tests/settings.rs`, delete the `cover = "yes"` input line, the `assert!(!cfg.panel.cover)` line and the `cover: true` struct field. Do not add a test for the ignored key. Verify: `cargo nextest run -p mbv-config` passes.
- [ ] 1.3 Remove the `cover` entry and its comment block from `dist/config.toml` (the "Covering:" comment and `cover = false`). Verify: `rg -n "cover" dist/config.toml` prints nothing.

## 2. Make the width decide push or cover

- [ ] 2.1 In `layout_from_config` (`src/pin.rs`), return the plain layout for `PinnedWidth::Collapsed` and `layout.covering()` for `PinnedWidth::Expanded`, replacing the `config.cover` branch. Update the comment above it. Verify: `cargo check -p mbv` passes.
- [ ] 2.2 Replace the test `layout_from_config_follows_the_cover_setting` (in `src/pin.rs`) with one `#[case]` table over the two widths. It asserts `Collapsed` gives `Coverage::Push` and `Expanded` gives `Coverage::Cover`, and it cites issue #898. Verify: `cargo nextest run -p mbv layout_from_config` passes.

## 3. Docs and spec vocabulary

- [ ] 3.1 Rewrite the *Panel covering* entry in `CONTEXT.md` to say the collapsed panel pushes and the expanded panel covers, with no setting. Keep the `_Avoid_` line. Verify: `rg -n "\`cover\`" CONTEXT.md` prints nothing.
- [ ] 3.2 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings` and `openspec validate pinned-expand-covers-collapsed-pushes`. Verify: all three exit 0.

## Workflow follow-up

- Live check by the user: expand and collapse the pinned panel on the left and confirm tiled windows do not move.
- Sync the delta spec into `openspec/specs/pinned-launch/` and archive the change.
