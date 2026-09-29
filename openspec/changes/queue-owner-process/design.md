# Design

## Context

See proposal.md, "Why". The current state that shapes the approach:

```
BARE (stay_alive=false)              STAY-ALIVE CLIENT                   DIRECT REMOTE (either)
App.player_tab.queue  (edited)       daemon PlayerOwnerState (canon)     peer PlayerOwnerState (canon)
App.bare_owner.queue  (sync copy)    App.player_tab (optimistic+adopt)   App.remote_player_tab (optimistic+adopt)
in-TUI Player thread + mpv           App.queue_source/dirty/undo         App.player_tab (Bare: live local;
sequence_generation fence            App.bare_owner (idle, meaningless)    Stay-alive: frozen private copy,
Client queue file (queue_state)      pending_queue_edit_cursor              local daemon link DROPPED)
```

- **Startup.** `src/main.rs` `run_local_instance` → `single_instance::resolve`. On `Fresh`, it either spawns the local daemon (`stay_alive`) or builds `App::new_independent` (Bare). `Attach` goes to `run_remote_app`. `Refuse` means the lock is held and the socket refuses, which today means a Bare instance is running.
- **Daemon lifetime.** `DaemonLoop.stay_alive` is captured at spawn (`crates/mbv-daemon/src/run.rs:533`). `prepare_shutdown` (`control.rs`) rejects a shutdown whenever that captured value is true. So the spec'd scenario "Stay Alive toggled off during the session" is rejected today whenever the daemon was spawned with Stay Alive on.
- **Client removal.** A ctrl client leaves the registry in two ways: `handle_ctrl_disconnected`, or silently inside `ClientRegistry::broadcast_to_all` when its channel fails.
- **ctrl vocabulary.** It already carries every edit as a unified command: `UnifiedQueueReplace`/`LoadIdle`/`Append`/`RemoveSlot`/`RemoveSlots`/`MoveSlot`/`PlaySlot`/`Clear`/`SourceUpdate`/`AdoptQueue`. The owner broadcasts `UnifiedQueueState` after each edit, and rejections reach only the sender (`CommandRejected`).
- **Request/response precedent.** `RemotePlayer::request_shutdown` registers a completer, sends the request, and waits with `recv_timeout`.
- **Owner-side refresh.** The daemon already merges refreshed items (`apply_queue_enriched`, `merge_refresh_for_slots`). The Client has its own merge (`App::merge_refreshed_queue`).
- **Fall-through.** It builds an in-TUI `Player` (`App::construct_local_session`, `prepare_local_player`).
- **Persistence migration exists.** The owner's first start reads the Bare queue file when its own file is absent (`initialize_queue`, `legacy_queue_for_owner_if_absent`).

## Goals / Non-Goals

**Goals:**

- One Player-owner host for local launches, and no terminal UI process that owns a Player.
- On the Client, no value that can hold an editable queue. The type makes invariant 13 unrepresentable instead of documenting it.
- An edit is visible in the next frame. Each edit has an owner-answered result that is adopted before the next input is handled.
- Stay-alive-off users see the same behaviour as today: quit stops playback, a second terminal is refused, there is no tray, the queue survives restarts, and `system_notifications` is honoured.

**Non-Goals:**

- Making `PlayerOwnerState.queue` private inside the owner. After this change there is one owner host, and its 24 mutation sites in `crates/mbv-daemon` belong to that single authority. That encoding is follow-up work, not part of invariant 13.
- Changing packaged `mbvd`, Cast dispatch, Session watch, or the Emby remote-control authority handover.
- Moving the queue cursor out of the shell into the queue component (see AGENTS.md "Interactive architecture"). The cursor stays where it is; only its anchoring changes.
- Moving queue dirtiness into the owner. `queue_dirty` stays Client-side, as it already works for Stay-alive.
- Changing `QueueEpoch`/`QueueOrigin` beyond deleting the `ThisProcess` variant.

## Decisions

### D1 — The local daemon is the only local Player-owner host

Every launch without an explicit endpoint resolves to the local daemon: start it if absent, then attach. `App::new_independent`, `construct_local_session`, `PlayerProxy::local` and `PlayerProxyInner::Local` are deleted.

Once `PlayerProxy` is always remote, `PlayerProxy::is_remote()` is always true. Deleting it makes the compiler list every Bare branch, and each branch is deleted rather than ported. Those branches include:

- the teardown in-process join;
- `reset_bare_transitions` / `expire_bare_transition`;
- the `bare_in_flight_slot` prediction;
- the `sequence_generation` fence (`PlayerTab.sequence_generation`, `stamp_queue_generation`, the `ThisProcess` branch of `local_queue_is_owner_queue`);
- Client queue persistence (`save_queue_state*`, `restore_queue_state`, `spawn_enrich_queue_state`, `handle_queue_enriched`);
- `LocalQueueOwner` with `owns_local_persistence` and `set_queue_source_if_not_local_daemon`;
- `QueueOrigin::ThisProcess`;
- `bare_owner`.

*Alternative rejected: an in-process owner behind the same op/snapshot interface.* It keeps one owner implementation but two hosting paths (startup, persistence, event draining, eligibility, owner loss), so modes can still diverge. Stay-alive exists only so playback survives the TUI closing, so a second host buys nothing.

### D2 — Stay Alive is read by the daemon when it decides

`DaemonLoop.stay_alive: bool` becomes an injected reader, `stay_alive: Arc<dyn Fn() -> bool + Send + Sync>`, in the same way `store: OwnerQueueStore` is injected today. It is shared with the ctrl connection threads, because admission happens there. Production reads the `stay_alive` key from the user's config file. It is called in three places:

1. **Admission**, at `Hello`, in the per-connection thread (`core_ctrl_spawn.rs`).
2. **Losing the last client**, when the registry transitions from one or more clients to zero.
3. **`prepare_shutdown`.**

Tests inject a closure.

The settings toggle must have written the config file before the daemon reads it. The implementing task checks that the `StayAlive` toggle path persists immediately, and makes it do so if it doesn't.

*Alternative rejected: the Client pushes `SetStayAlive` over ctrl.* Several Clients could push different values, while the config file is the single per-user source. It also adds a protocol message for a value both processes can already read.

### D3 — Exclusive admission and ending with the Client

- **Admission.** At `Hello` in `core_ctrl_spawn.rs`, the check and the `ClientRegistry::connect` call happen under one registry lock, so two simultaneous connections can't both pass. When `stay_alive()` is false and the registry already has a client, the daemon sends `Disconnected(DisconnectReason::ExclusiveOwner { pid })` and closes the connection before registering it or sending any state. `DisconnectReason` gains that variant; it's additive, and older Clients treat it as an unknown disconnect.
- **Surfacing the refusal.** `RemotePlayer::connect_endpoint` surfaces it as `RemotePlayerError::ExclusiveOwner { pid }`. `main.rs` prints the existing refusal message, reworded per the spec, and exits with status 1.
- **The other refusal case.** `Resolution::Refuse` (lock held, socket not accepting) keeps its exit, with a message saying the owner process is not accepting connections and pointing to `mbv -q`.
- **Last client lost.** Clients are pruned on other threads too: `broadcast_to_all` runs on the status-broadcast thread as well as the loop. So `ClientRegistry` holds the loop's `merged_tx`, and whenever a removal of either kind (`remove` or pruning) leaves it empty after it has held a client, it sends `DaemonEvent::LastClientGone`. The loop handles that event by reading `stay_alive()`. When it's false and the registry is still empty (a new client may have attached in the meantime), the loop runs the `DaemonEvent::Shutdown` handling, the same path `mbv -q` and an accepted `RequestShutdown` take: persist, announce, stop, exit.
- **Startup is safe.** A freshly spawned daemon has zero clients and has never had one, so it cannot shut down before its launcher attaches. If the launcher dies before attaching, the daemon stays up until `mbv -q` or the next launch attaches; that race is accepted.
- **The tray.** At startup the tray hook requires `stay_alive()` in addition to `show_systray_icon`. Toggling afterwards doesn't add or remove it.

### D4 — The home link lives as long as the Client

A Client launched without an explicit endpoint keeps its local-daemon `RemotePlayer`, the home link, for its whole life:

- **Route switches suspend the home link.** Direct remote control, a Library route, or a restore from fall-through suspends it inside the existing `SuspendedLocalSession` (which already holds a `PlayerProxy` and its `player_rx`) instead of calling `disconnect_remote()` on it. Switching away from a non-home remote still disconnects that remote, as it does now.
- **The Local view keeps tracking the owner.** While suspended, the shell drains the home link's `player_rx` every tick. It adopts `UnifiedQueueUpdated` snapshots and queue-op results into the Local view and drops other events, so the Local scope keeps showing the owner.
- **Edits pick their link by scope.** One resolver, `App::queue_link(scope) -> Option<&PlayerProxy>`, returns the home link (current or suspended) for Local and the current link for Remote.
- **Stop on switch only with Stay Alive off.** Suspending the home link sends Stop to the home owner only when `stay_alive` is false. That preserves Bare's current stop on switching to a direct remote. With Stay Alive on, the owner keeps playing, which preserves Stay-alive's current behaviour.
- **`restore_local_mode` reinstates the suspended home link.** Its `home_is_local_daemon` reconnect branch goes away, except for the owner-lost case, which keeps the existing restart/reattach flow.
- **Teardown with Stay Alive off** uses the home link for `RequestShutdown`. The short-lived `DaemonEndpoint::Local` connection path is deleted, because an exclusive owner would refuse it as a second client.
- **Fall-through** prepares the local Player by reinstating the suspended home link. A Client launched on an explicit endpoint has no home link, so it runs the local-launch resolution: start or attach, where the attach may be refused as exclusive.

*Alternative rejected: drop the home link and reconnect later, as Stay-alive does today.* With Stay Alive off, dropping the only client ends the owner and its queue. With Stay Alive on, the Local view freezes into a private copy that Local edits then mutate, which violates invariant 13 today.

### D5 — `QueueView`: an adopt-only queue for both scopes

`PlayerTab` is replaced by `QueueView` in `src/app/state/queue_view.rs`. `player_tab` and `remote_player_tab` become `local_view: QueueView` and `remote_view: Option<QueueView>`.

Fields are private. The public surface is:

- read accessors: `slots`, `item_at`, `emby_item_at`, `slot_id_at`, `slot_index`, `revision`, `pending_playback_slot`, `total_queue_len`, `cursor`;
- `set_cursor(index)` for user navigation;
- `from_snapshot`;
- `adopt(&UnifiedQueueStateData, AdoptCause)`.

```
enum AdoptCause {
    OwnAnswer,                 // result of this Client's own edit
    Background { held: bool }, // any other snapshot; held = user navigated recently
    Replacement,               // whole-queue replacement (lineage changed)
}
```

`adopt` applies the selection rule from `unified-playback-queue`, "Queue selection follows the selected slot":

- The cursor is kept as the selected `QueueSlotId`. If that slot is gone, it takes the old index, clamped.
- `Background { held: false }` moves the cursor to the active slot.
- `Replacement` moves it to the start entry.

**Detecting a replacement.** A lineage change between the previous and the adopted snapshot means a replacement. A Client's own `UnifiedQueueReplace` answer is `Replacement`.

**What goes away:**

- the mutators `set_items`, `set_queue_items`, `remove_slot_at`, `insert_item_at`, `append_item(s)`, `move_slot`, `clear`, `merge_refresh`, `sync_active_slot`, `consume_slot`, and progress application;
- `App::merge_refreshed_queue`, `consume_slot_from_active_playback_queue`, `replace_playback_queue`'s local mutation, `pending_queue_edit_cursor` and `pending_remote_move_cursor`.

`pending_queue_cursor_reanchor` stays, only for queue-scope switches (see `set_queue_scope`).

**Keeping repaints working.** `QueueView` mints a Client-local `QueueRevision` on every adoption (`queue-canonical-list`, "A queue revision names one queue state"). It does this through the existing `QueueRevisionMint`, reusing `PlaybackQueue::from_queue_items` internally, so the projection fingerprint behaves as before.

*Alternative rejected: keep `PlayerTab` and make its mutators `pub(crate)`.* Visibility can't stop another shell method from calling a mutator. Removing the mutators from the type is the only enforcement.

### D6 — Answered queue operations

**Wire (`crates/mbv-ctrl`, additive, capability `answered-queue-ops`):**

- `QueueOpId(u64)` is minted per connection by the Client.
- These commands gain `#[serde(default)] op: Option<QueueOpId>`: `UnifiedQueueReplace`, `UnifiedQueueLoadIdle`, `UnifiedQueueAppend`, `UnifiedQueueRemoveSlot`, `UnifiedQueueRemoveSlots`, `UnifiedQueueMoveSlot`, `UnifiedQueuePlaySlot`, `UnifiedQueueClear`, `UnifiedQueueSourceUpdate`.
- `UnifiedQueueAppend` gains `#[serde(default)] before: Option<u64>`: insert before that slot, or at the end when `None`. An absent anchor is rejected as stale slot addressing.
- New command `UnifiedQueueRefresh { op }`.
- New event `QueueOpResult { op: QueueOpId, outcome: QueueOpOutcome }`, where `QueueOpOutcome` is `Applied(Box<UnifiedQueueStateData>)` or `Rejected(String)`.

**Owner (`crates/mbv-daemon`).** When a command carries `op`:

- The handler sends `QueueOpResult` to the sender only, and broadcasts the resulting state to every other client.
- `broadcast_queue_state` gains an `except: Option<CtrlClientId>`. Without the exclusion, the sender would adopt its own edit twice, the second time as a `Background` snapshot, which would snap the cursor to the playing entry. Excluding the sender avoids that.
- `reject_command` sends `QueueOpResult{Rejected}` instead of `CommandRejected` when `op` is present.
- `UnifiedQueueRefresh` starts the owner's existing enrichment fetch for its Emby slots and answers `Applied` with the current snapshot at once. The merged result arrives later as an ordinary broadcast through `apply_queue_enriched`.

**Client.**

- `RemotePlayer::send_queue_op(QueueOp) -> Result<Option<QueueOpId>, RemotePlayerError>` returns `Some(id)` when the peer advertises `answered-queue-ops`. Otherwise it sends the legacy form and returns `None`.
- The shell then calls `App::await_queue_op(scope, id)`. That pumps that link's `player_rx` through the ordinary event handler, in delivery order, until it sees `QueueOpResult{op: id}` or the deadline `QUEUE_OP_ANSWER_BOUND = 250 ms` passes. It then adopts the result with `AdoptCause::OwnAnswer`, or reports the rejection or timeout.
- Pumping the same channel rather than registering a completer keeps ordering intact: any snapshot the owner sent before the answer is adopted before the answer.

**One entry point.** `App::queue_op(scope, QueueOp)` wraps the send and the wait. All edit dispatch goes through it, including remove, remove-range, move, append, insert, clear, load, replace, play-slot, source update, refresh, and undo. The `sync_playback_queue_items_after_append`, `queue_edit_reaches_player` and `queue_edits_reach_owner` branches collapse into this single path.

**Peers without the capability** (older remote peers) get the edit as today and no wait. Their undo of a removal and their refresh report "not supported by this owner".

*Alternatives rejected:*

- **Fire-and-forget plus a tick wake.** It fixes latency but not ordering: a second keystroke dispatched before the first answer is adopted acts on a stale view.
- **A typed provisional overlay.** That is design B from exploration: it reintroduces a second queue answer.
- **A completer map in `RemotePlayer`.** Snapshots queued in `player_rx` ahead of the answer would be adopted after it, out of order.

### D7 — Undo sends the inverse operation

`UndoEntry` becomes `Remove { item: QueueItem, index: usize }` or `Move { slot_id, from: usize }`, and it records nothing else:

- **Undoing a removal** resolves `before` at undo time as the slot now at `index`, or `None` past the end, then calls `queue_op(Append{items: [item], before})`.
- **Undoing a move** calls `queue_op(MoveSlot{slot_id, to: from})`.
- **The undo stack** stays per Client and per scope (`queue_undo_stack`, `remote_queue_undo_stack`).

### D8 — Parity for Stay Alive off

The remote constructor (`App::new_remote_optional_with_config`):

- calls `should_open_services` / `open_services_settings` as `new_independent` did;
- sets `system_notifications` to the config value when `stay_alive` is false. It stays `false` with Stay Alive on, as today.

MPRIS is started by every local Client, as Stay-alive Clients already do. For Stay-alive-off users this adds media keys, which Bare lacked. This is accepted: MPRIS follows the Client, and one Client is the Stay-alive-off case.

## Risks / Trade-offs

- **[The owner loop blocks inside a queue handler, e.g. on network I/O]** → The UI thread waits up to 250 ms per edit. Mitigation: the bound, plus the D6 task audits the unified queue handlers for inline blocking I/O and moves any it finds onto the existing worker/merged-event path before the Client starts waiting.
- **[The Client dies before sending `Hello` right after spawn]** → A Stay-alive-off daemon with no clients lingers until `mbv -q` or the next launch attaches. It is accepted, logged, and cleared by the next launch.
- **[The config toggle is not yet persisted when the daemon reads it]** → Covered by the D2 verification step.
- **[A legacy Bare queue file exists alongside a Stay-alive owner file]** → The owner file wins, as today. The Bare file is never written again. No migration code is added.
- **[A large deletion blast radius in `src/app`]** → Tasks are ordered so deleting `PlayerProxy::is_remote` and the `PlayerTab` mutators drives the compiler. Old tests that exercise Bare or optimistic edits are deleted, not ported, and new tests are written against `QueueView::adopt`, `queue_op`, and the daemon lifetime/admission paths.
- **[Media keys for Stay-alive-off users (D8)]** → A deliberate behaviour gain, called out in the release notes.

## Migration Plan

1. The owner-side lifetime, admission and answered ops ship first (task groups 1 and 2). They are additive and invisible to current Clients.
2. The Client switches to always attach and deletes Bare (groups 3 and 4) in the same release as `QueueView` and `queue_op` (groups 5 and 6). An intermediate commit may use the legacy fire-and-forget path, but no release has Bare deleted without answered ops.
3. **Rollback** is reverting the Client commits. The owner-side changes are backward compatible. The Bare queue file is still readable by an older binary because nothing deletes it.

## Open Questions

- **CONTEXT.md naming.** "Stay-alive process" names the owner process even when Stay Alive is off. The docs task proposes introducing **Owner process** (the per-user local Player owner) and making Stay-alive purely a lifetime policy, with "Stay-alive process" and "Bare mode" becoming _Avoid_ aliases. This is a glossary rename, so it needs the maintainer's approval before the CONTEXT.md edit. The specs keep their existing "local daemon" wording either way.
