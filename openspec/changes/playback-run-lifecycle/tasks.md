# Tasks

All work is in `crates/mbv-player`. For each task, run `cargo fmt`, then `cargo nextest run -p mbv-player`, then `cargo clippy -p mbv-player --all-targets -- -D warnings`. A task is done only when all three pass. Groups 1-2 move state with no behavior change. Groups 3-5 fix one defect group each (design "Context", defects 1-6).

## 1. The lifecycle module, with no behavior change

- [x] 1.1 Add `run/lifecycle.rs` with `RunLifecycle`, `Projection`, `JumpArm` (`Idle`, `Pending(ForcedJump)`, `ResumeOnly { slot_id, resume_ticks }`), and the `began` and `ended` state (design D1). Add only the constructors and the read accessors. Verify: `cargo check -p mbv-player`. (JumpArm lands in 1.3, `ended` in 2.1: dead_code gate.)
- [x] 1.2 Replace `PlaybackRun::playback_began` and `active_file` with the lifecycle (`playback_started()`, `enter_active_file()`, `is_active_file()`, `began()`) at every site that design "Context" lists. Change the direct field writes in `src/tests/submit.rs` and `src/tests/active_file.rs` to the facts. Verify: the existing tests pass unchanged in what they assert.
- [ ] 1.3 Replace `PlaybackRun::forced_jump` with `JumpArm`. Port `set_pending_playlist_jump`, `cmd_jump_to_playlist`, `settle_forced_jump`, `settle_idle_jump_on_restart`, `apply_forced_resume`, `adopt_mpv_entry`, the `queue_commands.rs:25` clear, `begin_item_lifecycle`, `is_superseded_jump_end_file`, and the drift mute. Each one does the same transition as today. Change the `forced_jump` writes in `src/tests/session.rs` to `arm_jump`. Verify: the existing tests pass, and `rg -n forced_jump crates/mbv-player/src` shows no field access.

## 2. The single exit

- [ ] 2.1 Add `StopOutcome` and `PlaybackRun::end_run` (design D4), backed by `RunLifecycle::close(identity)`. Route the nine in-run `Stopped` sends through it (`run_loop.rs` `finalize_quit`, `queue_advance.rs` ×3, `termination.rs` ×4, `commands/load.rs` `accept_stopped_replacement`). Remove `stopped_event_sent`, and remove the `stop_report_sent()` term from the guard in `stop_run_with_error`. Call `submission_accepted()` where `run_identity` is refreshed (`run/commands.rs` `SubmitQueue`), in the active-file replacement path, and in `cmd_load_new`. Verify: the existing tests pass, and `rg -n "PlayerEvent::Stopped" crates/mbv-player/src/run` lists only `end_run` and the panic teardown.
- [ ] 2.2 Add `run/lifecycle/tests.rs` with the unit test for the contract "one close per run identity": a second `close` returns false, and `close` after `submission_accepted` returns true. Verify: the test passes.
- [ ] 2.3 Add one run-level regression test in `src/tests/session.rs` for defect 2: a failed active-file replacement, then a quit, sends exactly one `Stopped` on the channel. Verify: the test fails on the parent commit of 2.1 and passes after it. If `session.rs` would grow past 800 lines, put the test in a new `src/tests/session/` submodule.

## 3. The load-failure verdict

- [ ] 3.1 Add `RunLifecycle::load_failed(RunPosition) -> FailureVerdict` with the D2 rule order, and make `stop_for_unrecoverable_error` act on the verdict. A stop names `failed_slot`. Verify: the existing tests for #941, #942, and #943 pass.
- [ ] 3.2 In `run/lifecycle/tests.rs`, add the named `#[case]` table for the contract "the load-failure verdict" (spec requirements 1-3). Keep only cases with different outcomes: before start → end; after start with a next entry → continue; after start with no next entry → end; active-file → end; pending from-idle target that is last → end naming the target (defect 4); pending from-idle target with a next entry → continue and arm idle; resume-only arm on a target with a next entry → continue and arm idle (defect 5). Delete the `tests/submit.rs` tests that set `playback_began` or `active_file` directly where the table now covers them. Verify: the table passes, and each regression case cites its issue or defect in its case name.
- [ ] 3.3 Make `submission_accepted()` reset `began` (design D5). Add one `#[case]` row to the 3.2 table: began in an earlier submission, then a new submission fails before start → end. Verify: the table passes.

## 4. Active-file failure arms

- [ ] 4.1 In `cmd_jump_to_active_file` (selection error) and in `promote_to_active_file` (install error), call `end_run` with an error that names the slot the run tried to select (defect 3, spec requirement 4). Verify: one run-level test in `src/tests/active_file.rs` drives a failed active-file jump and asserts one `Stopped` with `error: Some(_)` and the target slot.

## 5. Docs and gates

- [ ] 5.1 Rewrite `docs/invariants/24-run-ends-when-nothing-can-play.md` to the new mechanism: the lifecycle verdict, `end_run`, and the per-identity close. Close the "Where it currently fails" entry, and record the D3 two-jump window as the remaining known limit. Verify: every function and file that the invariant names exists (`rg`).
- [ ] 5.2 Run the full gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run --workspace`. Then run `make check-code-file-lines` just before the push. Verify: all are green.

## Workflow follow-up

- Live test (user only): play a queue whose last entry is a broken URL, and jump to it. Expect one stop with an error, and a now-playing panel that is not frozen.
- Sync the delta into `openspec/specs/` and archive only when the user approves the PR for merge.
