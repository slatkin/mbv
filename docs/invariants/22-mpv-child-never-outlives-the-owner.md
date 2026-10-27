# Invariant 22 — The mpv child never outlives the Owner process and never serves two runs

## The invariant

1. One `mpv` child process serves exactly one Playback run at a time. A later
   submission of the same run may reuse the live child; a later run never
   does. Two live runs never share one child.
2. The child never outlives the Owner process. When the run ends, or when the
   Owner process dies, the child exits.

## Why it matters

An mpv child holds the audio device and its IPC socket. An orphan child can
keep the audio device busy after a crash, or answer a later run on a stale
socket. Nothing in the type system prevents this: `std::process::Child` has no
`Drop`, so a forgotten child either stays alive (mpv runs with `--idle=yes`)
or becomes a zombie. The property holds only because every start and every
stop goes through the code below.

## How the code maintains it today

- `Mpv` (`crates/mbv-player/src/mpv_session.rs`) owns the `Child` in its
  `_child` field. The session lives exactly as long as the run, so the child
  does too. `Child::drop` (`crates/mbv-player/src/mpv_session/spawn.rs`)
  kills and reaps the process.
- `start_process` (`mpv_session/spawn.rs`) sets `PR_SET_PDEATHSIG` to
  `SIGKILL` in `pre_exec`. The kernel sends `SIGKILL` to the child when the
  thread that started it ends. An Owner crash cannot leave an mpv child.
- One long-lived `Spawner` thread (`mpv_session/spawn.rs`), owned by the
  player controller (`controller.rs`), starts every child. `pre_warm` and the
  on-demand start in `run_player_thread` both use it. This thread outlives
  every run, so `PR_SET_PDEATHSIG` cannot fire early on a short-lived caller
  thread.
- A submission either goes into the live run or starts a new one, and the
  choice never doubles up the child. `Controller::submit_queue_slots`
  (`submit.rs`) reuses the live run through `submit_into_live_run` when a run
  exists, the headless class matches, no stop was requested (`stop_tx` is
  `Some`), and `run_exited` is unset; the run then takes the submission
  (`PlayerCommand::SubmitQueue`, `run/commands.rs`). The reported playback
  state does not decide this, so an idle or parked run keeps its child. Every
  failed check falls back to the cold start, which calls `stop()` and `join()`
  before it spawns, so the old child is gone before the new one starts.
- `prepare_ipc_socket` (`runtime.rs`) removes a stale socket before each
  spawn. `submit.rs` takes the pre-warmed session once
  (`pre_warmed_mpv.take()`), so no child serves two runs.

## Where it currently fails

No known violation. Known limits:

- `PR_SET_PDEATHSIG` is Linux-specific. On another platform the child relies
  on `Drop` alone.
- `Child::drop` kills and reaps without a `quit`. The exit is not graceful
  when the run's shutdown grace period has not already passed.
- The one-pre-warmed-instance rule is a convention in `submit.rs`, not a type.
  A later call site that clones a session or keeps the pre-warm would break
  it.
- The reuse decision is a check-then-send, not a handshake. `run_exited` (set
  in `PlaybackRun::run` before `drop(mpv)`) narrows the teardown window to one
  loop pass, but a submission accepted in the pass where the loop exits is
  lost (invariant 24 names that residual window). It never leaves a second
  child: the next submission reads `run_exited` set, cold-starts, and joins
  the exited run first.
