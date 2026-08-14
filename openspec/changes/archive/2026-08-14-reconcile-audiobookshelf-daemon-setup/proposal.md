## Why

Roadmap milestone 4 (#524) needs daemon Player owners to use their own installed singleton Audiobookshelf setup. This second child (#526) establishes the owner context, administration, and reconciliation boundary that bare mode already has for Audiobookshelf — loading setup, API key, generation, and stable device identity without moving credentials over ctrl, and giving packaged `mbvd` supported local Audiobookshelf administration — while every daemon owner remains ineligible for Audiobookshelf playback.

This change begins only after #525 (`transport-audiobookshelf-daemon-state`) has landed so the ctrl seam it built is present.

## What Changes

- Load owner-scoped Audiobookshelf setup, API key, setup generation, and stable device identity for bare, Local daemon, and packaged `mbvd` owners, without transporting credentials through ctrl.
- Establish one persisted-source-of-truth reconciliation boundary: committed owner state is reconciled by signaling what changed and making the owner reread its own storage. Bare mode invokes the same semantic operation directly; Local daemon and packaged owners reread on an owner-scoped ctrl signal.
- Apply successful mbv Services setup, repair, replacement, and removal to a running same-user Local daemon when possible.
- Add `mbvd --connect abs` with bounded `GET /api/me` validation before any transactional setup/secret commit; failed candidates preserve working state.
- Add `mbvd --disconnect abs` with no confirmation prompt and explicit durable credential-removal reporting.
- Preserve a validated durable commit when live reconciliation fails, reporting restart required; on failed disconnect reconciliation, report that the running process may retain the deleted key in memory.
- Advance owner generation and implement replacement/removal cleanup of Audiobookshelf-owned state without enabling daemon Audiobookshelf playback.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `audiobookshelf-service-setup`: Add owner-scoped setup/secret loading for daemon owners, a persisted setup revision and owner-side generation advance, stable device identity, and a reconciliation boundary that applies to bare, Local daemon, and packaged owners.
- `mbvd-service-administration`: Add `mbvd --connect abs` and `mbvd --disconnect abs` owner-local administration with `GET /api/me` validation and durable credential-removal semantics.
- `packaged-daemon-service-runtime`: Packaged `mbvd` loads owner-local Audiobookshelf setup/secret as an optional runtime (no playback) and applies `ApplyServiceSetup` for Audiobookshelf.
- `ctrl-protocol`: The owner-service reconciliation command carries `ServiceKind::Audiobookshelf` and a same-user Local daemon accepts the reread signal from its attached client, while packaged and TCP boundaries stay unchanged.

## Impact

- Affects Audiobookshelf setup lifecycle persistence (a persisted `revision`), daemon startup owner context, bare-mode Services settings applying changes to a running Local daemon, packaged `mbvd` administration, ctrl `ApplyServiceSetup` handling, and Audiobookshelf-owned state cleanup.
- Introduces no daemon Audiobookshelf admission, source preparation, stream resolution, progress generation, or stay-alive playback.
- Depends on #525 for the ctrl capability/progress seam and on #523 for the Service-independent packaged-daemon foundation.
