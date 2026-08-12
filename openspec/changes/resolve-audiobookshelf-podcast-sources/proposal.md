## Why

Audiobookshelf opens a server playback session for the active episode and may return direct authenticated audio or session-scoped HLS. The Player therefore needs a verified just-in-time source boundary and a one-file mpv projection before user-facing activation or progress reporting can be enabled safely.

## What Changes

- Require #516's QueueItem, Service-capability admission model, persistence, and cleanup as the implementation baseline.
- Capture and validate the Audiobookshelf 2.36 direct, forced-transcode, sync, close, and failure contracts before decoder or Player work.
- Add bounded authenticated playback-session API types and methods plus one stable local mbv device identifier.
- Give the in-process Player a runtime-only, generation-tagged Audiobookshelf context without advertising playback support yet.
- Prepare only the active queue slot, including source URL, per-file options, authoritative resume position, and optional lifecycle state.
- Scope Bearer authentication to a direct Audiobookshelf file and keep it off HLS, Emby, and Feed requests.
- Add owner-driven projection in which the canonical queue retains all slots while mpv contains exactly the active materialized file.
- Define canonical handling of selection, mutations, advance, and mpv playlist observations in owner-driven mode.
- Close opened sessions on preparation, load, and source-transition failure; defer full progress synchronization and finalization policy to #518.

## Capabilities

### New Capabilities

- `audiobookshelf-podcast-source-resolution`: Verified playback-session decoding, direct/HLS preparation, authoritative resume, header isolation, and owner-driven active-file projection.

### Modified Capabilities

- `unified-playback-queue`: Permit lifecycle-backed queues to project only the active canonical slot into mpv without changing canonical queue coordinates or operations.

## Impact

- Audiobookshelf playback API types and bounded requests in `mbv-core`.
- Player runtime context, source preparation, mpv per-file options, queue command/event adaptation, and minimal opened-session cleanup.
- No user-facing Audiobookshelf play/enqueue activation, periodic progress reporting, Socket.IO, ctrl transport, daemon support, or audiobook model.
