## Context

See proposal.md - Why/What Changes for the two bugs and the log evidence. Relevant state, all on `App`
(`src/app/app_struct.rs`):

- `launched_as_remote: bool` — set once, true for any `App::new_remote`-built instance, `false` for
  `App::new()`. Never reassigned after construction.
- `is_local_daemon: bool` — starts as the constructor's `is_local_daemon` argument, but is
  **mutable at runtime**: `switch_to_direct_remote` and `switch_to_library_route` overwrite it with
  the *newly connected* target's locality every time either is called (added so the visualizer gate
  tracks the current player target, not the launch-time one).
- `home_is_local_daemon: bool` — a one-time, launch-time snapshot of the constructor's
  `is_local_daemon` argument. Never reassigned after construction (unlike `is_local_daemon`).
  Already used by `restore_local_mode` for exactly this "what was I originally" purpose.
- `try_auto_reconnect()` (`src/app/session_connect.rs:174`) — reads `config.auto_reconnect` and
  `last_remote_connection.json`; on a match, calls `switch_to_library_route` or `connect_to_session`
  (which may itself call `switch_to_direct_remote` when the target session's device also exposes a
  reachable ctrl-protocol daemon — the "direct-daemon-upgrade" case visible in the log).
- `teardown()` (`src/app/run_loop_events.rs:187`) — on every clean exit, decides what to write to
  `last_remote_connection.json` from whichever of `active_route` / `connected_session_state` /
  `direct_remote_label` is currently `Some`, or writes `None` (clear) if all three are `None`.

## Goals / Non-Goals

- Goal: a local-daemon-attached client restores a saved auto-reconnect target on startup, the same
  way bare mode already does.
- Goal: a local-daemon-attached client's teardown never overwrites a good saved target with `clear`
  purely because auto-reconnect was structurally never given a chance to run.
- Non-goal: changing behavior for an explicit `--connect-daemon`/`daemon_client_endpoint` launch.
  That path already deliberately skips auto-reconnect (the user gave an explicit target) and this
  change does not touch it.
- Non-goal: fixing whatever caused tonight's manual Sessions-panel connect to "music" to stay a
  plain `AttachedSession` instead of upgrading to `DirectRemote` (see Open Questions) — that
  determines whether the Local/Remote *toggle* specifically reappears, separately from whether
  auto-reconnect itself fires and restores *some* connection.
- Non-goal: fixing the pre-existing, narrower gap where an explicit `--connect-daemon` session that
  manually switches routes mid-session already fails to persist that switch on quit (same root
  mechanism, but out of scope — not what was reported, not touched by this fix; see Open Questions).

## Decisions

### 1. Call `try_auto_reconnect()` from `App::new_remote()`, gated on the constructor's `is_local_daemon` argument

At the tail of `App::new_remote()` (`src/app/construct.rs`, right before returning `app`, after the
existing `handle_failed_local_daemon_adoption` check), add:

```rust
if is_local_daemon {
    app.try_auto_reconnect();
}
```

Using the constructor's local `is_local_daemon` argument (not `app.is_local_daemon`) makes the
intent explicit at the call site and matches `home_is_local_daemon`'s semantics — this must reflect
what the client was launched as, not any later mutation.

**Why gate on `is_local_daemon` at all, instead of always calling it for every `new_remote`
instance:** an explicit `--connect-daemon`/`daemon_client_endpoint` launch is the user stating a
target directly; auto-reconnecting somewhere else on top of that would silently override an explicit
instruction. This mirrors the existing design comment on `App::new_remote`'s `is_local_daemon`
parameter: local-daemon attach "should behave exactly like a plain local session."

**Alternative considered:** call it unconditionally in `new_remote`. Rejected — changes behavior for
explicit remote launches, which isn't what's broken and isn't what `local-daemon-thin-client`'s
existing requirement scopes this to.

**Composition with the rest of `new_remote`:** `try_auto_reconnect()` runs after
`local_daemon_bootstrap`'s queue adoption completes, so it sees a fully-constructed `app` with
`player` already wired to the local daemon (`self.player.is_remote() == true`). Both
`switch_to_library_route` and `switch_to_direct_remote` already have an `else` branch for exactly
this case (`self.player.is_remote()` true at call time): it calls `self.player.disconnect_remote()`
and swaps in the new `PlayerProxy::remote(...)`, instead of the `if !self.player.is_remote()`
branch that suspends an in-process `Player`. No new branch is needed — this is the same code path
already exercised whenever a genuinely-remote-launched client switches to a different remote target
mid-session. The local daemon itself is untouched (per ADR 0015, a client disconnecting never stops
it); this client just stops being one of its attached clients.

### 2. Rebase `teardown()`'s persistence skip-gate onto `home_is_local_daemon` instead of `is_local_daemon`

Current code (`src/app/run_loop_events.rs:207`):

```rust
if self.launched_as_remote && !self.is_local_daemon {
```

Change to:

```rust
if self.launched_as_remote && !self.home_is_local_daemon {
```

**Why this alone is sufficient, no new tracking state needed:** the existing comment justifying the
gate assumes "`App::new_remote` instances never populate `active_route`/`connected_session_state`" —
true before decision 1 above, false after it (and already slightly false today for the narrow case
of an explicit-launch client that manually switches routes mid-session; see Open Questions). Once
decision 1 makes reconnect-on-attach the normal case for a local-daemon launch, that launch's
`is_local_daemon` will very often flip to `false` mid-session (a successful reconnect to a genuinely
remote target sets it, e.g. the "music" daemon-upgrade case) — at which point the *old* gate
(keyed on live `is_local_daemon`) would wrongly skip persisting the connection this session just
worked to establish. `home_is_local_daemon` doesn't have this problem: it's the immutable
launch-time snapshot, so it keeps meaning "was this client launched attached to the local daemon"
for the client's entire lifetime, regardless of what it reconnects to afterward. Tracing all
reachable states through the gate with this change (see the four cases below) shows it's exactly
the condition the original comment was trying to express.

Walking the cases:
- Launched local-daemon, stays local all session (nothing saved, or reconnect attempt failed): gate
  false → decision block runs → correctly computes `clear` (matches bare-mode parity: an
  auto-reconnect attempt that ran and found nothing, or found a now-stale target, clearing it is
  today's accepted bare-mode behavior, not a new risk).
- Launched local-daemon, decision 1's call succeeds and reconnects (to "music" or elsewhere): gate
  false → decision block runs → correctly re-saves the now-active target, even though
  `is_local_daemon` has flipped to `false`.
- Launched explicit remote (`--connect-daemon`): gate true (`home_is_local_daemon` is `false` for
  this launch and never changes) → skipped, exactly as today. Unaffected by this change.
- Bare `App::new()`: `launched_as_remote` is `false`, so the gate's first condition is always false
  regardless of either flag. Unaffected.

This closes bug 2 without introducing a separate "did this session attempt a reconnect" flag: the
launch-time snapshot already carries the right information, decision 1 just needed to stop being
undermined by reading the mutable flag at the wrong point in the session's lifetime.

**Alternative considered:** add a new `App` field (e.g. `local_daemon_reconnect_attempted: bool`)
set once `try_auto_reconnect()` returns, and gate the `clear` branch specifically on it rather than
changing which flag guards the whole block. Rejected as unnecessary complexity — the walk above
shows `home_is_local_daemon` alone produces the right decision in every reachable case, with no new
state and no new field to keep in sync.

**Alternative considered:** never let a `launched_as_remote` session write `clear`, only ever a
still-`Some` value. Rejected — this would leave a stale saved target (device permanently gone,
route removed from config) undeletable except from a bare-mode launch, which is a worse regression
than the one being fixed.

## Risks / Trade-offs

- [A local-daemon session now silently jumps to a remembered remote target on every launch, even
  ones the user intended as "just browse locally for a minute"] → This is exactly bare mode's
  existing, accepted behavior today (any `mbv` launch with `auto_reconnect = true` already does
  this) — decision 1 restores parity rather than introducing new behavior. If this surprises users
  in stay-alive mode specifically, that's a product question about `auto_reconnect`'s scope, not
  something this fix should try to solve unilaterally.
- [Every additional terminal attached to an already-running local daemon (`Resolution::Attach`,
  any number of clients per ADR 0015) will now also independently call `try_auto_reconnect()` on
  its own startup] → Each call only affects that one client's own `player`/`remote_player_tab` (it
  disconnects *that terminal* from the local daemon and connects it elsewhere); the shared local
  daemon and any other already-attached terminal are untouched. Matches bare mode's existing
  single-instance-per-process behavior; not a new multi-client hazard.
- [Sequencing against `retire-pty-relay-for-local-daemon-stay-alive`, still unarchived] → see
  proposal.md's Capabilities note; this change's delta spec is written against that change's
  `local-daemon-thin-client` requirement text as it exists today (already shipped in v0.15.5 code).
  If the two changes archive out of the order assumed here, the delta may need a mechanical rebase
  at archive time — no design impact, just an archiving-order note.

## Migration Plan

No data migration. No config schema change. Existing `last_remote_connection.json` files (including
the one already on disk for the reporting user, `{"kind":"DirectSession","device_name":"music"}`)
are read as-is by the restored `try_auto_reconnect()` call. No rollback concerns beyond reverting the
two call sites.

## Open Questions

- Tonight's manual Sessions-panel connect to "music" logged `connect: device="music"` but not the
  daemon-upgrade path, while an equivalent connect on the pre-#416 build did upgrade
  (`outcome=direct-daemon-upgrade`, full `DirectRemote` queue + toggle). Unconfirmed whether this is
  a further regression in `connect_to_session`'s upgrade probe, or simply that `music`'s own daemon
  wasn't reachable at that specific moment. Doesn't block or change this fix (decision 1 restores
  the *attempt*, using the same `connect_to_session` call already exercised successfully pre-#416);
  worth a quick manual check after this ships — if the toggle still doesn't reappear on a successful
  auto-reconnect, that's a separate follow-up, not a reason to revisit this design.
- The pre-existing, narrower gap where an explicit `--connect-daemon` client that manually switches
  library routes mid-session doesn't get that switch persisted at teardown (same
  `launched_as_remote`-gate mechanism, but that client's `home_is_local_daemon` is legitimately
  `false`) is unaffected by this change and out of scope. Worth its own issue if it matters in
  practice; not touched here since it isn't what was reported and isn't covered by
  `local-daemon-thin-client`'s requirement.
