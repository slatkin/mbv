# Tasks

Work in a worktree (`wt resume-seam`). Run `cargo fmt` after each Rust change.
Use `cargo nextest run -p mbv-player` for tests. Follow `docs/standards/README.md`
(read the rules the change touches only).

## 1. Settle the facts the plan rests on

- [x] 1.1 Pin the Audiobookshelf jump facts with a `RunLifecycle`-level test (no mpv handle): `arm_jump` on an active-file lifecycle leaves the arm `Idle`, so `take_resume_ticks` returns `None`. Contract: an Audiobookshelf jump arms no resume seek, so `jump_ticks` returning `None` for `Session` changes nothing. Verify with `cargo nextest run -p mbv-player` on the named test. If a test for this already exists, add nothing.
- [x] 1.2 Re-check the stop-path facts against the code before building on them: `rg "report_stopped_background|transition_to_deferred" crates/mbv-player/src` shows callers only under `cmd_load_new`; `PlayerCommand::LoadNew` has no constructor outside tests. Record the result in the PR description. No code change.

## 2. Resume seam (no behavior change)

- [x] 2.1 Create `crates/mbv-player/src/resume.rs`. Move `resume_start_pos`, `resume_ticks_for_item`, `resume_ticks_for_slot`, `refresh_emby_resume`, and `EMBY_RESUME_RETRY_DELAYS` into it. Re-export the two public functions from `lib.rs`. Verify with `cargo check -p mbv-player -p mbv-daemon`.
- [x] 2.2 Add `ResumeSource` and `resume_source(&QueueItem)` with one explicit arm per kind (`Server`, `Session`, `Local`, `Beginning`) and no wildcard. Make `resume_start_pos` match on `ResumeSource`. Verify with the existing `resume_start_pos_*` tests in `tests/session/progress.rs`, unchanged and green.
- [x] 2.3 Add `ResumeSeam` with `refresh` (today's `refresh_emby_resume` body, keyed on `ResumeSource::Server`). Move the four callers to it: `load_queue_sources` in `submit.rs`, `cmd_submit_queue` in `run/commands/load.rs`, `append_items_via_mpv` and `cmd_jump_to` in `run/commands/queue.rs`. Verify with `cargo nextest run -p mbv-player` green and `rg refresh_emby_resume crates/` showing only `resume.rs` and its tests.
- [x] 2.4 Add `ResumeSeam::jump_ticks(item, owner_ticks)`. Replace the hand-written branch in `cmd_jump_to`. Make `PreparedSource::override_jump_resume` match on `resume_source` instead of `is_audiobookshelf`. `override_jump_resume` keeps its apply-or-zero semantics with `Session` as the pass-through arm; `jump_ticks` is for `cmd_jump_to` only, and `select_active_slot` still never fetches. Verify with `cargo nextest run -p mbv-player` green and no kind branch left in `cmd_jump_to` or `prepare_active_slot_with_resume` (read both). `cmd_append_queue`'s `is_audiobookshelf` promotion check is not a resume decision and stays.
- [x] 2.5 Re-point `tests/resume_refresh.rs` at the seam and add one table-free test per source. Contract: the source choice per kind at the seam, with the fetch mocked. Cases: Emby video fetches and the server value wins; Emby audio never fetches and starts at 0; Feed keeps its local position under the 1% rule and never fetches; Audiobookshelf never fetches and `jump_ticks` yields `None`. Verify with `cargo nextest run -p mbv-player resume`. Keep the retry and missing-id tests, no real sleeps (`NO_DELAYS`).
- [x] 2.6 Add the *Resume source* term to `CONTEXT.md` (with *Avoid*: progress source, position source). Verify with `qmd query "Resume source"` returning the entry.

## 3. Drain the run's reports before its thread ends

- [ ] 3.1 Add `ReportJob::Barrier(mpsc::Sender<()>)` and the worker arm in `report_worker.rs`. Add `SessionReporter::settle(bound)`: send the barrier, wait with `recv_timeout`, and log `player.resume.settle_timed_out` on timeout. Verify with two worker-level tests (no real sleeps, a gate held by the test): a held job ahead of the barrier keeps the ack back until the gate opens; a zero bound returns without the ack. Contract: the FIFO worker acks the barrier only after earlier jobs ran (#963).
- [ ] 3.2 Call `self.reporter.settle(REPORT_SETTLE_BOUND)` (2 seconds, named constant) in `run()` after the loop returns and the mpv child is dropped, before the panic teardown. Verify with `cargo check -p mbv-player` and by reading that `run_exited` is already stored before the call, so no submission reuses the dying run.
- [ ] 3.3 Check every caller of `Player::join` and `join_or_timeout` for the added wait (`rg "\.join\(\)|join_or_timeout" crates/mbv-player crates/mbv-daemon src`). Confirm the daemon callers pass a deadline. Verify by listing the callers and their bounds in the PR description. Fix any caller that waits without a bound, or put it in front of the user.

## 4. Docs and gates

- [x] 4.1 Update `docs/invariants/06-queue-progress-application-sites.md`: the seek source is chosen by `resume_source`; the per-kind list moves to the seam; state the run-exit drain as the ordering rule and name its test (task 3.1). Verify by reading the file against the code (`rg resume_source`).
- [ ] 4.2 Run the gates: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv-player -p mbv-daemon`, and `make check-code-file-lines` just before the push. Fix every red gate or put it in front of the user. No lint suppression.
- [ ] 4.3 Open the PR with the change left open. Ask the user for a live test of the stop-then-replay case, and for approval to sync and archive.

## Workflow follow-up

- On the user's merge approval only: sync the `playback-resume` delta into
  `openspec/specs/` and archive the change as the last commit before the merge.
