# Design: fix remote queue after disconnect

## Context

See proposal.md — Why. The bug lives in `App::restore_local_mode`'s reconnect tail
(`src/app/session_connect.rs:500-513`): the `home_is_local_daemon` branch
(`session_connect.rs:456-499`) reconnects to `DaemonEndpoint::Local` and unconditionally
repopulates `remote_player_tab`, leaving `has_remote_queue()` true and the queue-scope pill lit.
Only the `suspended_local` arm (bare mode) takes the correct `None`/`Local` path. `construct.rs`
already encodes the correct end state for this exact scenario (local daemon ⇒ unified
`player_tab`, `remote_player_tab = None`, no pill) at `construct.rs:416-430`.

The tail's `reconnected_local_daemon` is only ever populated in the `home_is_local_daemon`
branch, which always connects to `DaemonEndpoint::Local`, so at runtime the tail is
unambiguously the local-daemon-reconnect case and needs no runtime distinction.

## Goals / Non-Goals

**Goals:**

- Every disconnect path (explicit `d`, `PlayerEvent::RemoteDisconnected`, unannounced daemon
  loss, `DaemonShutdownAnnounced` — all funnel through `restore_local_mode`) lands a
  stay-alive client back on the unified local-daemon queue: `remote_player_tab = None`,
  scope `Local`, no scope pill.
- The daemon's live items remain visible after disconnect (queue is not emptied).
- The reconnect tail and startup (`construct.rs`) agree on how a local daemon's items land in
  `player_tab`.
- Regression coverage using the existing `make_local_daemon_app_stub` /
  `DAEMON_ROUTE_CONNECT_OVERRIDE` fixtures.

**Non-Goals:**

- No change to cold-attach semantics: a fresh `App::new_remote(..., is_local_daemon = true)`
  still restores a saved queue snapshot onto an idle daemon via `local_daemon_bootstrap`
  (that's `local-daemon-thin-client`'s idle-daemon attach requirement and stays untouched).
- No change to genuine remote/network-daemon routing (`switch_to_library_route` to a non-local
  daemon keeps its remote `player_tab`).

## Decisions

### D1: Reconnect tail puts reconnected items into `player_tab`, `remote_player_tab = None`, scope `Local`

The `home_is_local_daemon` reconnect branch sets `player_tab = PlayerTab::new(initial_items,
initial_cursor)` instead of `remote_player_tab = Some(PlayerTab::new(...))`. The shared tail
after the match sets `remote_player_tab = None` and `set_queue_scope(QueueScope::Local)` for
both arms, so the `has_initial_items`-gated scope branch is deleted.

Rationale: `PlayerTab::new(remote_items, remote_cursor)` is exactly how `construct.rs` lands a
non-empty local daemon's items into `player_tab` (the first branch of
`bootstrap_local_daemon_queue`, `src/app/bootstrap.rs:24-33`). With `remote_player_tab =
None`, `has_remote_queue()` (`queue_scope.rs:7-9`) is false, so `has_direct_remote_queue()` is
false, the scope resolution shows no remote scope, and `remote_slot_state()` reads `LocalDaemon`
(`remote_slot_state.rs:19-24`). This makes the fix work for every caller of
`restore_local_mode` with no per-path branching.

Alternative considered: keep `remote_player_tab = Some(..)` and instead change
`has_remote_queue()` to also read `direct_remote_connected`. Rejected — it leaves the ghost
remote tab in place (stale queue data, remote undo stack, remote queue-source metadata) and
papers over the state instead of converging it, whereas the construct.rs invariant already says
this state has no remote tab at all.

Alternative considered: re-run the full `bootstrap_local_daemon_queue(saved_state)` on
reconnect so the empty-daemon case also re-adopts the saved snapshot. Rejected — `restore_local_mode`
is a return to an already-attached daemon, not a cold attach, so the idle-daemon snapshot-restore
requirement doesn't apply; re-adopting could resurrect a queue the user deliberately cleared
during the remote session.

### D2: Mirror construct.rs's non-empty-daemon metadata rather than leaving fields stale

Where construct.rs sets `queue_source = remote.queue_source` for the local-daemon case
(`construct.rs:482`), the reconnect branch applies the same, and the existing
`sync_subtitle_prefs_to_player()` call already mirrors construct.rs:465. `last_played_item_id` /
`last_played_completed` / `positions` enrichment stay as they are — those are snapshot-restore
concerns (non-empty daemon path leaves them unset at startup too), so they are out of scope here.

Rationale: keeps the two local-daemon paths observably identical and avoids a stale
`queue_source` confusing local-queue metadata decisions after a remote session.

## Risks / Trade-offs

- **Stale queue-source parity** → D2 is a small, separately-reviewable change; if the
  reconnect path turns out to already be covered by later `retire_remote_tracking(true)`
  handling, D2's field write is a harmless no-op alignment.
- **Regression test flakiness** → The new test reuses the exact fixture shape of the existing
  `restore_local_mode_reconnects_local_daemon_when_no_suspended_local_player_exists`
  (`tests_route_state.rs:633`): `TestStateDirGuard` + `DAEMON_ROUTE_CONNECT_TEST_LOCK` +
  `DAEMON_ROUTE_CONNECT_OVERRIDE`, so it stays deterministic and non-flaky.
- **Empty-daemon reconnect diverges from cold attach** → Intended (see D1); the only observable
  difference is a reconnect-to-empty daemon does not resurrect a saved snapshot, which is the
  safer behavior. If a future requirement wants snapshot adoption on reconnect, that's a new
  delta against `local-daemon-thin-client`, not this change.
