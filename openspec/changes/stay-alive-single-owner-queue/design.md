# Design

## Context

See `proposal.md` for the problem and the two delta specs for behavior. Today `execute_pending_queue_action` handles non-autostart loads with `replace_playback_queue`, which stamps the Client's generation one ahead of the owner; `player_event.rs` skips owner snapshots while fenced. `UnifiedQueueReplace` starts playback and carries no source; `UnifiedAdoptQueue` is cold-owner-only. The owner broadcasts queue/source/status from `daemon_control.rs`, but the shell currently ignores broadcast source after a stale-source fix. `QueueRevision` resets on some replacements and is not a usable owner-wide ordering key (invariants 01 and 05). Bare mode, Direct remote control, observed Sessions, and cast have different queue semantics. The Stay-alive process and packaged `mbvd` share core handler code but are different product surfaces.

## Goals / Non-Goals

**Goals:** The Stay-alive process accepts an idle whole-queue replacement (including source) as one owner operation, with previous playback stopped; every attached Client displays its accepted snapshot. Follow-up Play addresses the owner-assigned slot. Source-only Save As updates the same owner's queue label without replacing or restarting it.

**Non-Goals:** Remove Composed queues from Bare mode or change the Local/Remote scope split for Direct remote control; change packaged `mbvd`'s load behavior; change Emby playlist API requests; introduce a second queue or a new UI surface. The owner-owned queue rule applies when the Client is presenting/controling its home Stay-alive queue, not when an observed Session or cast is the playback target.

## Decisions

### 1. An explicit idle replacement operation, not Stop plus the existing play command

Add a ctrl operation for whole-queue **load without play**, carrying items/slots, cursor, Queue source and a request identity. Negotiate an additive capability; a Client attached to an older Stay-alive peer lacking it fails visibly instead of falling back to a private replacement or sending an unknown command. Restrict the new behavior to Stay-alive owner role at the TUI dispatch and owner admission boundaries. Keep existing play-submit behavior elsewhere. An empty accepted load clears the queue and stops playback. Reuse existing item/Service admission; reject non-empty submissions with no admitted slots before disturbing current playback. Do not reuse cold-only `UnifiedAdoptQueue`, or chain `Stop` and `UnifiedQueueReplace` (the latter starts a run, and separate commands can expose mismatched states).

The owner serializes accepted load commands in its event loop, finalizes/stops the prior run under its existing progress-reporting policy, invalidates outstanding old-run transitions/observations, installs new owner-assigned slots and source, and broadcasts one coherent stopped snapshot. A late event from the old run is ignored by run/transition identity and must not mutate the new queue. If stop/finalization can fail, report failure rather than claim a stopped replacement; do not publish a new queue with an old run still playing. The owner sends a correlated accept/reject result so the initiating Client can distinguish its own load from another Client's broadcast. An accepted result is emitted after the new state is committed/published; a lost connection leaves the result unknown, so reconnect must fetch the owner state rather than retry automatically.

### 2. Owner snapshots drive Stay-alive UI; no optimistic replacement

For a non-autostart load targeting home Stay-alive, send the idle load and keep displaying the last confirmed owner state until an accepted owner snapshot arrives. Do not set `queue_source`, save a replacement snapshot, claim Loaded, or advance a local generation before acceptance. Reconcile the owner's queue **and source** together; source changes from successful Save As must reach the owner as a source-only update, guarded by an owner-minted replacement lineage so delayed Save As from an earlier playlist cannot rename a later queue. A source-only update publishes a snapshot; it must not stop or replace playback. On rejection or disconnection, surface failure/unknown outcome, then adopt the live snapshot on reconnect. A queued Client request never becomes a durable private queue.

For Stay-alive, remove generation-fence suppression as an authority test: generation equality across two Clients says nothing about whether their queues match. Instead retain the latest owner snapshot in the Client, including source and owner-assigned slots. Whole-queue replacement is serialized by the owner; per-slot edits target owner slot IDs and their results reconcile from owner snapshots. Keep any local transient cursor movement and the independent Composed behavior of non-Stay-alive targets.

### 3. Treat snapshots as a coherent ordered stream, not a structural revision alone

The existing `QueueRevision` tracks shape (and has known reset/bump gaps), so it cannot order source-only, status-only, or replacement events. Preserve per-connection event order; on reconnect reset the local snapshot to the new owner handshake, then apply that connection's subsequent owner snapshots in order. Correlated load results identify *acceptance*, not final queue ownership: another Client may replace the queue immediately afterward. Use a distinct owner-minted queue lineage for stale Save As protection, not a coincidental Client generation or a recycled Queue slot ID. Do not invent a revision comparison on top of the currently unread `QueueRevision` without first repairing its contract.

### 4. Persist confirmed state only

A Client of Stay-alive saves only the last confirmed owner queue/source (or relies on owner shutdown persistence); its old saved queue can seed a genuinely cold empty owner at initial attach, as the existing thin-client contract requires. Revisit `bootstrap_local_daemon_queue`/cold adoption to ensure an intentionally empty owner queue cannot be mistaken for an uninitialized owner and repopulated from an older saved snapshot. A successful empty load must stay empty across subsequent attaches/restarts. Do not rewrite persistence for Bare or packaged `mbvd`.

## Risks / Trade-offs

- Stop is asynchronous in the current Player implementation → trace stop acknowledgment/finalization before defining the owner commit point; never publish new slots paired with old active status. Capture with owner-level mocked lifecycle tests, not sleeps or real mpv.
- An old run may report completion after replacement → carry or check run identity at owner application; assert no new-slot progress/consume from old reports.
- Multiple Clients and delayed Save As → compare owner-minted queue lineage at the source-update boundary; test A load then B load with crossed completion order.
- Connection lost after sending a load → outcome is unknown, not automatically failed; show last confirmed state and reconcile on reconnect, without retry.
- Empty owner versus cold owner is presently inferred from `queue.is_empty()` → represent cold/adoptable separately or durably record accepted clear, so a later Client cannot resurrect an old snapshot.
- Shared core code serves packaged `mbvd` → gate Stay-alive semantics explicitly and retain existing packaged/direct-remote behavior.

## Migration Plan

Introduce additive capability and idle-load/result commands in Client and owner together. An older live Stay-alive process without the capability remains connected but refuses idle loads visibly; it is never given a private queue fallback. On restarting the Stay-alive process, normal cold-owner adoption remains available where appropriate. During rollout, preserve persisted queue format and existing play/clear ctrl operations. Rollback uses the prior Client and owner together; no persisted schema migration is required.
