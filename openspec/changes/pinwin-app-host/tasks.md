# Tasks

## 0. Gate

- [x] 0.1 Confirm that pinwin 0.4.1 with slatkin/pinwin#55 is released: `pinwin run --help` lists `--show-existing`, `pinwin set-size --help` lists `--reserve`, `pinwin run --help` lists the `--gutter-*` options, `pinwin set-gutters` exists, and a `pinwin run` app has `PINWIN_PANEL` and `PINWIN_SOCKET` in its environment. Do not start group 2 before this holds.

## 1. `[panel]` settings (design D5)

- [x] 1.1 In `crates/mbv-config/src/panel.rs`, replace `cols` / `cols_expanded` with `width` / `width_expanded` (logical pixels, 1..=65535, defaults 400 and 1100), and add `pin_terminal: Option<Vec<String>>`. The four `gutter_*` fields stay. Update `parse.rs` and `save.rs` to match: `save` writes `[panel]` in full with the new keys only. Verify: `cargo check -p mbv-config`.
- [x] 1.2 In `parse.rs`, ignore `cols` and `cols_expanded` with one logged warning that names the keys present; keep the silent ignore of `cover`. Extend the existing `[panel]` range contract test in `crates/mbv-config/src/tests/settings.rs` (task 4.2 of `pin-mbv-in-pinwin`) with the new width bounds, and add one test for the "Old column keys" scenario. Verify: `cargo nextest run -p mbv-config`.
- [x] 1.3 In `crates/mbv-ui-model/src/settings.rs`, rename the width `SettingKey`s and `PanelEditedWidth` mapping to `Width` / `Expanded width`, and step the width rows by 10 (100 with Shift). The gutter rows stay unchanged. Update the Panel page rows in `crates/mbv-components/src/settings.rs` and `crates/mbv-render/src/components/settings_component.rs`. Verify: `cargo nextest run -p mbv-ui-model -p mbv-components -p mbv-render`.
- [x] 1.4 Update `CONTEXT.md`: "Pinned panel width" (pixel keys, `x` only) and add "Pin terminal". "Gutter" keeps its meaning. Verify: `qmd query "pinned panel width"` returns the new text.

## 2. pinwin boundary in `src/pin.rs` (design D3, D4)

- [x] 2.1 Add the panel identity claim: read `PINWIN_PANEL` and `PINWIN_SOCKET` into a `OnceLock` and remove both from the environment in the same startup step as `claim_swap_token`. `is_pinned()` reads the claimed panel name (empty means unpinned). Remove `HANDED_OVER`. Unit-test the pure value-to-identity mapping (set, empty, unset). Verify: `cargo nextest run -p mbv pin::`.
- [x] 2.2 Add pure argv builders: `run_argv` (`run --name mbv --show-existing --edge --size <width>px --reserve push --gutter-top … --gutter-bottom … --gutter-left … --gutter-right … --focus-ring … -- <terminal> <exe> [--log-level …]`), `set_size_argv` (`set-size <panel> <N>px --reserve push|cover`), `set_edge_argv`, `set_gutters_argv` (one edited gutter), and `toggle_argv`, plus the terminal resolution (`pin_terminal`, else `TERMINAL -e`, else an error naming both). One `#[case]` table per builder, with only cases that have different outputs. Verify: `cargo nextest run -p mbv pin::`.
- [x] 2.3 Add the request runner: run `pinwin` with the argv and the claimed `PINWIN_SOCKET`, and map the outcome to `Result<(), String>` (exit 0 ok; exit 1 → pinwin's trimmed stderr; exit 2 → a usage-mismatch reason; spawn error → "pinwin is not installed"). Unit-test the pure outcome mapping only; the process call is the boundary. Verify: `cargo nextest run -p mbv pin::`.
- [x] 2.4 Delete the 0.3.4 code: `open_pty`, `detach_controlling_terminal`, `hand_over_stdio`, `redirect_stderr`, `PanelEnv`, the instance-socket code, `layout_from_config`, `PinnedPanel`, `PinLaunch`, the old `PinStartError` variants, and their tests. Verify: `cargo check -p mbv` after group 3 and 4 callers move (do 2.4 last in this group or together with them).
- [x] 2.5 Write ADR `docs/adr/0033-pinwin-is-an-external-app-host.md` (D1 to D4: terminal host, spawn-and-wait launcher, identity from `PINWIN_PANEL`, CLI transport). Verify: `qmd query "pinwin app host"` finds it.

## 3. Launcher, `--toggle`, and dependency (design D1, D2, D8, D9)

- [x] 3.1 Move `launch_exe` from `src/local_daemon.rs` to a shared place, and use it in the `--pin` launcher with the launcher's own PID. Keep the existing `launch_exe` test. Verify: `cargo nextest run -p mbv launch_exe`.
- [x] 3.2 Rewrite the `--pin` path in `src/main.rs`: pre-checks (`WAYLAND_DISPLAY`, terminal resolution) take today's start-failure path; then spawn `pinwin run` (2.2 argv) with `MBV_SWAP_TOKEN` set from the claimed token, wait, log a non-zero status, notify when stdin is not a terminal, and exit with the child's status. A spawn failure (no `pinwin`) takes the pre-check failure path. Remove `PinnedPanel` from `run_tui` and the startup plumbing. Unit-test the pure "status → report action" decision with the existing `failure_action` style. Verify: `cargo nextest run -p mbv pin::` and `cargo check -p mbv`.
- [x] 3.3 Rewrite `toggle_running` to run `pinwin toggle mbv` with the inherited environment and report a failure with pinwin's reason through `report_toggle_failure`. Verify: `cargo check -p mbv`.
- [x] 3.4 Update the `--pin` and `--toggle` help lines in `src/main.rs` and the `mbv-ctrl` comment at `crates/mbv-ctrl/src/protocol.rs:79`. Verify: `cargo run -- --help` shows the new text.
- [x] 3.5 Remove the `pinwin` dependency from `Cargo.toml` and refresh `Cargo.lock`; add `pinwin>=0.4.1` to `optdepends` in `PKGBUILD` and `PKGBUILD-git`. Verify: `cargo tree -i pinwin` reports no match, and `cargo check --workspace` passes.
- [x] 3.6 Update the pinwin entry in `AGENTS.md` (external program, minimum 0.4.1, `src/pin.rs` is the only caller) and the "Pinned panel" and "Pinned launch" terms in `CONTEXT.md`. Verify: `rg -n "git-rev dependency" AGENTS.md` finds nothing.

## 4. `x`, F2 rows, and width-toggle removal (design D6, D7)

- [x] 4.1 Replace `App.pinned_panel: Option<PinnedPanel>` with the pinned check from 2.1 at every reader (`action.rs`, `settings.rs`, `panel_focus.rs`, `construct.rs`, `app_struct.rs`) and fix the comments that name `pinwin::Panel`. Verify: `cargo check -p mbv`.
- [x] 4.2 Point `pinned_view_toggle` at the 2.3 runner with `set_size_argv` (`width_expanded` + cover to expand, `width` + push to collapse); keep `apply_pinned_view` as the success and failure handler. Keep `note_pinned_width_applied` as the width record only; remove the same-iteration pty-size adoption and `pinned_resize_pending` if nothing else reads them. The existing `src/app/tests/pinned_view_toggle.rs` tests own the `panel-mode` contract; adjust them to the new names. Verify: `cargo nextest run -p mbv pinned_view_toggle` and the invariant 17 tests pass.
- [x] 4.3 Rewrite the pinned arm of `apply_panel_setting` in `src/app/dispatch/settings.rs` (D7): width rows send the collapsed or expanded request with the candidate, `Side` sends `set-edge`, a gutter row sends `set-gutters` with the edited gutter, only an accepted request saves, and accent rows keep the next-launch toast. Verify: `cargo nextest run -p mbv settings`.
- [x] 4.4 Remove `pinned_width_toggle`: the registry entry in `crates/mbv-keybinds/src/registry.rs`, the policy in `src/app/input/key_policy.rs`, `Command::TogglePinnedWidth`, `toggle_pinned_width`, the "needs a pinned launch" toast, and their tests (`crates/mbv-keybinds/src/tests.rs`, `key_policy/resolution_tests.rs`). Verify: `cargo nextest run -p mbv-keybinds -p mbv`.
- [x] 4.5 Update the pinwin tween wording in `docs/invariants/17-resize-neither-blanks-nor-wipes-art.md` and `20-resize-drag-never-reencodes-images-inline.md` to the 0.4 model (one new size per change, delivered by the terminal). Verify: `rg -n "apply_layout_animated|pty" docs/invariants/17-*.md docs/invariants/20-*.md` finds nothing stale.

## 5. Integration checks

- [x] 5.1 Run `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo nextest run --workspace`; all pass.
- [x] 5.2 Run `make check-code-file-lines` just before the push; no file is over 800 lines.
- [ ] 5.3 Hand the user a live checklist (the user runs it): desktop-entry launch; `x` expand and collapse with no tiled window moving; `mbv --toggle` from a compositor key; a second `mbv --pin` shows the panel; Tray Pin and Unpin; F2 width and side while pinned; a launch on a session without layer-shell notifies; the Owner process environment has no `PINWIN_*`. Record the result in the PR.

## Workflow follow-up

- Sync the deltas into `openspec/specs/` and archive the change only when the user approves the PR for merge, as the last commit before the merge.
