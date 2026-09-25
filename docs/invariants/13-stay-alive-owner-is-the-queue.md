# Invariant 13 — The Stay-alive process is the queue; Clients are readers

**Scope:** every path that loads, edits, saves, clears, or reconciles the
Stay-alive owner's queue — the owner side in `src/local_daemon.rs` /
`crates/mbvd`-adjacent local-daemon code and the Client side in
`src/app/shell/`, `src/app/dispatch/queue.rs`, and the
`dispatch/session/player_event.rs` adoption arms.

## The invariant

The Stay-alive process **is** the queue. It owns the canonical Bound queue,
its Queue source, and the lineage that authorizes queue-affecting updates;
it persists that state and reloads it at startup. A Client attached to it
is a reader only: it reconciles its displayed Local queue, source, and
status solely from owner snapshots and never holds an authoritative copy of
its own. Four properties no type enforces:

1. **No client-side seeding or persistence.** A Client SHALL NOT seed the
   Stay-alive queue from its own saved snapshot and SHALL NOT persist the
   owner's queue locally. An empty owner queue persists as empty — a
   reload after a clear restores nothing; there is no older-snapshot
   fallback on startup, and the legacy one-time takeover applies only when
   the owner's own persistence file is absent.
2. **Owner-minted lineage guards source-only updates and playlist
   mutations.** A `QueueLineage` minted by the owner accompanies each
   accepted queue state; a source-only update (Save As) applies only to
   the lineage the owner held when it was requested. Clients never
   self-authorize lineage: they echo owner-minted values, and a delayed
   update from an earlier queue is rejected rather than renaming a later
   one.
3. **Owner-minted playback run identity filters stale observations.** Each
   playback run carries an owner-assigned identity; stop/completion/track
   observations from a run that a queue replacement already retired are
   filtered by that identity so a late report cannot mark a new-queue slot
   as playing, completed, or consumed.
4. **Adoption, not merge.** A Client's reconcile step replaces its
   displayed state from the owner snapshot wholesale (including source and
   cursor arbitration); it never keeps a local fragment as authority
   because a generation happened to match.

## Why it matters

The pre-change model let each Client carry its own queue staging area: a
populate-only load updated the Client's Composed snapshot without the
owner, Client-side persistence raced the owner's file, and two attached
Clients could hold divergent "Local" queues while one owner played. The
single-owner model removes the entire class, but only if no path quietly
reintroduces a second authority.

## What breaks if it is violated

A Client that seeds the queue from its own snapshot resurrects a cleared
queue on attach; a Client that persists the owner's queue makes the
owner's restart state depend on which Client happened to exit last; a
self-authorized Save As renames a queue another Client just replaced; an
unfiltered late completion from a retired run consumes a slot in the new
queue. Each failure is invisible to the Client that caused it and loud in
every other attached terminal.

## How the code maintains it today

- Owner identity is typed, not inferred: `LocalQueueOwner`
  (`state/queue_owner.rs`), derived from `player_endpoint`, decides at each
  owner-sensitive site which side holds the authoritative Local queue; every
  connect/switch/teardown write to `player_endpoint` advances the
  Client-local `QueueEpoch`.
- Owner snapshots are authoritative for adoption: the Client adopts
  unfenced owner state including source on attach and after every
  accepted load/replace/edit/clear (`dispatch/session/player_event.rs` `UnifiedQueueUpdated`
  arm via `adopt_owner_source`), with the Bare-mode fence byte-identical to
  the pre-change behaviour so direct/bare scopes are untouched.
- All Client source writers route through one shared guard
  (`set_queue_source_if_not_local_daemon`): a Client attached to the
  Stay-alive owner cannot write a source of its own.
- Save As and Overwrite completions share one source-update path
  (`apply_saved_playlist_source`): under Stay-alive both send a
  lineage-guarded `UnifiedQueueSourceUpdate` carrying the `QueueOrigin`
  captured at request time, and the queue stays dirty until the owner's
  snapshot adopts the new source; a Stay-alive Client without an owner
  snapshot refuses playlist saves at request time.
- Owner persistence writes only what the owner accepted; the reload path
  restores exactly the persisted state, and an empty queue persists as
  empty.

## Where it currently fails

No known violation. Known residual (observed in review, not a regression):
on a plain autostart play, the owner's Queue source stays at its previous
value until the next idle load or clear — a property of the adoption
model, since submission does not carry a source update. Nothing
structural prevents a new code path from staging a local queue copy for a
Stay-alive Client; the guarantee lives in routing every queue- and
source-affecting write through the owner and adopting its snapshots, so
each new write path must repeat that routing deliberately.
