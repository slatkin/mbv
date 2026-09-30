# Design

## Context

Every local launch is a Client of the per-user Owner process (ADR 0030). `run_local_instance`
in `src/main.rs` loops `resolve()` → (spawn if `Fresh`) → `attach_owner_process()`, and already
reacts to two typed admission refusals from `RemotePlayerError`: `is_owner_shutting_down()`
(retry `resolve()` for up to 10 s, then a fresh Owner takes over) and `is_exclusive_owner()`.

The handshake order is: Owner sends hello → Client sends its hello (and control credential) →
Owner sends initial state or a `Disconnected` refusal (`connect.rs` `read_server_hello`,
`send_client_hello`, `read_initial_state`). The Owner's hello carries `app_version` and
`protocol_version`; the Client currently logs `app_version` and drops it, and `validate_peer`
separately handles protocol compatibility (left as is).

`mbv -q` (`stop_running_instance`) reads the PID from the lock file and sends SIGTERM; the Owner
handles SIGTERM as a graceful shutdown (`mbv-daemon/src/run_shutdown.rs`).

## Goals / Non-Goals

**Goals:**
- Never send credentials to, be admitted by, or run against an Owner of a different build.
- Give the user one clear remedy (restart) at the point they hit the problem.

**Non-Goals:**
- No ctrl protocol change or `CTRL_PROTOCOL_VERSION` bump; packaged `mbvd` and explicit
  endpoints are untouched.
- No in-session (TUI) handling: in-app reconnect paths spawn the current binary, so a mismatch
  there is not expected; if it occurs it surfaces as an ordinary connect error.
- No ctrl protocol handling of any kind, and no build identity beyond `app_version` (a
  same-version dev rebuild is not detected).
- #559 Feature 2 (already shipped).

## Decisions

**D1. Check on the server hello, not after attach.** The mismatch is raised by the Client from
the Owner's hello, before `send_client_hello`. This is what gives "otherwise, no connect": the
control credential is never sent and the Owner never admits this Client. Alternatives: attach
then compare (admits a client and sends the credential to an outdated process; with Stay Alive
off it would also occupy the single slot); a separate probe connection (extra connection, extra
admission surface, nothing gained).

**D2. Mismatch = `app_version` differs, nothing else.** The local Owner is the same binary as the
Client, so its application version is the only meaningful comparison. Ctrl protocol version is
an mbvd (server) concern: `protocol_version` is neither compared nor reported by this feature,
`validate_peer` and its errors are untouched, and `ctrl-protocol` is not modified. The
`app_version` check runs immediately before `validate_peer()` (it must precede
`send_client_hello` either way); when a release differs in both fields, the prompt therefore
wins over the generic protocol error for the local Owner, and when only the protocol differs
the existing error is unchanged. If an older Owner's hello cannot be deserialised at all, the
existing "invalid daemon protocol hello" error stands (no prompt) — accepted limit.

**D3. Local-only via an explicit parameter, not by inspecting the stream.** A two-variant enum
(`PeerBuild::Any` / `PeerBuild::MustMatch`) is threaded from `connect_endpoint` (which knows the
`DaemonEndpoint`) down to `read_server_hello`. `Local` passes `MustMatch`; `Unix`/`Tcp` and the
existing `perform_handshake`/`perform_service_setup_admin_handshake` entry points pass `Any`, so
current behaviour and tests are untouched. A bool would be an unnamed flag at five call
boundaries; the enum is the named state. `signal_local_daemon_service_setup` keeps `Any`: it is
best-effort and already reports a restart requirement on failure.

**D4. New `RemotePlayerError` kind `OwnerBuildMismatch`** carrying the Owner's `app_version`,
with an accessor returning it (sibling of `is_owner_shutting_down`). Its `Display` names both
application versions and `mbv -q`, so every caller that only prints the error
(in-app connect paths) still shows an actionable message.

**D5. Restart is SIGTERM-by-PID, reusing the existing wait loop.** Stay Alive on makes
`RequestShutdown` reject, and an old Owner may not honour newer lifecycle semantics; SIGTERM is
the `mbv -q` path and works across versions. The signalling moves out of `stop_running_instance`
into a helper in `single_instance.rs` that returns the PID or a small domain error
(`NoOwnerPid` / `Signal(io::Error)`); `-q` keeps its exact messages by formatting that result.
After signalling, `run_local_instance` does not add a new wait: it sets `restart_requested` and
falls into the same bounded retry-resolve branch as `is_owner_shutting_down`, which ends in a
`Fresh` spawn from this binary.

**D6. After R, a mismatch is a wind-down, not a new prompt.** A stopping Owner still sends its
hello first and refuses only after the Client hello, so the retry loop would re-detect the
mismatch and re-prompt. The follow-up decision is a pure function of `(error, restart_requested)`
returning `Prompt`, `WaitForOwnerExit`, or `Other`; `restart_requested && mismatch` →
`WaitForOwnerExit`. It lives in the new `src/owner_restart.rs` so this trap is unit-testable
without `process::exit`.

**D7. Prompt is plain line input on stdin/stderr, before any UI.** Precedent: interactive login
already runs as plain terminal I/O before the daemon starts. No raw mode, no TuiRealm: this is
not TUI keyboard routing (`src/app/input/` is untouched). The function takes
`impl BufRead`/`impl Write` so it is tested hermetically. `r`/`R` (after trim) = restart;
everything else, empty, and EOF = quit. The caller checks `stdin().is_terminal()` and skips the
prompt entirely (print error, exit 1) when it is not a TTY. The warning line ("restarting stops
playback and closes other terminals") is always shown: the Client does not know the Owner's
live `stay_alive` or attached-client count, and the handshake is not extended to learn them.

**D8. User-facing wording says "Owner process"**, matching the existing `main.rs` messages and
`CONTEXT.md`; no new domain term.

## Risks / Trade-offs

- [Every version bump now gates launch] → Intended; the prompt is one keypress, and the old
  behaviour was silently running against a stale process.
- [Restart ends playback and closes other terminals] → Stated in the prompt. SIGTERM persists
  the queue; a non-video position may not be preserved. No attempt to detect other clients.
- [SIGTERM wind-down exceeds the 10 s retry window] → Existing timeout message already names
  `mbv -q`; unchanged.
- [`read_pid` returns nothing or the signal fails on R] → Report the failure and exit non-zero;
  do not spawn a second Owner.
- [Same-version dev rebuilds are not detected] → Accepted; `mbv -q` remains the manual path.
- [`main.rs` growth] → Prompt and follow-up logic go in `owner_restart.rs`; `main.rs` (587
  lines) gains only the loop branch.

## Migration Plan

None: no persisted state, config, or wire format changes. Rollback is reverting the commit.
