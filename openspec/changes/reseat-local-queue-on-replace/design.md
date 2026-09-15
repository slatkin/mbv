# Design

`PlayerStatus.sequence_generation` is serde-defaulted for wire compatibility. The Player owner increments it for every accepted canonical queue submission. A local `PlayerTab` records the generation of its snapshot; replacement deliberately fences it ahead of the owner's generation. Queue cursor play checks this fence before minting a slot-addressed jump. A mismatch follows the existing full `submit_queue_slots` path at the selected cursor, and does not move playback during populate-only replacement.

`PlayerTab::set_queue_items` replaces slots in the existing `PlaybackQueue`, preserving its monotonic allocator. Thus replacement cannot reuse an old occurrence's slot identity, including numeric collisions with the old run.

A missing slot in a local Playback run emits `CommandRejected`; because that event has no request identity, the shell clears only a queued (not yet confirmable) bare transition and preserves the in-flight transition. The existing reconciliation and timeout paths remain unchanged.

## Scenarios

### Generation-fenced jumps upgrade to canonical submission

- **Given** an active local Playback run and a replacement queue whose generation is
  ahead of the owner's generation
- **When** the Client plays a row in that replacement
- **Then** it submits the tab's canonical slot pairs at the selected index rather
  than sending a slot-addressed jump into the old run

### Replacement preserves monotonic client-minted ids per tab

- **Given** a tab that has previously allocated queue slots
- **When** its queue is replaced
- **Then** the replacement slots have fresh, monotonically increasing identities,
  even when the old run used the same numeric ids for different content

### Now-playing requires owner confirmation

- **Given** a replacement queue with an optimistic selection
- **When** the owner has not confirmed a slot, or rejects/expiry discards it
- **Then** no queue row claims now-playing status and no rejected/expired ghost row remains

### Rejection clears only the rejected, unconfirmable transition

- **Given** an in-flight optimistic transition and a newer queued transition
- **When** an identity-less `CommandRejected` arrives
- **Then** only the queued transition is cleared; the in-flight transition remains
  available for owner confirmation
