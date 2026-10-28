# Design

## Context

See `proposal.md` (Why). Current facts that shape the approach:

- A Client that connects to a peer keeps two ctrl links. `attach_remote_owner` (`src/app/dispatch/session/switch.rs`) moves the local Owner link into `suspended_local` and makes the peer link `self.player`. The suspended home link keeps playing and is drained every tick. So during a connection the Client can already address both Player owners.
- The Client keeps one `QueueView` per link: `local_view` and `remote_view`. A user-selectable `queue_scope` (`src/app/state/queue_scope.rs`) chooses which view is shown and which link takes edits. The Queue component sends `QueueRequest::Scope` on `[` and `]` (`crates/mbv-components/src/queue/keys.rs`). A library route also fills `remote_view`, through the same `attach_remote_owner`.
- `CtrlCmd::UnifiedQueueReplace` carries slots, `start_idx`, and `source`, but no start position. The start position of each kind comes from `resume_source` (`crates/mbv-player/src/resume.rs`).
- F3 keys live in `crates/mbv-components/src/sessions.rs`: `Enter` sends `ShellRequest::SelectSession(key)`, `d` sends `DetachSessions`, `r` refreshes.
- MPRIS lives in the Client and rebinds on every link change (`rebind_mpris_to_current_player`). The Tray lives in the Owner process and reads only the local Player.

## Goals / Non-Goals

**Goals:**

- Hand off and Take as Client-run sequences over the two links the Client already has.
- One additive ctrl field for the start position. No protocol version bump.
- The shown queue follows the connection. No user-selectable scope.

**Non-Goals:**

- Owner-to-Owner links. The local Owner never talks to a peer Owner.
- Tray changes. The Tray stays local by design.
- New behavior for Emby Session and Cast rows, or for library route selection.
- Any change to how a peer Owner admits items.

## Decisions

### D1. The Client runs Hand off and Take

The Client already holds the local Owner link and, while connected, the peer link. It reads the source snapshot and position, sends the replacement to the target, waits for the target to report playback, and then sends `Stop` to the source.

```
Hand off                                   Take
1 confirm if peer queue not empty          1 confirm if local queue not empty
2 sever current connection, connect peer   2 open peer link (reuse when connected)
3 read local snapshot + status position    3 read peer snapshot + status position
4 peer: replace(slots, start_idx, pos)     4 local Owner: replace(slots, start_idx, pos)
5 wait: peer observes start slot playing   5 wait: local observes start slot playing
6 local Owner: Stop                        6 peer: Stop
7 stay connected                           7 close link; queue panel shows local
```

Alternative: the local Owner holds the peer link, as for Cast and Session outputs. Rejected. It needs an Owner-to-Owner ctrl client and a second authority relation, and its only gain is a Tray that follows the peer, which the user does not want.

### D2. A pending transfer is one shell value, settled in the tick phase

The shell holds at most one `PendingQueueTransfer { direction, target, start_slot, op }`. It is set after step 4 is sent. It settles in the same tick-phase step that adopts owner snapshots:

- success: the target's adopted snapshot names `start_slot` as the observed active slot, and the status plays. Then run step 6 and step 7.
- failure: the target rejects the replacement, the answer has no pending transition for the start slot (admission dropped it), or the target link is lost. Then report the failure and do not stop the source.
- cancel: a new F3 action, a disconnect, or a new Hand off or Take replaces the pending value. The source keeps playing.

The replacement answer uses the existing answered-edit bound. Playback observation has no own timeout, because the user can cancel and the source still plays. When the source has no active slot, the sequence ends after the answer, with no wait and no stop.

Alternative: stop the source first, then start the target. Rejected. A failed target leaves no music.

### D3. Admission is checked before the copy and backstopped by the answer

Before step 4, the Client checks the target's advertised capabilities. If the target is audio-only and the active item is not audio, the Client refuses with a message and sends nothing. Other inactive items that the target cannot hold are dropped by owner admission, as for any replacement. The answer check in D2 is the backstop for every other refusal.

### D4. Start position is an optional field plus a capability

Add `start_position_secs: Option<f64>` with `#[serde(default)]` to `UnifiedQueueReplace`, and a capability `CTRL_CAP_QUEUE_REPLACE_START_POSITION` (`"queue-replace-start-position"`), following the ctrl variant recipe in `docs/recipes.md`. The field is meaningful only with `start_idx`. The Client sends it only to an owner that advertises the capability.

In the Owner, the field becomes a one-time override for the start slot. `mbv-player` gets a `ResumeSource::Handoff(seconds)` value. It is not derived by `resume_source(item)`. The replace path sets it for the start slot only, and the one-percent gate does not apply. Every later start in that queue uses `resume_source` as before.

Alternative: seek after start. Rejected. The item plays audibly from the wrong position before the seek, which the existing start-slot rule in `unified-playback-queue` forbids.

### D5. The shown queue follows the connection; the user scope goes

Remove the `queue_scope` field, `QueueRequest::Scope`, `set_queue_scope`, the `[`/`]` handler in the Queue component, `display_peer_queue_on_connect`, and the `visible_scope` half of `QueueScopeResolution`. The shown queue is `remote_view` when a peer link is current, else `local_view`. Edits go to the link whose view is shown.

Some code must still address the hidden link. A Service purge (`remove_queue_slots_where`) edits both views. Hand off and Take read and write both. For this, keep a two-value link address. Rename `QueueScope { Local, Remote }` to `QueueLink { Local, Peer }`, so the retired glossary term *Queue scope* does not stay in code. The rename is mechanical.

A library route also fills `remote_view`. After D5 a route shows the route peer's queue, as a connection does, with the peer named. Today the user reaches that view with `]`. Route selection itself does not change.

### D6. F3 keys

In the Sessions component, add `h` for Hand off and `t` for Take. Both send a new `ShellRequest` with the stable target key, as `SelectSession` does: `HandOffSession(key)` and `TakeSession(key)`. The keys are offered only when the selected target has a direct endpoint, so the component receives a per-target `has_direct_endpoint` projection with the rows. Shell dispatch resolves the key against the current snapshot and does nothing when the target is gone. The footer lists the keys. These are fixed sidebar keys, like `r` and `d`, not registry actions.

### D7. Queue owner label

The queue footer already has one informational indicator (commit `b17763718`). It shows only while a remote owns playback. Change it to always show, naming the local Owner or the connected peer by device name. It stays non-interactive.

### D8. Copy contents

The copy carries the item sequence, `start_idx`, and the Queue source. The target mints new slot identities and a new lineage. Feed positions and Audiobookshelf progress stay in their own stores. A playlist Queue source is copied, so save-on-consume can run on whichever owner plays the queue.

### D9. Glossary and ADR

`CONTEXT.md`: add *Queue handoff* (the Client action that copies a queue to a peer, starts it there, and stops local playback) and *Take* (the reverse). Keep *hand-off* for the move of the active slot between Playback outputs, and add *Queue handoff* to its *Avoid* cross-reference. Retire *Queue scope*. Update *Direct remote control* (connect moves control only; the aqua scope pill is gone). A new ADR records "each Player owner keeps its own queue; Hand off and Take copy it" and updates ADR 0012 (F3 now owns Hand off and Take as well as connect).

## Risks / Trade-offs

- [Two copies drift after a Hand off] → This is intended. The queue panel always names its owner, so the user sees which copy they edit.
- [Brief double playback during the wait in D2] → The source stops as soon as the target reports playback. The overlap is a short and accepted trade for never losing the music.
- [The `QueueScope` rename touches about 30 files] → Do it as its own mechanical task with no behavior change, before the scope removal.
- [Library route presentation changes as a side effect of D5] → Recorded in D5 and in the `remote-queue-disconnect` and `queue-playback-panel` deltas. The route rules stay parked.
- [Peers on older builds] → Hand off is refused with a message when the peer lacks the capability. Take needs only the local Owner, which ships with the Client.

## Migration Plan

Additive ctrl field and capability: an old peer keeps working for connect and disconnect. No stored state changes. Rollback is a revert.

## Open Questions

- The exact device-name source for the owner label when a peer's Emby Session name is unavailable (fall back to the endpoint host). This does not change the specs.
