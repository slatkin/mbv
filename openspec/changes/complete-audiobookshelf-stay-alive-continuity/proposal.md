## Why

Roadmap milestone 4 (#524) is complete on the owner side once daemon Player owners play Audiobookshelf podcasts and sync progress (#527), but attached clients still ignore what the owner reports: the client `PlayerEvent::AudiobookshelfProgress` handler is a dormant stub and the owner's `broadcast_audiobookshelf_progress` helper is dead code. This fourth and final child (#528) closes the client-facing loop — clients reconcile daemon-owned acknowledged progress into their queue and Audiobookshelf browse state, adopt the live daemon queue on attach, and the whole milestone is proved across every stay-alive client exit and later attachment.

This change begins only after #527 (`activate-audiobookshelf-daemon-owners`) has landed so daemon owner playback and authoritative progress exist to reconcile against.

## What Changes

- Activate owner emission of the provider-qualified acknowledged-progress event at the daemon owner's post-sync acknowledgement point, reusing the existing capability-gated `broadcast_audiobookshelf_progress` plumbing (#525).
- Replace the dormant client `PlayerEvent::AudiobookshelfProgress` stub with reconciliation that reuses the existing bare-mode apply path (match queue slots by `(library_item_id, episode_id)`, apply position/completion, update every browse state's progress map), gated on the current setup generation via `audiobookshelf_runtime.accepts`.
- Apply acknowledged daemon progress to Audiobookshelf browse state and its episode filters (e.g. Unplayed) only for the current setup generation; drop stale-generation acknowledgements.
- Let a later capable client adopt the live daemon queue, active slot, status, and last-acknowledged Audiobookshelf progress on attach, without overwriting daemon authority from a saved local/shared snapshot.
- Preserve coherent behavior with multiple attached clients and mixed-version peers: only capability-negotiated clients receive Audiobookshelf progress; every peer keeps its existing state stream.
- Verify direct/HLS playback, resume, pause, seek, completion, no-client operation, reattachment, and explicit daemon shutdown for daemon-owned Audiobookshelf podcasts.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audiobookshelf-podcast-playback`: The daemon owner emits acknowledged provider-qualified progress and attached clients reconcile it into their canonical queue; playback, progress synchronization, and session finalization continue after every Local-daemon client exits.
- `audiobookshelf-podcast-browsing`: Acknowledged daemon progress updates client browse progress and episode filters for the current setup generation; stale-generation acknowledgements are ignored.
- `unified-playback-queue`: A later capable client adopts the live daemon Audiobookshelf queue, active slot, and acknowledged progress on attach without overwriting daemon authority from a persisted snapshot.
- `ctrl-protocol`: The previously-dormant Audiobookshelf progress event is emitted by active daemon owner playback and consumed by capable clients, gated per connection by capability and by setup generation; incapable and mixed-version peers are unaffected.

## Impact

- Affects the client `PlayerEvent::AudiobookshelfProgress` handler, the shared acknowledged-progress apply path (`handle_lib_event` reconcile reused for the daemon route), the daemon owner acknowledged-progress emission seam (`broadcast_audiobookshelf_progress`), live-queue adoption of Audiobookshelf items on attach, and Audiobookshelf browse progress/filter reconciliation.
- Introduces no Socket.IO live refresh, audiobook playback, multiple Audiobookshelf servers, per-client credentials, credential transport over ctrl, or cross-provider browsing.
- Depends on #527 for daemon owner Audiobookshelf playback and authoritative progress, on #526 for owner setup/generation context, and on #525 for the capability-gated queue/progress ctrl seam.
