# Design

## Context

See `proposal.md` (Why) and the Findings of `openspec/changes/mpv-ipc-event-probe/design.md` (evidence).

Today `init_mpv` (`crates/mbv-player/src/runtime.rs`) creates one `libmpv2::Mpv` for each playback run, or takes one that `Player::pre_warm` made earlier for PCM-pipe output. One thread (`run_player_thread`) owns the handle. Commands reach that thread through a channel, and a self-pipe wakes it. libmpv's wakeup callback writes to the pipe when mpv has an event. The run loop then calls `wait_event(0.0)` until it returns `None`.

The libmpv type shows in about 2,800 lines across `lib.rs`, `runtime.rs`, `controller.rs`, `submit.rs`, `tracks.rs`, `run/*`, and `error.rs`. Raw FFI shows in two places: `mpv_command_async` for an async quit, and `mpv_request_log_messages`.

mbv already runs mpv with `input-ipc-server=<mbv_ipc_path()>`. So user tools can talk to the socket that this change makes mbv's own control channel.

## Goals / Non-Goals

**Goals:**
- Replace libmpv with a child process, with the behavior in `specs/mpv-process-isolation/spec.md`.
- Keep every step compile-clean and test-green. The libmpv-to-IPC swap itself is one local edit.

**Non-Goals:**
- A runtime switch between the two backends.
- Batching `track-list` reads, or any other speed work, before a measurement asks for it.
- Capture of the child's stderr. Log messages travel on the socket, as today.

## Decisions

### D1. Land the change in three layers, so the swap is small

1. A domain `MpvError` replaces `libmpv2::Error` in `PlayerError` and in `mpv_err_str` (still on libmpv).
2. A crate-private struct `Mpv` (new file `mpv_session.rs`) wraps the libmpv2 handle and offers only the calls the player uses. All call sites move to it (still on libmpv). It is a struct, not a trait: one backend exists at any time.
3. The inside of that struct changes from libmpv2 to the child process.

Rejected: a trait with two implementations (nothing needs both at run time, and the probe example already compares them), and a big-bang rewrite (no green step in the middle).

### D2. The wrapper API uses `PropertyValue`, not generics

libmpv2 uses `get_property::<T>` and `set_property<T>` with its own traits. The wrapper offers:

- `set_property(name, impl Into<PropertyValue>)`, with `From` for `i64`, `bool`, `f64`, `&str`, `String`. The existing `PropertyValue` (`mpv_event.rs`) is the value type for set and for observed changes.
- `get_f64`, `get_i64`, `get_bool`, `get_string`, each `Result<_, MpvError>`.
- `command(name, &[&str])`, `command_async(name, &[&str])`, `observe_property(name, id, format)`, `request_log_messages(level)`, and `next_event() -> Option<MpvEvent>`.

A property that is unavailable gives `Err`, as today, and the call sites already use `unwrap_or`.

### D3. One thread owns the socket; no reader thread

The session lives on the playback thread, as the handle does today. It holds a non-blocking `UnixStream`, a read buffer, a `VecDeque<MpvEvent>`, and a counter for `request_id`.

- `request` writes `{"command":[...],"request_id":N}` and reads lines until the reply with id `N`. A line that is not that reply is decoded and queued. Order is kept, so a handler that runs later sees events in the order mpv sent them.
- `next_event` pops the queue, or does one non-blocking read.
- The run loop waits with `poll` on the wakeup pipe and the socket together, with the existing 50 ms ceiling for its timers. The libmpv wakeup callback and its lost-byte comment go away: a readable socket is level-triggered.
- A reply that does not come within `REQUEST_TIMEOUT` is a fault: the session kills the child and returns `MpvError::Timeout`. The timeout is a parameter, so tests pass zero.

Rejected: a reader thread plus a channel (a second thread for each run, and it breaks the single-owner rule that today's code relies on).

### D4. Child start and options

`spawn_args(&MpvRunConfig) -> Vec<String>` is a pure function. It turns the `opt!` list in `init_mpv` into arguments with the same values (`config-dir`, `input-ipc-server`, `input-default-bindings`, `input-vo-keyboard`, `wayland-app-id`, `gapless-audio=weak`, and the script options). It adds `--idle=yes` and `--no-terminal`. stdin, stdout, and stderr are null. Settings that `init_mpv` makes with `set_property` after init (caches, `hwdec`, `vo=null`, audio output, `pause`) stay as `set_property` calls on the new session, so the order of effects stays the same.

libmpv's `mpv_create` sets defaults for an embedded client that the `mpv` program does not have. Task 3.1 reads that code in mpv's source and lists each default, so the argument list restores the same ones (for example `idle` and `terminal`, and the keyboard and bindings that `init_mpv` already sets).

Connect to the socket after the spawn with a bounded retry (`connect_with_retry(path, attempts, interval)`). Tests pass a zero interval. `prepare_ipc_socket` still removes a stale socket before the spawn.

Rejected: `--input-ipc-client=fd://N` on a socket pair. It would remove the retry, but it needs a newer mpv, and the server path stays for user tools anyway.

### D5. Child lifetime

- `Drop` for the session sends an async `quit`, then kills and reaps the child. Killing an exited child is harmless. The run's existing 200 ms wait for `Shutdown` stays the grace period.
- If the Owner process dies without running `Drop`, the child must not stay. The child sets `PR_SET_PDEATHSIG` with `SIGKILL` (`CommandExt::pre_exec`, one `unsafe` block with a SAFETY comment).
- `PR_SET_PDEATHSIG` fires when the **thread** that started the child ends, not the process. So one long-lived spawner thread, owned by the player controller, starts every child (the pre-warm and the on-demand start use it). It receives a start request on a channel and returns the `Child`. Rejected: start on the caller's thread (the daemon start-up thread that calls `pre_warm` may end while the pre-warmed child waits).

### D6. Synchronous reads stay as requests

A local socket round trip is cheap, and a fresh read is more correct than a cached value. The probe showed that a property change can land before or after the event that sits next to it. A read at `playback-restart` gives the current value either way. So the reads in `lib.rs`, `run/queue.rs`, `run/events/restart.rs`, `run/events/track_progress.rs`, `runtime.rs`, and `tracks.rs` stay, and each becomes one request. Batching the `track-list` read (a request for each property of each track today) is left for a measurement to justify.

### D7. Failure, quit, and shutdown

- `MpvFailure` gets a variant `ProcessExited`, and `MpvError` has the cases `Spawn`, `NotFound`, `Io`, `Reply` (mpv's error text), `Timeout`, `Closed`, and `Decode`.
- The session sets `quit_requested` when the player sends `quit` (sync or async). When the socket reaches end-of-file (or `poll` reports a hang-up), `next_event` returns `Shutdown` once if `quit_requested` is set, and `Failed(ProcessExited)` if not. This is the S8 finding: IPC never delivers `shutdown`, and `end-file(quit)` may or may not come first.
- `on_mpv_error` ends the run with `PlayerEvent::Stopped { error: Some(..) }` for `ProcessExited`, with the last observed position, through the same teardown that the audio-output failure uses today. A spawn failure (no `mpv` program) takes the existing `player.startup.failed` path with a message that names `mpv`.
- The `EndFile(Quit)` reason stays in the domain type, but no stop path may depend on its arrival.

### D8. Property-change events

`PropertyValue::None` (an unavailable property) needs an arm in `on_property_change`, and the `("video-params/h", change)` arm must not log it as an unexpected type. The IPC client uses `observe_property_string` for the properties that libmpv observes as `Format::String` (`sid`, `aid`, `audio-codec-name`). The decoder from `mpv-ipc-event-probe` types each value by name. The run loop already ignores the position of a property change among other events (the probe saw it change from run to run on both backends).

### D9. Tests

No test starts mpv. Contracts and their owners:

- Request and reply matching, with an event between the request and its reply: a socket-pair peer in `mpv_session` tests. The event is still queued, in order.
- End of file with and without `quit_requested`: the same peer closes the socket. `Shutdown` and `Failed(ProcessExited)` are the two expected outcomes. This cites S8.
- Request timeout: the peer never replies, and the timeout is zero.
- `spawn_args` returns the option list that `init_mpv` set (a pure test, one case table).
- The run loop on recorded traces (`fixtures/S1..S10`): the existing classifier tests stay.

### D10. Documentation that the change must record

An ADR (0032) records "mpv runs in a child process of the Player owner" with the findings as its evidence. `AGENTS.md` ("It embeds mpv") and the README get the new wording. `docs/invariants/` gets one entry: the child never outlives the Owner process, and never serves two runs.

## Risks / Trade-offs

- [The `mpv` program differs in defaults from an embedded client] → Task 3.1 lists the libmpv client defaults from mpv's source, and `spawn_args` restores them. A test pins the list.
- [The mpv version that a user has is older than 0.41, where the probe ran] → The events used are old and stable. Name 0.41 in the ADR. `mpv_ipc_probe` stays as the way to recheck a version.
- [A hung child blocks the run thread] → `REQUEST_TIMEOUT` turns a hang into a fault that kills the child. In-process libmpv has no such bound.
- [An orphan child holds the audio device after an Owner crash] → `PR_SET_PDEATHSIG` from a long-lived spawner thread (D5).
- [Two runs use one socket path] → Same as today: the stale-socket removal and the one pre-warmed instance rule are unchanged. The invariants entry states it.
- [A Debian install without the `mpv` program] → The package depends on `mpv`. Without it, the run fails with a message that names the program (spec).
- [`set_property` value types differ over JSON (for example `aid` as a number, not text)] → The wrapper sends the JSON type that matches the `PropertyValue` variant, and mpv converts the value for its property type. `spawn_args` and the wrapper keep the strings that today's call sites pass.

## Migration Plan

Each group below ends green (`cargo nextest run -p mbv-player`, fmt, workspace clippy) and committed. Only the last code group changes behavior. To roll back, revert that group: the wrapper and the domain error stay valid on libmpv.
