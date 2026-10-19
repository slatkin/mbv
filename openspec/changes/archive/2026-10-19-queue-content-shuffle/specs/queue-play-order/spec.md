# Spec Delta

## REMOVED Requirements

### Requirement: The Player owner holds the playback order
**Reason**: The owner no longer has a playback order separate from the queue order. Shuffle is now a one-time reorder of the queue (capability `queue-content-shuffle`).
**Migration**: None. A saved queue keeps its slot order. The owner is always in sequential order.

### Requirement: The status payload carries the playback order
**Reason**: There is no playback order to report. The `play_order` field leaves the status payload.
**Migration**: A peer that sends the field is ignored for that field. A peer that reads it before this change gets its serde default of Sequential.

### Requirement: Shuffle picks a random slot other than the active slot
**Reason**: Advance never picks a random slot. It goes to the next slot in the queue.
**Migration**: Use the Queue list Shuffle entry to reorder the queue once.

### Requirement: One pick drives every forward advance
**Reason**: With no pick, the end-of-file advance, Next, and the Next Up card all use the queue neighbor.
**Migration**: None.

### Requirement: Previous is disabled in Shuffle
**Reason**: Previous follows the queue order, which Shuffle has already changed. Nothing disables it.
**Migration**: None. MPRIS reports that the player can go to the previous item by the same rule as for any queue.

### Requirement: Replacing the queue resets the order
**Reason**: There is no order to reset.
**Migration**: None.

### Requirement: The Queue list menu toggles Shuffle
**Reason**: The toggle and its "Shuffle Play" label are replaced by the Shuffle entry of capability `queue-content-shuffle`.
**Migration**: See requirement "The Queue list menu offers Shuffle" in `queue-content-shuffle`.

### Requirement: The footer shows Shuffle
**Reason**: There is no playback order to show. The queue itself shows its shuffled order.
**Migration**: None.

### Requirement: MPRIS reports the shuffle state
**Reason**: MPRIS Shuffle no longer maps to an owner state. It reports false at all times.
**Migration**: None. A request from an MPRIS client to change Shuffle does not change the queue.
