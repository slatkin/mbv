## 1. Default chord

- [x] 1.1 In `crates/mbv-keybinds/src/registry.rs`, change the `stop` default chord from `Esc` to `Shift+S`, and reword the `prefix_addressable` comment per design D6. In `crates/mbv-keybinds/src/tests.rs`, change the default-chord table row for `stop` and swap the `("stop", "Esc")` entry in the `override_count` test for `Shift+S`, so it stays a redundant default-equal override. Verify: `cargo nextest run -p mbv-keybinds` passes.
- [x] 1.2 In `src/app/shell/settings.rs`, change the expected default text for the `stop` row from `Esc` to `Shift+S`. Verify: that test passes under `cargo nextest run -p mbv`.

## 2. Playback panel

- [x] 2.1 In `crates/mbv-components/src/library_playback_panel.rs`, remove the `Key::Esc` arm in `key()` and the `Key::Esc` term in the `Consumed(None)` list of `key_result` (design D5). Change the existing `Esc` assertion in its test to expect an unhandled key. Verify: `cargo nextest run -p mbv-components` passes.

## 3. Remove the double-Esc window

- [ ] 3.1 Delete `last_esc` and `DOUBLE_ESC_STOP_WINDOW` from `src/app/shell.rs`, the reset line in `src/app/shell/ui_state_reset.rs`, and in `Model::router_outcome` (`src/app/shell/arbitration.rs`) the `now` and `double_esc` locals, the `leaf_stops` scan, and the `match` that rewrites `Deferred(Stop)` (design D3). Remove imports this leaves unused. Reword the `apply_deferred_candidate` doc comment per design D4 and keep its `debug_assert!`. Verify: `cargo check -p mbv` passes.
- [ ] 3.2 Delete the three double-Esc tests in `src/app/shell/tests.rs` and `live_tick_escape_stops_on_the_second_press` in `src/app/tests/tick_integration.rs`, and the doc comments that point at them. Verify: `cargo nextest run -p mbv` passes with no reference to `last_esc` left (`cargo check` is the check, not a grep script).

## 4. Router tests

- [ ] 4.1 In `src/app/tests/routing_matrix/playback.rs`, change `playback_gating_esc_claims_deferred_stop` to the default-binding outcome (`Esc` is `FallThrough`), and add rows for the default Stop chord: `Command(Stop)` with the gate open, and `FallThrough` while text entry is focused. Add one row with a keybinds fixture that assigns `Esc` to `stop`, expecting `Deferred(Stop)`. Name each contract in the test name. Verify: `cargo nextest run -p mbv` passes, and each row has a different expected outcome.

## 5. Docs

- [ ] 5.1 Edit the ADR 0023 Amendment (`docs/adr/0023-one-central-keyboard-router.md`) and the `CONTEXT.md` *Arbitration fold* entry per design D7. Verify: the edited text no longer mentions the 300 ms double-tap or candidate clocks, and `CONTEXT.md` adds no new term.

## 6. Gates

- [ ] 6.1 Run `cargo fmt`, then `cargo clippy --workspace --all-targets -- -D warnings`. Verify: both pass with no new `allow` attribute.
- [ ] 6.2 Run `make check-code-file-lines` before the push. Verify: no touched file is over 800 lines.

## Workflow follow-up

- Reduce `keyboard-resolution-seam` with `openspec-update-change` once this change is accepted: drop its Escape-clock work (design D2, D6, D7, tasks 2.x and 3.1, and its `semantic-input-arbitration` delta). Do not archive both with overlapping deltas.
- After PR merge approval, sync the deltas into `openspec/specs/` and archive the change as the last commit before the merge. Do not sync or archive when the PR opens.
