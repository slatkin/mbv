# Tasks

## 1. Reuse decision in the Controller

- [x] 1.1 Add the `run_exited` flag (D7): a per-run `Arc<AtomicBool>` on `Player`, replaced at each cold start, passed into the run thread, and set in `PlaybackRun::run` when the loop returns, before `drop(mpv)`. Verify: `cargo check -p mbv-player`.
- [x] 1.2 In `Controller::submit_queue_slots` (`crates/mbv-player/src/submit.rs`), replace the `status.active` fast-path gate with the D1 order (run exists, class match, `stop_tx` is `Some`, `run_exited` unset, send), with the `stop_tx` lock held across the send. Seed the status after the send is accepted (D2). Fall through to the existing cold start on any failed check. Verify: the existing submit tests still pass, `cargo nextest run -p mbv-player`.
- [x] 1.3 Add the D5 structured events at the decision point: one when the live run takes the submission, one when a cold start happens with `reason` from `no_run`, `headless_mismatch`, `stop_requested`, `run_exited`, `send_failed`. Verify: the contract test in 2.1 asserts the reason of each case.

## 2. Contract test

- [x] 2.1 One `#[case]` table at the Controller level (`crates/mbv-player/src/tests/submit.rs`, a stubbed player with a spy or dropped command receiver; cite this change in the test name). Each case has a different expected outcome: idle run (`active=false`) reuses; parked run reuses; headless mismatch cold-starts; `stop_tx` taken cold-starts with `stop_requested`; `run_exited` set cold-starts; dropped receiver cold-starts with `send_failed`; no `cmd_tx` cold-starts with `no_run`. Verify: `cargo nextest run -p mbv-player`.
- No test for the loop-exit window (it needs a live mpv; AGENTS.md forbids it) and none for stop ordering inside `run_tick`: the change no longer depends on it.

## 3. Daemon helper rename

- [x] 3.1 Rename `submit_queue_slots_cold_start` to `submit_queue_slots_for_replace` (D6): the definition in `crates/mbv-daemon/src/core.rs`, the `lib.rs` re-export, `control.rs`, `reconciliation.rs`, `output/handoff.rs`, `core/transition_dispatch.rs`, and the doc comments in `core.rs` and `control_queue.rs` that call it a cold start. No behaviour change. Verify: `cargo check -p mbv-daemon` and `cargo nextest run -p mbv-daemon`.

## 4. Docs

- [x] 4.1 Update `docs/invariants/22-mpv-child-never-outlives-the-owner.md` (item 1: a live child may take a later submission of the same run; it never serves two live runs at once and never outlives the Owner) and `docs/invariants/24-run-ends-when-nothing-can-play.md` (the parked run now has a consumer; name the loop-exit residual window). Verify: read both files against D7.

## 5. Gates

- [ ] 5.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-player -p mbv-daemon`, and `make check-code-file-lines` just before push; fix or surface any red gate.

## Workflow follow-up

- Sync the delta into `openspec/specs/` and archive the change (`openspec archive submit-reuses-live-run`) only when the user approves the PR for merge, as the last commit before the merge. Never at PR open.
