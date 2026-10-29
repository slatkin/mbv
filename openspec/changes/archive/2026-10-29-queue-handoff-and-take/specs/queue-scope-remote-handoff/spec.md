# Spec Delta

## REMOVED Requirements

### Requirement: Playing the queue cursor switches to the destination queue scope

**Reason**: The user-selectable Local/Remote queue scope no longer exists. The queue panel always shows the queue of the connected Player owner, so a play from the queue cursor already acts on the shown queue and nothing needs to switch.

**Migration**: To move playback to a peer, use Hand off (`queue-handoff`). A play from the queue cursor while connected plays on the connected peer, as before.
