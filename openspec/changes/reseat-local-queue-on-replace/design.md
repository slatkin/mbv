# Design

`PlayerStatus.sequence_generation` is serde-defaulted for wire compatibility. The Player owner increments it for every accepted canonical queue submission. A local `PlayerTab` records the generation of its snapshot; replacement deliberately fences it ahead of the owner's generation. Queue cursor play checks this fence before minting a slot-addressed jump. A mismatch follows the existing full `submit_queue_slots` path at the selected cursor, and does not move playback during populate-only replacement.

`PlayerTab::set_queue_items` replaces slots in the existing `PlaybackQueue`, preserving its monotonic allocator. Thus replacement cannot reuse an old occurrence's slot identity, including numeric collisions with the old run.

A missing slot in a local Playback run emits `CommandRejected`; the shell clears the bare optimistic transition immediately. The existing reconciliation and timeout paths remain unchanged.
