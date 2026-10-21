# Tasks

## 1. Types first

- [x] 1.1 In `src/app/dispatch/action.rs`, rename the four sub-enums to `VisualIntent`,
      `PlaybackIntent`, `NavigationIntent`, `PanelIntent`; add `ShellCommand` and `AppCommand`;
      make `Command` the two-variant wrapper; add the `From` impls. Verify:
      `cargo check -p mbv` fails only at caller sites.
- [x] 1.2 Rewrite `App::dispatch` to take `&AppCommand` with six direct arms; delete both
      `unreachable!` arms and the translation. Verify: `dispatch` has no `_` arm and
      `ast-grep scan .` is clean.
- [x] 1.3 Rewrite `Model::dispatch_router_command` in `src/app/shell/arbitration.rs` as an
      exhaustive match over `Command` with no `_` arm. Verify: `cargo check -p mbv`.

## 2. Callers

- [x] 2.1 Update construction sites in `src/app/input/key_policy.rs` (`command_for_policy`),
      `router.rs`, `src/app/shell/playback.rs`, `queue.rs`, `mouse_gestures.rs`,
      `context_menu/actions.rs`, and `crates/mbv-components/src/status_bar_panel.rs`. Verify:
      `cargo check --workspace`.
- [x] 2.2 Update the existing tests that build or match `Command` (routing matrix, tick
      integration, `shell/tests.rs`, `dispatch/action/tests.rs`, `key_policy` tests). Add no
      new tests: the routing-matrix and `dispatch_router_command` tests already own the
      contract "each key resolves to the same effect" (`docs/invariants/14-test-ownership.md`).
      Verify: `cargo nextest run --workspace` passes with the same test count.

## 3. Docs and gates

- [ ] 3.1 Add the *UI command* term to `CONTEXT.md` and update the `Command` variant site in
      `docs/recipes.md`. Verify: `qmd query "UI command"` returns the term.
- [ ] 3.2 Update `openspec/changes/land-tier2-ast-grep-tripwires/` `design.md` decision 2 and
      `tasks.md` row 3.1 to point to this change. Verify: `openspec validate
      land-tier2-ast-grep-tripwires --strict` passes.
- [ ] 3.3 Run the gates: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D
      warnings`, `cargo nextest run --workspace`, `ast-grep scan .`, and the rca complexity
      check on all changed `.rs` files. Verify: every gate is green and no function is over
      cyclomatic 25.

## Workflow follow-up

- Sync and archive only when the user approves the PR for merge.
