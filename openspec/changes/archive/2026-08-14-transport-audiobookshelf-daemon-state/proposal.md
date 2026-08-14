## Why

Roadmap milestone 4 (#524) needs a safe ctrl boundary before daemon owners can become eligible for Audiobookshelf playback. This first child (#525) adds additive, secret-free transport for provider-qualified queue and progress state while keeping all daemon Audiobookshelf admission disabled.

This change begins only after #523 and the completed bare-mode playback milestone #515 are present in the implementation baseline.

## What Changes

- Add additive Audiobookshelf queue and progress capability strings without changing `CTRL_PROTOCOL_VERSION`.
- Carry Audiobookshelf podcast `QueueItem` values through unified queue commands and state only when both peers support their transport.
- Add a provider-qualified acknowledged-progress event carrying episode identity, acknowledged position/completion, and setup generation.
- Gate incoming queue operations, initial snapshots, later queue broadcasts, and progress events per ctrl connection so older peers never receive unsupported Audiobookshelf variants.
- Keep API keys, Authorization headers, resolved URLs, and playback `sessionId` values outside ctrl messages, queue state, and logs.
- Keep every daemon Player owner ineligible for Audiobookshelf admission and playback until later #524 children install owner context and activate the lifecycle.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `unified-playback-queue`: Add separately negotiated Audiobookshelf item transport while preserving one canonical internal queue and disabled daemon admission.
- `ctrl-protocol`: Advertise Audiobookshelf queue/progress support and carry secret-free provider-qualified progress events compatibly.

## Impact

- Affects ctrl hello/compatibility state, per-connection daemon client metadata, unified queue serialization and filtering, initial state, queue broadcasts, remote Player compatibility, and dormant progress-event plumbing.
- Introduces no source resolution, owner credential loading, daemon playback, setup administration, or client browse reconciliation.
- Depends on #523 because packaged `mbvd` must first use the same Service-independent control/runtime foundation.
