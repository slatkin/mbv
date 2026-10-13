# Invariant 22 — The mpv child never outlives the Owner process and never serves two runs

## The invariant

1. One `mpv` child process serves exactly one Playback run. A later run never
   reuses a live child.
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
  sends a best-effort async `quit`, then kills and reaps the process.
- `start_process` (`mpv_session/spawn.rs`) sets `PR_SET_PDEATHSIG` to
  `SIGKILL` in `pre_exec`. The kernel sends `SIGKILL` to the child when the
  thread that started it ends. An Owner crash cannot leave an mpv child.
- One long-lived `Spawner` thread (`mpv_session/spawn.rs`), owned by the
  player controller (`controller.rs`), starts every child. `pre_warm` and the
  on-demand start in `run_player_thread` both use it. This thread outlives
  every run, so `PR_SET_PDEATHSIG` cannot fire early on a short-lived caller
  thread.
- `prepare_ipc_socket` (`runtime.rs`) removes a stale socket before each
  spawn. `submit.rs` takes the pre-warmed session once
  (`pre_warmed_mpv.take()`), so no child serves two runs.

## Where it currently fails

No known violation. Known limits:

- `PR_SET_PDEATHSIG` is Linux-specific. On another platform the child relies
  on `Drop` alone.
- `Child::drop` sends `quit` without waiting. It relies on its own kill and
  reap, so the exit is not graceful when the run's shutdown grace period has
  not already passed.
- The one-pre-warmed-instance rule is a convention in `submit.rs`, not a type.
  A later call site that clones a session or keeps the pre-warm would break
  it.
