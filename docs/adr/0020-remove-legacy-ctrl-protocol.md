# Remove Legacy Ctrl Protocol Shapes

## Decision

Delete the legacy ctrl wire shapes — `CtrlEvent::State`/`CtrlState`, the
`CtrlCmd::PlayItems`/`AdoptQueue` variants, and `WireCommand::LoadFeed` —
along with their split-item projections (`split_queue_for_legacy`,
`legacy_cursor`, `has_feed_entries`). Serialize only the unified queue shape
(`UnifiedQueueStateData`, `UnifiedQueueReplace`, `UnifiedAdoptQueue`, …).
The ctrl protocol version is not bumped.

## Context

The ctrl handshake is exact-match on protocol version; `CtrlCompatibility`
rejects every peer that is not exactly v9, and every v9 peer advertises
`unified-queue`. The legacy branches that emit `CtrlState` or accept
`PlayItems`/`AdoptQueue`/`LoadFeed` are therefore unreachable in practice:
no current peer takes them, and a peer old enough to need them cannot pass
the version guard. mbv is a single-user terminal client with no third-party
ctrl clients, so there is no external compatibility surface to preserve. The
legacy path is a second, permanently-unexercised implementation of queue
serialization and mutation.

## Considered Options

- Keep the legacy shapes as wire-compat scaffolding for hypothetical older or
  third-party clients.
- Delete the legacy shapes and serialize one unified shape (chosen).

## Consequences

Removing wire variants changes no reachable behavior: a peer that could decode
them can no longer connect in the first place. The protocol version is not
bumped because the version guard already makes the removed variants unreachable
rather than misparsed. `PlayItems` is retained as an internal control-flow
vehicle (the resolved-play re-entry from `PlaybackIntentAction::Play`), decoupled
from the wire: it no longer appears as a `CtrlCmd` variant. Future queue
additions remain additive via the existing capability-string rule.
