# Proposal

## Why

Stay-alive Clients currently display a locally replaced playlist before the Stay-alive process accepts it. The process can continue playing its previous queue while the Client shows a different one; later owner broadcasts, edits, and now-playing coordinates then disagree. A Stay-alive Client must never have a private queue masquerading as the owner's queue.

## What Changes

- **BREAKING**: Loading a playlist into the Stay-alive queue without starting playback immediately stops the current item and replaces the owner's queue with that playlist, in a stopped state. The load does not wait for a later Play to submit the queue.
- The Stay-alive process is the queue. It holds, persists, and reloads its own queue and source; an empty queue persists as empty. A Client never seeds it from a saved snapshot and never persists it.
- Every attached Client displays the owner's accepted queue, source, and playback state in every state, including while an attached Session or cast receiver is the playback target; a failed or disconnected load does not become a private displayed queue. An attached Session or cast is a playback target, not a queue owner.
- The owner owns the queue source on every change — load, play/replace, clear, Save As — and Clients read it from owner snapshots; a delayed Save As cannot rename a later queue.
- The owner keeps playback and queue membership coherent: no playing item remains outside its Bound queue, even across rapid loads, edits, reconnects, and late playback observations.
- Bare-mode Composed queues, Direct remote control of another daemon, and packaged `mbvd` behavior are not redefined by this Stay-alive policy.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: Restrict populate-only Composed replacements to modes without a Stay-alive owner; make replacement, source ownership, and playback state coherent at that owner; scope canonical-queue edits to the owner that holds the queue.
- `local-daemon-thin-client`: Require every Stay-alive Client to reflect the accepted owner queue in every state, and move queue persistence and seeding from Clients to the owner.

## Impact

The Stay-alive ctrl queue command and owner handler (`crates/mbv-core/src/ctrl.rs`, `daemon_control.rs`), playback stop/replace lifecycle, owner queue persistence and reload, TUI queue loading and owner-snapshot handling (`src/app/queue_actions.rs`, `queue_scope.rs`, `player_event.rs`), removal of Client-side queue seeding and persistence for the Stay-alive owner (`src/app/bootstrap.rs`, `construct.rs`, `daemon_restart.rs`), and mock-based multi-Client tests. No remote Service API response is required for this change; playlist contents are already fetched by existing flows before the owner command. No new dependency or UI surface is proposed.
