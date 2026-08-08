## Context

See `proposal.md` — Why, and ADR 0017 for the model this implements.

The constraints that shape the approach:

- `audio_only_rejection` (`daemon_core.rs:565`) is load-bearing. `PlayItems`
  hands the whole fetched list to `play_queue` (`daemon_control.rs:396`), which
  loads it into mpv as an mpv playlist that mpv advances through unaided. The
  rejection is the only thing keeping video away from a player with no display.
- It is reached from three places: `daemon_control.rs:361` (ctrl play),
  `daemon_run.rs:559` (playback intents), and the ws path via `daemon_ws.rs`.
- `CtrlHello::current()` takes no arguments and is shared between the daemon
  hello (`daemon_core.rs:589`) and the client hello (`current_client`). The
  daemon side has no access to `audio_only` at that point.
- The client already keeps two queues: `player_tab` (its own) and
  `remote_player_tab: Option<PlayerTab>` (the owner's), selected by
  `queue_for_scope` (`queue_scope.rs:15`), with separate undo stacks.
- The client's local Player is suspended, not discarded, while attached —
  `suspended_local: Option<SuspendedLocalSession>` (`app_struct.rs:244`),
  populated by both `switch_to_direct_remote` and `switch_to_library_route`.
- `restore_local_mode` (`session_connect.rs:417`) is the only path that wakes it,
  and it also calls `disconnect_remote()` and clears the route.
- `debug_assert_eq!(self.player.is_remote(), self.player_endpoint.is_some())`
  appears at `session_connect.rs:290`, `:380`, and in `restore_local_mode`.

## Goals / Non-Goals

**Goals:**

- One admission point per submission path, so the owner's queue and its mpv
  playlist can never diverge.
- The client decides routing from an advertised capability, before submitting.
- Wake the suspended local Player without tearing down the ctrl connection.
- Leave the attachment fields (`active_route`, `connected_session_id`,
  `home_is_local_daemon`, `player_endpoint`) with their current meanings.

**Non-Goals:**

- Any index mapping between the owner's queue and mpv's playlist. The design
  exists to avoid needing one.
- Reporting owner-side discards over ctrl.
- Both players producing sound at once.
- Changing library-route resolution order or config.

## Decisions

### Filter at admission, not at advance

`audio_only_rejection` becomes an admission filter returning the admitted items
and a discard count, applied where the item list is resolved and before it
reaches `play_queue`/`play`. The owner's `items`, its cursor, and mpv's playlist
stay one list.

*Alternative considered:* keep non-audio items in the owner's queue, mark them
unplayable, and skip at advance. Rejected in ADR 0017 — it splits the owner's
list from mpv's playlist permanently, for visibility better provided at the
client.

### One filter, three call sites

The filter is a single pure function over `&[MediaItem]`, called from the ctrl
play path, the intent path, and the ws path, mirroring how `audio_only_rejection`
is called today. Keeping it pure preserves the existing property that it is
testable without a live `Player` or `EmbyClient`.

### Start index is remapped, not clamped

Filtering shifts positions, so a start index computed against the unfiltered
list is wrong. It is remapped to the first admitted item at or after the
original position, falling back to the last admitted item. Clamping to
`len - 1` after filtering, which is what `PlayItems` does today, would silently
start the wrong track.

### Capability advertisement gets its own constructor

`CtrlHello::current()` cannot learn `audio_only` without changing every caller.
A `CtrlHello::current_daemon(audio_only: bool)` constructor is added and used at
`daemon_core.rs:589`; `current()` keeps its meaning for the client path.
`audio_only` is threaded into `spawn_ctrl_client`, which does not receive it
today.

*Alternative considered:* push the capability onto the vec at the call site.
Smaller, but leaves nothing preventing a future daemon hello path from
forgetting it.

### Routing decision sits with route resolution, not inside PlayerProxy

The choice of target happens at the explicit play/enqueue sites alongside
`apply_route_for_playback` (`actions.rs:186`, `:231`), which is already the
point where a target is chosen. `PlayerProxy` stays a dumb pair of variants.

*Alternative considered:* teach `PlayerProxy` to switch its own `inner`.
Rejected — it would put a policy decision behind a transport abstraction, and
the routing inputs (item type, owner capability, explicit-vs-advance) are not
visible there.

### Fall-through wakes the local Player without disconnecting

A new path takes `suspended_local`, installs it as the active player, and
rebinds MPRIS — the same three steps `restore_local_mode` performs — but does
not call `disconnect_remote()`, does not clear `active_route`, and does not
touch `player_endpoint`. The owner is stopped explicitly before local playback
starts. Returning reverses it: re-suspend the local Player, restore the remote
as active target.

`restore_local_mode` is left alone. It remains the disconnect path.

### Active playback target becomes an explicit value

A single value on `App` answers "where does playback go", set when a target is
chosen and read wherever that question is being asked. The
`debug_assert_eq!` pairings are replaced by it rather than relaxed.

The audit is the bulk of the work: 27 non-test `is_remote()` call sites, each
read for whether it asks about the connection (keeps `is_remote()`) or about
the playback target (reads the new value). Sites in `playback_target_local.rs`,
`queue_actions.rs`, `remote_slot_state.rs` and `consume_quit_actions.rs` are the
likely target-question cluster; `session_connect.rs` and
`run_loop_events_teardown.rs` are the likely connection-question cluster.

### The pinned row is rendered, not stored

The row is derived at render time from the local player's status plus the
active-target value. It is not inserted into `remote_player_tab.items`, so
nothing downstream — cursor bounds, queue mutations, undo, projection slot
mapping — has to learn about a member that is not really there.

## Risks / Trade-offs

- **An `is_remote()` site classified wrongly fails only in fall-through, which
  nothing currently tests.** → The change adds coverage for the fall-through
  state specifically; the audit is done site-by-site with the question written
  down per site rather than in bulk.
- **`bootstrap.rs` builds `player_tab` from remote items for a client that
  starts up already attached, so there is no separate local queue on that
  path.** → Fall-through needs an explicit answer there; treated as a task, not
  assumed to work.
- **Stopping the owner discards its position.** → Accepted, per ADR 0017.
- **A client exiting mid-item leaves the owner stopped.** → Accepted; no
  recovery is added.
- **`CtrlHello::current()` is shared with the client path.** → Splitting the
  daemon constructor out risks the two drifting. Mitigated by `current_daemon`
  delegating to `current()` and only appending.
- **Discarding is silent at the owner.** → Accepted, per ADR 0017. The client
  strips first and reports, so an owner-side discard means the client's type
  information was wrong or no client was involved.

## Migration Plan

No migration. The capability is additive: a daemon that does not advertise it
produces exactly today's behavior against any client, and a client that does not
recognise it submits exactly as it does today. The `AudioOnly` rejection stays
in place throughout, so partial deployment degrades to current behavior rather
than to a broken state.

## Open Questions

- Whether the pinned row shows elapsed/remaining in the same format the owner's
  now-playing row uses, or a distinct one. Affects rendering only; specs and
  tasks are unchanged either way.
