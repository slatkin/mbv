# Design

## Context

See proposal.md, "Why". The current state that shapes the approach:

```
BARE (stay_alive=false)              STAY-ALIVE CLIENT                   DIRECT REMOTE (either)
App.player_tab.queue  (edited)       daemon PlayerOwnerState (canon)     peer PlayerOwnerState (canon)
App.bare_owner.queue  (sync copy)    App.player_tab (optimistic+adopt)   App.remote_player_tab (optimistic+adopt)
in-TUI Player thread + mpv           App.queue_source/dirty/undo         SuspendedLocalSession.player_tab/queue_source
sequence_generation fence            App.bare_owner (idle, meaningless)  RemotePlayer.items/queue_source (optimistic)
Client queue file (queue_state)      pending_queue_edit_cursor           Stay-alive: local daemon link DROPPED
```

**Startup and lifetime**

- **Startup.** `src/main.rs` `run_local_instance` → `single_instance::resolve`, which only does a raw connect (`single_instance.rs:78`).
  - `Fresh` either spawns the local daemon (`stay_alive`) or builds `App::new_independent` (Bare).
  - `Attach` goes to `run_remote_app`.
  - `Refuse` means the lock is held and the socket refuses, which today means a Bare instance is running.
- **Shared daemon library.** `mbv_daemon::run_with_options` serves both `DaemonRole::Local` and packaged `mbvd` (`DaemonRole::Packaged`, `crates/mbvd/src/main.rs:636`).
- **Settings the daemon captures at spawn:**
  - `DaemonLoop.stay_alive` (`run.rs:533`). `prepare_shutdown` rejects a shutdown whenever that captured value is true, so the spec'd "Stay Alive toggled off during the session" is rejected today for a daemon spawned with Stay Alive on.
  - `consume_videos`/`consume_audio`, read from the spawn-time config clone (`event_loop/player_events.rs:198-202`).
  - `show_audio_window`, forced to false for every role (`run.rs:192`).
- **Client removal.** A ctrl client leaves the registry through `handle_ctrl_disconnected`, or silently through `retain` in any of four broadcast helpers in `ctrl.rs`: `broadcast_to_all`, `broadcast_state_gated`, `broadcast_progress_gated` and `broadcast_book_progress_gated`. Some of those run on the status-broadcast thread.
- **Connection handshake.** `core_ctrl_spawn.rs` validates `Hello`, sends the initial queue state, and then registers the client.
- **Shutdown.** `handle_shutdown` (`event_loop/control_events.rs`) persists the queue, announces the shutdown, and stops the player (a bounded join) before exiting. The lock stays held and the socket stays connectable throughout.
- **Persistence migration exists.** The owner's first start reads the Bare queue file when its own file is absent (`initialize_queue`, `legacy_queue_for_owner_if_absent`).

**The ctrl queue protocol**

- **Vocabulary.** It already carries every edit as a unified command: `UnifiedQueueReplace`, `LoadIdle`, `Append`, `RemoveSlot`, `RemoveSlots`, `MoveSlot`, `PlaySlot`, `Clear`, `SourceUpdate` and `AdoptQueue`.
  - `UnifiedQueueClear` is a unit variant.
  - `UnifiedQueueLoadIdle` has its own `request_id`/`UnifiedQueueLoadResult` correlation and can wait up to 30 s for the prior item to stop (`control/queue_load.rs`).
- **Broadcast and rejection.** The owner broadcasts `UnifiedQueueState` after each edit, with four ABS-capability variants per peer (`control_queue.rs`). Rejections reach only the sender (`CommandRejected`).
- **Request/response precedent.** `RemotePlayer::request_shutdown` registers a completer, sends the request, and waits with `recv_timeout`.

**The Client**

- **Event draining.** It drains one player event per tick through `handle_player_event` (`shell/run/drains.rs:76-94`).
  - Some events (`RemoteDisconnected`) replace `self.player_rx`.
  - The handler can return `RestartLoop`.
  - The drain runs `push_*` projections after each event.
  - `handle_unified_queue_updated` writes `self.player.status`.
- **Queue copies outside `player_tab`:**
  - `SuspendedLocalSession` holds `player_tab` and `queue_source` and writes them back in `install_suspended_local`.
  - `RemotePlayer` keeps `items`/`queue_source` mirrors, and `adopt_queue`/`play`/`play_queue` write them and `status.current_idx` before the owner answers.
- **Queue writes driven by events.** Besides edits, the Client writes to queue slots from:
  - `record_reported_progress` and consume in `player_event.rs`;
  - feed hydrate (`action.rs`), `feed_tab.rs`, and `event_reconcile.rs`;
  - `playlist.rs` (`playlist_item_id`);
  - ABS acknowledged-progress events;
  - the ABS Socket.IO merge (`audiobookshelf-progress-refresh`). The TUI opens that socket in every mode.
- **Fall-through.**
  - It builds an in-TUI `Player` (`App::construct_local_session`, `prepare_local_player`).
  - `play_pending_local_play` stops and disconnects the current player when it is remote, and that includes the home local-daemon link of a Client controlling an audio-only Emby session.

## Goals / Non-Goals

**Goals:**

- One Player-owner host for local launches, and no terminal UI process that owns a Player.
- On the Client, no value that can hold an editable queue: queue slots, source, and optimistic mirrors included. The type makes the Client-side half of invariant 13 unrepresentable.
- An edit is visible in the next frame. Each edit has an owner-answered result that is adopted before the next input is handled.
- Stay-alive-off users keep today's Bare behaviour:
  - quit stops playback, and a second terminal is refused;
  - there is no tray, and the queue survives restarts;
  - `system_notifications`, `show_audio_window`, first-run service setup and Emby startup behave as today;
  - mid-session `consume_*` toggles take effect;
  - ABS progress from other devices updates the queue.

**Non-Goals:**

- Changing packaged `mbvd` behaviour. Every lifetime and settings change here is gated to `DaemonRole::Local`.
- Making `PlayerOwnerState.queue` private inside the owner. There is one owner host after this change; that encoding is follow-up work.
- Moving the queue cursor into the queue component, or queue dirtiness into the owner.
- Changing `QueueEpoch`/`QueueOrigin` beyond deleting the `ThisProcess` variant.
- Changing Cast dispatch, Session watch, or the Emby remote-control authority handover.

## Decisions

### D1 — The local daemon is the only local Player-owner host

Every launch without an explicit endpoint resolves to the local daemon: start it if absent, then attach. `App::new_independent`, `construct_local_session`, `PlayerProxy::local`, `PlayerProxyInner::Local`, `inhibit_mpv` and `PlayerProxy::is_remote()` are deleted. Deleting `is_remote()` makes the compiler list every Bare branch, and each branch is deleted rather than ported. Those branches include:

- the in-process teardown join and `stop_for_shutdown`;
- `reset_bare_transitions` / `expire_bare_transition` and `bare_in_flight_slot`;
- the `sequence_generation` fence;
- Client queue persistence;
- `LocalQueueOwner` and `QueueOrigin::ThisProcess`;
- `bare_owner`;
- the `LOCAL_PLAYER_PREPARE_OVERRIDE` seam.

The sites span `src/app` production code and test harnesses (`tests/tick_integration/harness.rs`, `tests/audiobookshelf_runtime.rs`, `tests/actions_tests_queue_state_reseat.rs`, and others).

*Alternative rejected: an in-process owner behind the same op/snapshot interface.* One owner implementation but two hosting paths, so modes still diverge. Stay-alive exists only so playback survives the TUI closing.

### D2 — Owner settings are read live, for the Local role only

`DaemonLoop.stay_alive: bool` and the spawn-time `consume_*` reads are replaced by an injected `OwnerSettings` reader, `Arc<dyn Fn() -> OwnerSettings + Send + Sync>`, where `OwnerSettings { stay_alive, consume_videos, consume_audio }`. It is injected like `store: OwnerQueueStore`, and it is shared with the ctrl connection threads because admission happens there.

- **`DaemonRole::Local`.** Each call reads those keys from the user's config file.
  - If the read fails, or the file is mid-rewrite and unparsable, it returns the last value it read successfully. The first fallback is the spawn-time config.
- **`DaemonRole::Packaged`.** It returns the spawn-time config with `stay_alive: true`. So `mbvd` is never exclusive and never shuts down on its last client, as today.
- **Call sites:**
  1. admission, at `Hello`;
  2. `LastClientGone`;
  3. `prepare_shutdown`;
  4. the consume-policy read at track completion.

The settings overlay debounces its save (`settings.rs` `settings_save_at`). Toggling `StayAlive`, `ConsumeVideos` or `ConsumeAudio` flushes the save at once, and quit flushes any pending save before `RequestShutdown`.

`show_audio_window` is honoured for `DaemonRole::Local`. The spawned daemon inherits the user's display environment (`local_daemon::session_display_env`); `Packaged` still forces it off.

*Alternative rejected: the Client pushes settings over ctrl.* Several Clients could push different values, while the config file is the single per-user source, and it would need a new protocol message for values both processes can already read.

### D3 — Exclusive admission, shutdown window, ending with the Client

All of D3 applies to `DaemonRole::Local` only.

**Admission.**

- In `core_ctrl_spawn.rs`, after `Hello` validation and **before** `send_initial_queue_state`, take the registry lock. Then:
  - if `shutting_down` is set, send `Disconnected(OwnerShuttingDown)` and close;
  - otherwise, if `!settings().stay_alive` and the registry has a client, send `Disconnected(ExclusiveOwner { pid })` and close;
  - otherwise `connect`, still under the same lock.
- `send_initial_queue_state` runs only after admission.
- `DisconnectReason` gains both variants. An older Client binary talking to a newer daemon can't parse them and sees a failed attach; that is a version-skew window only (see Risks).

**Client refusal handling.**

- `RemotePlayer::connect_endpoint` maps the reasons to `RemotePlayerError::ExclusiveOwner { pid }` and `RemotePlayerError::OwnerShuttingDown`.
- `main.rs`:
  - `ExclusiveOwner` prints the refusal message and exits 1.
  - `OwnerShuttingDown` makes it re-run `single_instance::resolve` every 100 ms for up to 10 s. That turns into `Fresh` once the old owner releases the lock, so a quick relaunch after quitting just works.
  - `Resolution::Refuse` prints that the owner process is not accepting connections and names `mbv -q`.
  - The `-q` help text drops "bare mbv".

**Shutdown window.** `handle_shutdown` sets `shutting_down` under the registry lock as its first step.

**Last client lost.**

- `CtrlClients` takes the loop's `merged_tx` at construction, so `CtrlClients::default()` in `run.rs` is replaced by a constructor.
- Any removal that leaves it empty after it has held a client sends `DaemonEvent::LastClientGone`. That covers `remove` and all four pruning `retain` sites.
- The loop handles the event: if the role is Local, `!settings().stay_alive`, and the registry is still empty, it runs the `DaemonEvent::Shutdown` handling.
- A freshly spawned daemon has never held a client, so it can't exit before its launcher attaches. A launcher that dies before attaching leaves the daemon running until `mbv -q` or the next launch.

**The tray.** It starts only when `settings().stay_alive` is true at startup.

### D4 — The home link lives as long as the Client

A Client launched without an explicit endpoint keeps its local-daemon link, the home link, for its whole life.

- **`SuspendedLocalSession` becomes link-only.** It holds `player: PlayerProxy` and `player_rx`. Its `player_tab` and `queue_source` fields and their write-back in `install_suspended_local` are deleted. The Local view already tracks the owner (see below).
- **Route switches suspend the home link.** Direct remote control, a Library route, or a daemon route switch moves the home link into `SuspendedLocalSession` instead of calling `disconnect_remote()`. When `stay_alive` is false, it sends Stop first, preserving Bare's stop on switch; with Stay Alive on, the owner keeps playing. Switching away from a non-home remote still disconnects that remote.
- **No second connection to the local endpoint.** Every code path that would open a new `DaemonEndpoint::Local` connection while a home link exists reinstates the suspended home link instead: the `endpoint.is_local()` branch of `switch_to_direct_remote`, `connect_daemon_route_endpoint` targeting `Local`, and `restore_local_mode`. A stay-alive-off owner would refuse the second connection. `daemon_restart.rs` opens a new link only after the owner is gone, so its registry is empty.
- **Draining the suspended home link.** The shell drains it **until empty** every tick, in `drain_suspended_home_events`:
  - `UnifiedQueueUpdated` and `QueueOpResult` go to `adopt_home_snapshot`, a separate adopt path that writes only the Local view and never `self.player.status`.
  - `RemoteDisconnected` goes to the existing owner-lost flow (`raise_daemon_lost_modal`).
  - `DaemonShutdownAnnounced` goes to the clean-exit flow.
  - Every other event is dropped.
- **`App::queue_link(scope)`** returns the home link (current or suspended) for Local and the current player for Remote.
- **Teardown with Stay Alive off** sends `RequestShutdown` over the home link. The short-lived `DaemonEndpoint::Local` connection path is deleted.
- **Fall-through.**
  - If the current player **is** the home link, as for a Client controlling an audio-only Emby session, `prepare_local_player` returns "already local". `play_pending_local_play` stops the Emby session and neither stops nor disconnects the home link.
  - If the home link is suspended, it is reinstated.
  - A Client launched on an explicit endpoint has no home link, so it runs the local-launch resolution (start or attach). Refusal is returned as `Err`.

*Alternative rejected: drop the home link and reconnect later, as Stay-alive does today.* With Stay Alive off, the owner would lose its only client and exit with its queue. With Stay Alive on, the Local view freezes into a private copy.

### D5 — `QueueView`: one adopt-only queue per scope, source included

`PlayerTab` is replaced by `QueueView` (`src/app/state/queue_view.rs`). `player_tab`/`remote_player_tab` become `local_view: QueueView` and `remote_view: Option<QueueView>`, and `App.queue_source` is deleted. The adopted source lives on `QueueView` (`source()`).

The fields are private. The surface is:

- read accessors: `slots`, `item_at`, `emby_item_at`, `slot_id_at`, `slot_index`, `revision`, `source`, `lineage`, `pending_playback_slot`, `total_queue_len`, `cursor`;
- `set_cursor` for user navigation;
- `from_snapshot`;
- `adopt(&UnifiedQueueStateData, AdoptCause)`.

```
enum AdoptCause {
    OwnAnswer,                 // result of this Client's own edit
    Background { held: bool }, // any other snapshot; held = user navigated recently
    Replacement,               // whole-queue replacement
}
```

`adopt` applies `unified-playback-queue`, "Queue selection follows the selected slot":

- the cursor is kept as the selected `QueueSlotId`, falling back to the clamped old index;
- `Background { held: false }` moves it to the active slot;
- `Replacement` moves it to the start.

A lineage change between the previous and the adopted snapshot marks the adoption as `Replacement`. `QueueView` mints a Client-local `QueueRevision` on every adoption through the existing `QueueRevisionMint`.

**Queue copies removed from `RemotePlayer`:**

- The `items` and `queue_source` fields are deleted, along with the writes to them and to `status.current_idx` in `adopt_queue`/`play`/`play_queue`.
- `unified_queue` stays. Only the reader thread writes it, from `UnifiedQueueState` and from `QueueOpResult::Applied`.
- `RemoteSnapshot::take` and `switch_to_direct_remote` read `unified_queue_state()`.

**What goes away from the Client:**

- the `PlayerTab` mutators;
- `App::merge_refreshed_queue`, `consume_slot_from_active_playback_queue`, and the local mutation in `replace_playback_queue`;
- `pending_queue_edit_cursor` and `pending_remote_move_cursor`.

`pending_queue_cursor_reanchor` stays, only for scope switches.

*Alternative rejected: keep `PlayerTab` with `pub(crate)` mutators.* Visibility can't stop another shell method from calling a mutator.

### D6 — Answered queue operations

**Wire (`crates/mbv-ctrl`, additive, capability `answered-queue-ops`):**

- `QueueOpId(u64)` is minted per connection by the Client.
- These commands gain `#[serde(default)] op: Option<QueueOpId>`: `UnifiedQueueReplace`, `UnifiedQueueAppend`, `UnifiedQueueRemoveSlot`, `UnifiedQueueRemoveSlots`, `UnifiedQueueMoveSlot`, `UnifiedQueuePlaySlot`, `UnifiedQueueSourceUpdate`.
- `UnifiedQueueLoadIdle` is **not** an answered op. It keeps its `request_id`/`UnifiedQueueLoadResult` correlation, because an idle load may wait up to 30 s for the prior item to stop. The Client already handles that result asynchronously and doesn't block on it.
- `UnifiedQueueClear` stays a unit variant for legacy peers. The new struct variant `UnifiedQueueClearOp { op }` is sent only to peers with the capability, and the daemon handles both through one clear function.
- `UnifiedQueueAppend` gains `#[serde(default)] before: Option<u64>`: insert before that slot, or at the end when `None`. An absent anchor is rejected as stale slot addressing.
- New commands:
  - `UnifiedQueueRefresh { op }`;
  - `UnifiedQueueApplyProgress { op, updates: Vec<ProgressUpdate> }`, where each `ProgressUpdate` carries the provider-qualified identity, position and finished flag. It relays ABS Socket.IO pushes to the owner (see D9).
- New event `QueueOpResult { op, outcome: QueueOpOutcome::{Applied(Box<UnifiedQueueStateData>), Rejected(String)} }`.

**Owner (`crates/mbv-daemon`):**

- **The `op` reaches every handler.** It is threaded through `CtrlContext`. **Every** handler path answers when `op` is present, including silent early returns such as an empty append or an empty removal set. A no-op answers `Applied` with the unchanged snapshot.
- **The answer goes to the sender only.** It is built with the sender's ABS capability gating (`unified_queue_state_for_peer`). `broadcast_queue_state` gains `except: Option<CtrlClientId>`, so the sender doesn't adopt its own edit twice as a `Background` snapshot. `reject_command` sends `QueueOpResult{Rejected}` when `op` is present.
- **Append with `before`** inserts into the canonical queue with `PlaybackQueue::insert` and keeps the player run's copy in order with `PlayerCommand::QueueAppend` followed by `PlayerCommand::QueueMove(slot, index)`.
- **`UnifiedQueueRefresh`** starts the existing enrichment fetch and answers `Applied` at once. The merged result arrives later as a broadcast through `apply_queue_enriched`.
- **`UnifiedQueueApplyProgress`** applies each update to matching inactive slots through the owner's progress path (respecting invariant 02 protection) and answers with the resulting snapshot.

**Client:**

- `RemotePlayer::send_queue_op(QueueOp) -> Result<Option<QueueOpId>, RemotePlayerError>` returns `Some` when the peer advertises the capability. Otherwise it sends the legacy form and returns `None`.
- `App::queue_op(scope, op)` sends, then `await_queue_op(scope, id)` pumps that link's receiver until `QueueOpResult{op: id}` or `QUEUE_OP_ANSWER_BOUND = 250 ms`. The pump:
  - adopts `UnifiedQueueUpdated` inline as `Background` (or through `adopt_home_snapshot` for a suspended home link);
  - adopts the matching `QueueOpResult` as `OwnAnswer`;
  - appends **every other event** to `App.deferred_player_events: VecDeque<PlayerEvent>`. `drain_player_events` takes from it before `player_rx`, so `RemoteDisconnected`, `RestartLoop`, and the per-event `push_*` projections keep running at the tick level, in order.
- **Late answers.** An `Applied` that arrives after the bound, or that matches no waiting op, is adopted as `Background`. A late `Rejected` flashes nothing more, because the timeout already reported.
- **Legacy peers.** Peers without the capability get the edit and no wait. Undo of a removal, refresh, and progress relay report "not supported by this owner".

*Alternatives rejected:*

- **Fire-and-forget plus a tick wake.** A second keystroke acts on a stale view.
- **A typed provisional overlay.** It is a second queue answer.
- **A completer map in `RemotePlayer`.** Snapshots queued ahead of the answer would be adopted after it.

### D7 — Undo sends the inverse operation

`UndoEntry` becomes `Remove { item: QueueItem, index: usize }` or `Move { slot_id, from: usize }`.

- **Undoing a removal** resolves `before` at undo time as the slot now at `index`, or `None` past the end, then sends `Append{items: [item], before}`.
- **Undoing a move** sends `MoveSlot{slot_id, to: from}`.
- **The undo stack** stays per Client and per scope.

### D8 — Parity for Stay Alive off

The remote constructor (`App::new_remote_optional_with_config`) does what `new_independent` did:

- calls `should_open_services`/`open_services_settings`;
- sets `setup.emby_startup_request` when Emby is configured and no cached client was passed in;
- sets `system_notifications` from config when `stay_alive` is false, and keeps it `false` otherwise.

On the owner side, D2 covers `show_audio_window` (Local role) and the live `consume_*` reads. MPRIS is started by every local Client, as Stay-alive Clients do. For stay-alive-off users this adds media keys, which Bare lacked. That is accepted and goes in the release notes.

### D9 — Client event side effects keep their non-queue work only

The Client stops writing to queue slots at every event-driven site, and keeps the side effect that isn't a queue write:

| Site | Deleted queue write | Kept side effect |
|---|---|---|
| `player_event.rs` `record_reported_progress` (Stopped/TrackCompleted) | slot progress | none; the owner records it and publishes |
| `player_event.rs` consume on Stopped/TrackCompleted | `consume_slot_from_active_playback_queue` | `on_video_consumed`/`on_audio_consumed` (playlist save on consume, `queue_dirty`), triggered by the event's `consume` flag and slot kind resolved from the adopted view |
| feed hydrate (`action.rs`), `feed_tab.rs`, `event_reconcile.rs` | slot item/position rewrite | `FeedEntryStore` writes; the queue shows the owner's copy |
| `playlist.rs` `playlist_item_id` clear | slot field | kept only as Client playlist bookkeeping outside the view; it is dropped if nothing reads it after the view change |
| `teardown.rs` `flush_playing_position_on_teardown` | slot progress | `last_played_item_id` write kept |
| ABS acknowledged-progress event (`audiobookshelf-podcast-playback`) | slot progress | browse-state progress; the queue reflects the owner's snapshot, and the owner publishes one after applying acknowledged progress to its own slots |
| ABS Socket.IO `user_item_progress_updated` | slot progress | browse-state progress, plus a relay to the home owner as `UnifiedQueueApplyProgress` |

## Risks / Trade-offs

- **[The owner loop blocks inside a queue handler]** → The UI waits up to 250 ms per edit. The owner task audits the unified handlers for blocking I/O and moves any it finds onto worker/merged-event paths first. Audit result (row 2.4): queue-mutation paths are clean (in-memory/channel only); refresh/enrichment already runs on the worker and merges through the event loop; one move made — owner queue persistence after mutations is now a FIFO-serialized worker with shutdown flush before the final direct write.
- **[Version skew]** → An older Client binary receives an unknown `DisconnectReason` from a newer daemon and reports a failed attach. It affects only the window between upgrading the binary and restarting an old daemon, and `mbv -q` clears it.
- **[A launcher dies before attaching]** → A stay-alive-off daemon with no clients lingers until `mbv -q` or the next launch. It is logged.
- **[The config is unreadable when the owner decides]** → The last-known-good value is used (D2), never a default that could wrongly refuse a client or exit.
- **[Relaunch during shutdown]** → Bounded retry (D3). If the old owner hangs for longer than 10 s, the user sees the refusal naming `mbv -q`.
- **[A large deletion blast radius in `src/app`]** → The compiler drives the deletions. Old Bare and optimistic-edit tests are deleted, not ported.
- **[Media keys gained (D8)]** → Deliberate; noted in the release notes.

## Migration Plan

1. The owner-side lifetime, admission, settings and answered ops ship first (task groups 1 and 2). They are additive and inert for current Clients (the Local role only).
2. Client attach-always, home-link retention and the Bare deletion (groups 3 and 4) ship in the same release as `queue_op` and `QueueView` (groups 5 and 6). An intermediate commit may use fire-and-forget, but no release ships without answered ops.
3. **Rollback** is reverting the Client commits. The owner changes are backward compatible, and the Bare queue file is never deleted.

### D10 — Glossary: Owner process

The maintainer approved this rename. CONTEXT.md introduces **Owner process**: the per-user local process that is the Player owner for every terminal UI launched on this machine. **Stay-alive** becomes only the lifetime policy that decides whether the Owner process outlives its Clients. "Stay-alive process" and "Bare mode" become _Avoid_ aliases. The specs keep their existing "local daemon" wording.
