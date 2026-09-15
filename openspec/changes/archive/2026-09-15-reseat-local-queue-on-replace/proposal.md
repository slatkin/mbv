# Reseat local queue on replacement

Local queue replacement while a Playback run is active must not leave the run and Client addressing different queue occurrences. Explicit play of a row in the replacement submits the canonical queue at that row; populate-only replacement does not claim the new queue is playing.

## Scope
- Fence Client slot commands with a serialized owner sequence generation.
- Keep local slot allocation monotonic across replacement.
- Reject stale local jumps visibly and clear optimistic playhead state.
