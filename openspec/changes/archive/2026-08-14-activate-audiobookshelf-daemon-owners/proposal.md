## Why

Roadmap milestone 4 (#524) needs daemon Player owners to play Audiobookshelf podcasts through stay-alive Local daemons and packaged `mbvd`. The first two children established the ctrl seam (#525) and the owner setup/reconciliation boundary (#526). This third child (#527) activates the full playback lifecycle — admission, source preparation, authoritative progress, and finalization — for owners with installed Audiobookshelf setup, while the final child (#528) closes the client-side loop.

## What Changes

- Admit Audiobookshelf podcast episodes to daemon Bound queues when the owner has installed Audiobookshelf setup and has negotiated transport capability with a capable attached client; reject otherwise with a visible failure.
- Reuse the existing bare-mode just-in-time direct/HLS source preparation, authoritative resume, Bearer isolation, active-file projection, and listening-time accounting, routed through the daemon owner's mpv projection.
- Perform periodic progress synchronization from the daemon owner using established bare-mode sync logic, updating the canonical daemon queue by provider-qualified identity.
- Broadcast acknowledged provider-qualified progress to capable attached clients at the daemon owner's post-sync acknowledgement point, reusing the `broadcast_audiobookshelf_progress` seam from #525.
- Apply bounded finalization on active Audiobookshelf playback: natural completion, explicit stop/skip, setup replacement/removal, and credential rejection all finalize within the teardown budget.
- Preserve installed Audiobookshelf setup and API key after a playback or progress failure; fail the request visibly and allow retry on the next explicit play.
- Finalize active Audiobookshelf playback and purge old-server slots on setup replacement.
- On `mbvd --disconnect abs`, finalize active Audiobookshelf playback, stop the entire queue, and purge Audiobookshelf Bound and persisted slots (reusing the disconnect cleanup path from #526).

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audiobookshelf-podcast-playback`: Extend eligible owners beyond bare mode to Local daemon and packaged `mbvd` owners with installed Audiobookshelf setup; reuse source preparation, authoritative resume, progress synchronization, and bounded finalization; add daemon-queue update by provider-qualified identity at the post-sync acknowledgement point.
- `unified-playback-queue`: Daemon Bound queues now admit Audiobookshelf `QueueItem` variants when the owner has installed setup and has negotiated transport capability.
- `local-daemon-stay-alive`: Audiobookshelf playback and progress synchronization continue after every terminal client exits; a later client adopts the live queue, status, and acknowledged progress.

## Impact

- Affects daemon owner playback admission, source preparation routing, progress synchronization, bounded finalization, setup-replacement cleanup, and disconnect handling.
- The `broadcast_audiobookshelf_progress` seam (#525) and `ApplyServiceSetup` reconciliation (#526) are the interfaces this change activates; both exist but are dormant until this change.
- Does not add Socket.IO, audiobook playback, multiple Audiobookshelf servers, per-client credentials, credential transport over ctrl, or attached-client browse/continuity reconciliation (those are #528).
- Depends on #526 for owner setup/generation context and on #525 for the ctrl transport seam.
