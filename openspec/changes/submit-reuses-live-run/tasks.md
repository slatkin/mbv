# Tasks

## 1. Reuse decision in the Controller

- [ ] 1.1 In `Controller::submit_queue_slots` (`crates/mbv-player/src/submit.rs`), replace the `status.active` fast-path gate with the D1 order: send `PlayerCommand::SubmitQueue` into the live run when `current_is_headless` matches, seed the status after the send is accepted (D2), and fall through to the existing cold start when the send fails. Verify with the existing submit tests still passing: `cargo nextest run -p mbv-player`.

- [ ] 1.2 Add the D5 structured events at the decision point: one event when the live run takes the submission, one when a cold start happens with `reason` (`no_run`, `headless_mismatch`, `send_failed`). Verify by running the owner-level test from 2.1 with logging enabled and reading the emitted lines.

## 2. Contract tests

- [ ] 2.1 Test that a submission into a live run that reports no active playback enters that run instead of tearing it down (idle and parked variants; cite this change in the test name). Verify: `cargo nextest run -p mbv-player`.
- [ ] 2.2 Test that a submission whose headless class differs from the running run's still cold-starts, and that a failed send (dropped receiver) falls back to a cold start (cite this change). Verify: `cargo nextest run -p mbv-player`.
- [ ] 2.3 Test that a submission racing a pending stop plays exactly once: cancelled stop keeps the run, applied stop ends it and the fallback cold-starts (extends the existing `cancel_pending_quit_clears_quit_at_and_shutdown_timeout` contract). Verify: `cargo nextest run -p mbv-player`.

## 3. Daemon helper rename

- [ ] 3.1 Rename `submit_queue_slots_cold_start` to `submit_queue_slots_for_replace` in `crates/mbv-daemon/src/core.rs` and its call sites; no behaviour change. Verify: `cargo check -p mbv-daemon` and `cargo nextest run -p mbv-daemon`.

## 4. Gates

- [ ] 4.1 Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run -p mbv-player -p mbv-daemon`; fix or surface any red gate before push.

## Workflow follow-up

- Archive the change after the project's review requirements are satisfied (`openspec archive submit-reuses-live-run`).
