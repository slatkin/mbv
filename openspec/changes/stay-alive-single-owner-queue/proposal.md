# Proposal

## Why

Stay-alive Clients currently display a locally replaced playlist before the Stay-alive process accepts it. The process can continue playing its previous queue while the Client shows a different one; later owner broadcasts, edits, and now-playing coordinates then disagree. A Stay-alive Client must never have a private queue masquerading as the owner's queue.

## What Changes

- **BREAKING**: Loading a playlist into the Stay-alive queue without starting playback immediately stops the current item and replaces the owner's queue with that playlist, in a stopped state. The load does not wait for a later Play to submit the queue.
- Every attached Client displays the owner's accepted queue, source, and playback state; a failed or disconnected load does not become a private displayed queue.
- The owner keeps playback and queue membership coherent: no playing item remains outside its Bound queue, even across rapid loads, edits, reconnects, and late playback observations.
- Bare-mode Composed queues and the separate Local/Remote queue scopes used for Direct remote control remain distinct; packaged `mbvd` behavior is not silently redefined by this Stay-alive policy.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: Restrict populate-only Composed replacements to modes without a Stay-alive owner; make replacement and playback state coherent at that owner.
- `local-daemon-thin-client`: Require every Stay-alive Client to reflect the same accepted owner queue on load, attach, and subsequent edits.

## Impact

The Stay-alive ctrl queue command and owner handler (`crates/mbv-core/src/ctrl.rs`, `daemon_control.rs`), playback stop/replace lifecycle, TUI queue loading and owner-snapshot handling (`src/app/queue_actions.rs`, `queue_scope.rs`, `player_event.rs`), queue source/persistence, and mock-based multi-Client tests. No remote Service API response is required for this change; playlist contents are already fetched by existing flows before the owner command. No new dependency or UI surface is proposed.
