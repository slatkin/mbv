## Context

See `proposal.md` for motivation. #492 added `FeedEntryState`, capability-gated
get/put/scan operations on `SharedClient`, and a daemon-hosted redb table keyed
by `(user_id, feed_id, entry_guid)`. Playback currently has no caller for those
operations: `FeedEntry` lacks feed identity and progress fields, queue progress
is a no-op for Feed slots, and Feed resume is hardcoded to zero. The App already
receives authoritative stop, completion, pause, and output-restart events from
both local and attached Player owners.

The stored subscription URL is immutable: changing a URL creates a new
subscription. That makes the normalized stored URL the stable `feed_id` needed
by #492 without adding another persisted subscription identifier.

## Goals / Non-Goals

**Goals:**

- Carry enough identity and progress in each queued FeedEntry to hydrate it from
  and write it back to #492's keyed state store.
- Make lifecycle writes event-driven and infrequent while preserving state
  across mixed queues and ctrl snapshots.
- Use one integer-safe resume predicate for Emby and Feed items.
- Keep storage absence and failures outside the playback-success path.

**Non-Goals:**

- Periodic progress checkpoints, a watched-filter UI, or entry-state prefix-scan
  consumption; the scan remains for #494.
- Emby progress/session reporting for Feed entries.
- A new protocol version, storage table, dependency, or feed-subscription ID.

## Decisions

### Use the normalized subscription URL as `feed_id`

Add an optional feed identity to FeedEntry and set it from the subscription's
stored, normalized URL when fetched entries are associated with that
subscription. The URL is already the domain identity boundary: edits preserve
it, while a URL change creates a new subscription. Entries copied into the All
group or a queue retain it.

The field is optional and serde-defaulted so old queue files and ctrl payloads
remain readable. A legacy entry without `feed_id` plays statelessly. Generating
a second opaque ID was rejected because it would require a subscription-config
migration and mapping logic without improving identity stability.

### Put Feed progress on FeedEntry and teach queue progress about both variants

Add serde-defaulted `position_ticks` and `played` fields to FeedEntry. Update
QueueItem's progress accessors and the queue's SlotProgress construction/apply
path to branch on both variants. This keeps the queue's existing progress
machinery authoritative while preserving the tagged queue and ctrl shape.

A separate App-only state map was rejected because queued snapshots must retain
state and identity after subscription deletion, queue transfer, and mixed-queue
transitions.

### Hydrate the entry at the explicit play boundary

Immediately before a FeedEntry is submitted for playback, use
`SharedClient::get_feed_entry(feed_id, guid)` when the capability and identity
are available, then copy the returned position/played state into the submitted
entry. Missing state, missing identity, unsupported capability, disconnection,
or read failure leaves the entry stateless and does not reject playback.

Reading at the play boundary, rather than bulk-scanning every feed refresh,
keeps D2 independent of #494's watched-filter hydration and prevents background
browsing from becoming a prerequisite for resume.

### Persist from existing lifecycle boundaries

Use App event handling as the storage boundary because it owns both the
canonical queue slot and `SharedClient`:

- `Stopped` stores the final non-completed position or completed state.
- `TrackCompleted` stores EOF completion before any consume mutation removes the
  slot.
- `PausedChanged(true)` snapshots the current PlayerStatus position.
- A confirmed seek is written from the output-restart boundary only when the
  App has a pending seek action, preventing ordinary startup or buffering
  restarts from becoming writes.

Centralize these paths in one helper that resolves the active slot, verifies it
is an addressable FeedEntry, derives completion, updates queue progress, and
calls the #492 put operation. Failures are logged and discarded. There is no
time-position or periodic writer.

### Define completion independently from resume

For a known runtime, EOF marks played, and a stop position at or above 95% marks
played. Played state is persisted with position zero, matching replay-from-start
behavior. Unknown-runtime EOF does not infer played because no reliable
completion boundary exists. Pause and seek preserve the current played value
unless a qualifying completion event occurs.

### Extract one shared 6% resume predicate

Move resume eligibility out of `EmbyItem::should_resume` into a shared helper
with a named 6% constant. Compare using a widened integer representation to
avoid multiplication overflow. Emby delegates to it; QueueItem/Feed resume uses
the same helper. Positive unknown-runtime positions remain resumable, while
zero and negative positions do not.

The existing `raise-playback-resume-threshold` change is superseded and should
be removed with this implementation so two active plans do not claim the same
requirement.

## Risks / Trade-offs

- **Synchronous state operations can briefly delay an event handler** → Keep
  operations limited to explicit lifecycle events and treat timeout/failure as
  non-fatal; do not add periodic calls.
- **Feed URL identity changes if a subscription is replaced** → This matches the
  existing rule that a URL change creates a new subscription and therefore new
  state identity.
- **Seek writes could be triggered by ordinary output restarts** → Gate the
  output-restart write on an explicit pending seek marker and clear it after one
  confirmed restart.
- **Old peers omit new FeedEntry fields** → Use additive serde-defaulted fields;
  omitted identity/state produces stateless playback rather than rejection.
- **A client may close while a Local daemon keeps playing** → Persistence is
  guaranteed only at lifecycle events observed by an attached App in this
  change; adding a Player-owner-resident state client would be a separate
  architecture change.

## Migration Plan

No redb or config migration is needed. Existing FeedEntry payloads deserialize
with no identity, zero position, and unplayed state. New payloads remain within
the existing Feed-capable ctrl path. Remove the superseded
`raise-playback-resume-threshold` change when this implementation and its specs
land; close #438 as absorbed after verification. Rollback leaves already stored
#492 rows intact and simply returns Feed playback to stateless behavior.
