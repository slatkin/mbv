---
status: accepted
---

# The Stay-alive Process Owns and Persists Its Queue

Beside [ADR 0015](0015-local-daemon-for-stay-alive.md), which made Stay-alive
"ensure the local daemon exists, then attach". 0015 settled *who runs*; this
ADR settles *who owns the queue*.

## Problem

Before this decision, a Client attached to the Stay-alive process still
behaved partly as a queue authority of its own: a populate-only playlist
load could stage in the Client's Composed snapshot without the owner,
Clients persisted queue snapshots that could resurrect a cleared queue on
attach, source-only updates (Save As) carried client-asserted authority,
and two attached terminals could show divergent Local queues while one
owner played. Every sync and fence mechanism that tried to paper over this
shared one property: the TUI was treated as a peer of the owner instead of
a reader.

## Decision

The Stay-alive process **is** the queue. It owns the canonical Bound queue,
its Queue source, and the owner-minted `QueueLineage` that authorizes
source-only updates; it persists that state on accepted queue-changing
operations and graceful shutdown — an empty queue persists as empty — and
reloads it at startup. Clients attached to it are readers: loading a
playlist (with or without playback), editing the queue, Save As, and clear
all go to the owner, and every Client displays only owner-accepted
snapshots, reconciling from the owner on reconnect. The owner-side gate for
this model is `DaemonRole::Local`: only the Stay-alive local daemon runs
this single-owner adoption path.

Packaged `mbvd` keeps its current behavior — it is not reinterpreted as
this model, and direct-remote and Bare mode are unchanged (a Client in
Bare mode owns its queue and persists it as before).

## Consequences

- All queue-changing operations are owner-accepted before a Client reports
  success; concurrent Clients converge on the latest accepted snapshot
  instead of resurrecting their own earlier queues.
- The canonical-queue-edits rule follows queue holdership, not merely the
  playback target: edits made while a session or cast is the playback
  target still reach the Stay-alive owner that holds the queue.
- Playback-run identity is owner-minted, so observations from a run
  retired by a queue replacement cannot mark new-queue slots as played.
- A known residual of the adoption model: on a plain autostart play, the
  owner's Queue source stays at its previous value until the next idle
  load or clear (see
  [Invariant 13](../invariants/13-stay-alive-owner-is-the-queue.md)).
