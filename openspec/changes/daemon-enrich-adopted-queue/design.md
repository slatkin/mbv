## Context

See proposal.md - Why for the root cause. Relevant existing pieces this
design reuses rather than reinvents:

- `handle_ctrl` (`crates/mbv-core/src/daemon_control.rs:48-61`), which
  handles `CtrlCmd::UnifiedAdoptQueue`, already receives both
  `client: &Arc<Mutex<EmbyClient>>` and `merged_tx: &mpsc::Sender<DaemonEvent>`
  — everything a spawned background fetch needs to report its result back to
  the event loop.
- `PlaybackQueue::merge_refresh` (`crates/mbv-core/src/playback/queue.rs:543`)
  already implements "merge a freshly-fetched Emby item list into an existing
  canonical queue, protecting the active/pending-sync slot and pruning/
  ignoring non-Emby slots" — exactly what this change needs. It already has
  callers on the client side (`merge_refreshed_queue`,
  `src/app/queue_scope.rs:176`); nothing on the daemon side calls it today.
- The existing `DaemonEvent::AudiobookshelfProgress(AudiobookshelfProgressUpdate)`
  variant (`daemon_core.rs:56`, handled `daemon_run.rs:597`) is the established
  pattern for "an async result from outside the event loop rejoins it as a
  typed event, gets applied to the canonical queue, and gets broadcast" — this
  change follows the same shape for an Emby refresh result instead of an
  Audiobookshelf progress push.
- The client's `spawn_enrich_queue_state` (`src/app/queue_actions_playlist_mutation.rs:304`)
  is the existing analogous mechanism for the plain-local (Bare-mode)
  restore path; this change gives the daemon-adoption path its own copy of
  the same idea rather than trying to route the client's copy through ctrl.

## Goals / Non-Goals

**Goals:**

- A cold daemon's canonical queue reflects real Emby-side progress shortly
  after adopting a persisted snapshot, without requiring playback first.
- Reuse `merge_refresh`'s existing safety properties (active-slot and
  pending-sync protection, non-Emby slots left alone) rather than
  reimplementing them daemon-side.
- No `CTRL_PROTOCOL_VERSION` bump, no new wire command, no change to
  `UnifiedAdoptQueue`'s existing shape.

**Non-Goals:**

- Periodic/ongoing refresh of a queue that's already live and playing (this
  change is adoption-time only; an already-running daemon's queue is kept
  correct by the existing play-driven progress-application path).
- Audiobookshelf or Feed queues: `merge_refresh` already scopes a refresh to
  the Service it queried and leaves other kinds' slots untouched (see the new
  spec's third scenario); this change adds no new per-kind branching.
- Preserving the client-side "saved positions" override for the
  daemon-adoption path (see Decisions below).

## Decisions

### Where the fetch lives: daemon-spawned, not client-supplied

The daemon spawns its own background thread (using the `EmbyClient` it
already holds) from inside the `UnifiedAdoptQueue` handler, rather than
having the client fetch-and-send refreshed items over ctrl.

**Alternative considered**: have the client perform the fetch (as
`spawn_enrich_queue_state` already does) and send the refreshed items to the
daemon via a new or extended ctrl command. Rejected: it requires a wire-shape
change reviewed against `CTRL_PROTOCOL_VERSION` compatibility rules
(`unified-playback-queue`'s existing capability-negotiation requirements),
and it duplicates a fetch the daemon can make itself with credentials it
already has — the daemon is the one that needs the answer, so it should be
the one that asks.

### New `DaemonEvent` variant, not a return value from `handle_ctrl`

`UnifiedAdoptQueue`'s handling stays synchronous and returns immediately
after admitting the snapshot (as it already does); the refresh's arrival
later is a new `DaemonEvent` (e.g. `DaemonEvent::QueueEnriched(Vec<EmbyItem>)`)
processed by the main event loop in `daemon_run.rs`, mirroring
`DaemonEvent::AudiobookshelfProgress`'s existing shape.

**Alternative considered**: block `UnifiedAdoptQueue` on the fetch. Rejected:
it would delay the daemon starting playback of the adopted queue on a
network round-trip, and the queue is already fully playable with its
persisted (if stale) progress in the meantime — exactly the property
`QueueState.items` storing full items (not just IDs) exists to guarantee
(see the comment at `crates/mbv-core/src/config_types_queue_state.rs:26-29`).

### Client no longer independently enriches after daemon adoption

`spawn_enrich_queue_state`'s call sites in `src/app/daemon_restart.rs` and
`src/app/construct.rs` (both specifically the daemon-adoption path) are
removed. The client's local mirror will receive the daemon's own refreshed
broadcast once the daemon's fetch completes, so a second, race-prone client
fetch against the same data adds nothing but the exact clobbering risk this
change closes (see proposal.md - Why). The plain-local restore path
(`restore_queue_state`, no Player owner involved) is untouched — it has no
daemon queue to defer to.

### Drop the "saved positions" override for the daemon-adoption path

> **REVISED 2026-09-19 after the 4.2 live test (see the follow-up decision
> below).** The original decision below was accepted on the assumption that
> Emby returns "slightly stale-but-real" data during the Stopped-report race.
> The live test falsified that: the fetch succeeded and returned zero UserData
> for adopted items, and the daemon-side merge verbatim-overwrote the good
> adopted ticks, making the refresh strictly worse than no refresh. The
> override's *semantics* (max of locally-saved and fetched) are restored in
> section 5, without threading the map through the wire.

`QueueState.positions` exists to override Emby's UserData with a locally-
saved position when Emby's write of a just-sent `Stopped` report may not
have landed yet (a few-seconds race, documented at
`crates/mbv-core/src/config_types_queue_state.rs:40-42`). Today only the
client-side `spawn_enrich_queue_state` applies this override
(`src/app/queue_actions_playlist_mutation.rs:329-341`); moving enrichment to
the daemon means either threading `positions` through `UnifiedAdoptQueue`'s
wire shape, or accepting Emby's response as-is without the override for this
path.

This design accepts the latter. The failure mode without the override is
strictly better than today's behavior (no refresh at all until played): in
the rare case the daemon's refresh races a just-sent `Stopped` report, the
adopted item briefly shows a slightly stale-but-real position from Emby
rather than the fully-fabricated zero it would otherwise show, and it
self-corrects on the next natural queue broadcast (or immediately on play).
Threading `positions` through the wire was rejected as disproportionate
complexity (a `UnifiedAdoptQueue` shape change plus its own compatibility
handling) for closing a narrow, already-self-correcting race.

### Documentation

`docs/invariants/06-queue-progress-application-sites.md` gets a note once
this ships: it currently documents three queue copies and their progress
sync points; this adds a fourth trigger (cold-adopt refresh) to the
canonical daemon queue, worth cross-referencing from the same invariant doc
rather than leaving it undiscovered next to the sites it already covers.

## Risks / Trade-offs

- **[Risk]** A large adopted queue means `get_items_by_ids` fetches many
  items in one request right after daemon startup. → Mitigation: this is the
  same call and item-count shape `spawn_enrich_queue_state` already makes
  client-side today; no new scaling behavior is introduced, only a change of
  which process makes the call.
- **[Risk]** Removing the client's post-adoption enrichment call means a
  client whose daemon-side fetch fails (network error, Emby unreachable) has
  no fallback enrichment attempt for that session. → Mitigation: `merge_refresh`
  already treats a failed fetch as a no-op today (the client's version logs a
  warning and returns without mutating anything, per
  `src/app/queue_actions_playlist_mutation.rs:322-328`); daemon-side, this
  change mirrors the same fail-soft behavior — the queue simply stays at its
  persisted values, which is exactly today's status quo for that failure
  case, not a regression.
- **[Trade-off]** Dropping the positions-override protection for the
  daemon-adoption path (see Decisions) accepts a narrow, self-correcting
  staleness window in exchange for no wire-protocol change. **Superseded
  2026-09-19**: the live test showed the window is not self-correcting and
  not narrow — a successful fetch returning zero/stale UserData permanently
  zeroes adopted positions for the session. Reverted by the monotonic-max
  decision below.

### Follow-up decision (2026-09-19): monotonic max overlay, no wire change

The live 4.2 test failed with "no change in behaviour": cold adopt installed
items carrying real ticks, then the enrichment fetch succeeded with zero/stale
server UserData and `merge_fetched_slot`'s non-active arm verbatim-overwrote
the good ticks; every later broadcast distributed the zeros. Three corrections,
all without touching the ctrl wire shape:

1. **Monotonic enrichment merge (restores the override's semantics).** The
   non-active merge arm never lowers a slot's stored `playback_position_ticks`
   during adoption-time enrichment: effective position is
   `max(fetched, stored)`. This is exactly what the removed client-side
   `spawn_enrich_queue_state` path did (its saved-positions overlay applied
   before merging). A genuinely newer server position still wins whenever it
   exceeds the stored value; the only loss case — Emby lagging a just-sent
   Stopped report — now keeps the adopted value, and playback still
   self-corrects via the play-driven path. The `QueueState.positions` map
   itself stays out of the wire: the daemon's adopted items already carry the
   saved ticks inside `slot.item`, so protecting them at merge time is
   equivalent to the old overlay without a protocol change.
2. **Played no longer suppresses resume percentage (user ruling).**
   `MediaSemanticState::from_progress`'s `played`-trumps-position rule was
   unintended migration fallout: it hid real progress on every played item
   across queue, library, home, tv, and music rows. New derivation: a
   positive position always yields the resume percentage; the played style
   applies only to played items with no position.
3. **`get_continue_watching` requests UserData.** Its sibling queries include
   `EnableUserData=true`; if the Resume fetch does not, Home rows parse
   without any UserData (position and played both lost). Align it with the
   siblings and cover with a test asserting the query parameter.

## Migration Plan

Additive: a new internal event variant and a new call inside an existing
handler; no persisted-data shape change, no `CTRL_PROTOCOL_VERSION` bump, no
client/daemon version compatibility concern (an older client talking to a
newer daemon still sends the same `UnifiedAdoptQueue`; an older daemon
talking to a newer client simply never performs the new refresh, which is
today's existing behavior). No rollback beyond reverting the commit.
