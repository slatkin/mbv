## Why

After provider-native queueing and safe active-source projection exist, milestone #515 still needs a complete Audiobookshelf reporting lifecycle and explicit episode actions. This final child enables bare-mode playback only after progress accounting, finalization, and Service invalidation are correct.

## What Changes

- Require #517's verified playback API, runtime context, prepared-source boundary, and owner-driven projection as the implementation baseline.
- Enable Audiobookshelf admission only for the in-process bare-mode Player while it has a current playback context.
- Add an active-item lifecycle strategy for Emby, Audiobookshelf, and items with no server reporting.
- Synchronize position, duration, and actual monotonic wall-clock listening time periodically and on pause/seek.
- Finalize every opened Audiobookshelf session in order on completion, transition, failure, Service invalidation, and teardown.
- Emit generation-safe provider-qualified progress for matching queue and browse state without Socket.IO.
- Replace inert downloaded-episode activation with provider-specific ordinary play and enqueue actions through #513's browse handler.
- Keep credentials and ephemeral playback state inside the in-process Player owner.
- Keep Audiobookshelf ctrl transport, Local daemon playback, remote routing, Socket.IO, and audiobook playback out of scope.

## Capabilities

### New Capabilities

- `audiobookshelf-podcast-playback`: Bare-mode owner eligibility, progress synchronization, ordered session finalization, local progress reconciliation, and explicit downloaded-episode play/enqueue behavior.

### Modified Capabilities

- `audiobookshelf-podcast-browsing`: Replace inert downloaded-episode activation with provider-native play and enqueue submission while browsing remains free of credentials and playback lifecycle state.
- `unified-playback-queue`: Enable the prepared Audiobookshelf source to use Audiobookshelf reporting at the existing item-kind reporting boundary.

## Impact

- PlaybackRun reporting lifecycle, progress events, finalization, and Service invalidation handling in `mbv-core`.
- In-process owner capability, Audiobookshelf episode actions, queue/browse progress reconciliation, and teardown in `src/app`.
- No dependency, ctrl capability, protocol version, daemon credential handling, Socket.IO connection, or audiobook model.
