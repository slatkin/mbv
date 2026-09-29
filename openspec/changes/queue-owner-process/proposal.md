# Proposal

## Why

The queue keeps breaking. Most queue fixes since August have landed on a seam where two copies of one queue get reconciled, for example:

- the attached-session queue-edit echo (#747);
- broadcasts reverting the Loaded Playlist source;
- the Bare local-replacement fence;
- local-daemon attach adoption;
- colliding revision mints (#836);
- progress applied to three copies.

Today a Bare TUI holds `player_tab` plus `bare_owner`, synchronized by whole-queue copies. A Stay-alive Client optimistically edits `player_tab` and then lets the owner's snapshot overwrite it. The rule that Clients only read the queue (invariant 13) is enforced by convention at a dozen `match self.local_queue_owner()` sites.

`unify-queue-playback-authority` (archived 2026-09-07) set this goal. It stopped at two accepted deviations: Bare keeps shell-side mutation with `sync_canonical_queue`, and optimistic Client edits returned. Stay-alive exists only so playback survives the TUI closing, so Bare and Stay-alive are the same owner with two lifetimes. Hosting both lifetimes in one owner process, with one mirror type on the Client, removes the whole class.

## What Changes

- **BREAKING (process model):** the Player owner for a local launch is always a separate owner process. Bare in-process ownership is retired. `stay_alive` becomes a lifetime policy only:
  - **Off:** the owner accepts one Client and shuts down when that Client quits or its connection drops. A second launch is refused with today's message.
  - **On:** unchanged. Any number of Clients may attach, and the owner outlives them.
- The owner reads `stay_alive` when it decides something (an attach, a last-Client disconnect, a shutdown request), not once at spawn. This fixes the spec'd "Stay Alive toggled off during the session" scenario, which the spawn-time `DaemonLoop.stay_alive` flag currently violates.
- Clients hold no editable queue. Both queue scopes (the home owner and a directly controlled remote owner) are read-only mirrors, and adopting an owner snapshot is the only way to write them. The Client's selection is anchored by `QueueSlotId`, not by index.
- Every queue edit is an owner operation with a correlated result. The Client waits a bounded time for its own result and adopts the resulting snapshot before handling the next input, so an edit is painted in the next frame, with no prediction and nothing to roll back.
  - Undo sends the inverse operation.
  - Queue refresh becomes an owner operation.
- Deleted:
  - `bare_owner`, `sync_canonical_queue` and Bare transitions;
  - the in-TUI `Player` (`PlayerProxy::local`);
  - the `sequence_generation` fence;
  - Client queue persistence and restore;
  - `LocalQueueOwner`, `set_queue_source_if_not_local_daemon`, and `QueueOrigin::ThisProcess`;
  - `pending_queue_edit_cursor` and `pending_remote_move_cursor`;
  - Client-side refresh merge;
  - `App::new_independent`.
- A Client keeps its connection to this machine's local daemon (its home link) for its whole life. That includes while a Library route, direct remote control or fall-through points playback elsewhere. Local-scope edits always reach the local daemon, and a stay-alive-off daemon doesn't see a route switch as its Client leaving. Fall-through plays on the local daemon, not on a Player built inside the TUI.
- Behaviour parity for stay-alive-off users: in-TUI service setup on first run, `system_notifications`, no tray.
- Docs:
  - invariant 13 is deleted, because the design enforces it;
  - ADR 0017 (Composed stage) and CONTEXT.md (Bare mode, Stay-alive, Stay-alive process, Client, Composed, Owner-held queue, Tray, the Audiobookshelf eligibility line) are amended;
  - a new ADR records the decision.

## Capabilities

### New Capabilities

None. The behaviour belongs to existing capabilities.

### Modified Capabilities

- `daemon-lifecycle`: Stay Alive off means an exclusive owner process that ends with its Client. An ordinary last disconnect ends a stay-alive-off owner. There is no in-process ownership.
- `local-daemon-stay-alive`: the owner process exists whatever `stay_alive` is set to. "A client exiting never stops the local daemon" and "Stopping … is always explicit" now apply only with Stay-alive on. "Bare mode is unchanged" is removed.
- `local-daemon-single-instance`: the owner process always holds the lock. With Stay-alive off, a second launch is refused through an attach refusal from the exclusive owner. The Bare scenarios are removed.
- `local-daemon-thin-client`: every local TUI is a Client. The requirements and scenarios for Bare persistence and quitting are removed. The "does not own playback" indicator now follows Stay-alive.
- `local-daemon-tray`: no tray while Stay-alive is off (this replaces the Bare scenario).
- `unified-playback-queue`: Clients hold no editable queue. Edits are correlated owner operations painted by the next frame. The Bare branches of edit routing, slot jumps and Client persistence are removed.
- `queue-canonical-list`: the Client holds no playhead prediction; the requirement on clearing the Bare prediction is replaced.
- `player-target-locality`: the in-process player target is removed.
- `non-audio-fall-through`: the "local Player" a fall-through prepares becomes this machine's local daemon, not a Player the TUI builds.

## Impact

- **Code:**
  - `src/main.rs` (startup resolution) and `src/local_daemon.rs` (spawn);
  - `src/app/state/{construct*, app_struct, queue_scope, queue_owner, player_tab, playback}`;
  - `src/app/dispatch/{queue*, action, session/*, run_loop/teardown, library/load, library/event}`;
  - `crates/mbv-daemon` (lifetime policy, exclusive attach, op results, refresh op);
  - `crates/mbv-ctrl` (op id, result event, insert-before and refresh ops, capabilities);
  - `crates/mbv-remote-player` (bounded op call);
  - `crates/mbv-player/src/proxy.rs` (the Local variant is removed).
- **Protocol:** additive ctrl capabilities; `CTRL_PROTOCOL_VERSION` does not change.
- **Persistence:** the Bare queue file is read once by the existing legacy takeover in the owner and never written again.
- **Tests:** Bare-mode and optimistic-edit tests are deleted, not ported. New tests are written against the owner-op and mirror APIs.
