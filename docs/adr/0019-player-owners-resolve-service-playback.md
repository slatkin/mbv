# Player Owners Resolve Service-Backed Playback

> **Status (2026-09-30):** The local Owner process is now the only local Player-owner host; Audiobookshelf eligibility is stated in the amendment below. See [ADR 0030](0030-owner-process-is-the-only-local-player-owner.md).

> **Amended (2026-08-14):** Audiobookshelf playback follows this decision: the owner resolves the source (direct or HLS, Bearer scoped to the direct file, bounded HLS readiness), projects only the active file into mpv while the canonical queue keeps all slots, and syncs and finalizes progress with monotonic wall-clock listening time. "Dormant in-process Audiobookshelf source machinery" no longer holds. Owner admission (`CONTEXT.md`; `daemon_admits` in `crates/mbv-daemon`) decides which Services a Player owner admits. The additive Audiobookshelf queue and progress capabilities in the `ctrl-protocol` spec decide which peers can carry Audiobookshelf items.

## Decision

An eligible Player owner resolves Service-backed media using Service setup and a
credential held in that owner's process, and owns the resulting provider
playback lifecycle and progress reporting. Clients submit provider-qualified,
secret-free QueueItems; ctrl, queue persistence, UI events, and logs do not
carry Service credentials, authenticated headers, provider playback-session
IDs, or resolved expiring sources. Client attachment grants a control
relationship, not a Service identity or login.

Source preparation happens just in time at the active-slot boundary. The
owner's canonical queue remains authoritative. A Playback run may mirror its
slots eagerly in mpv, but once a lifecycle-backed source is needed it may
materialize only the active canonical slot; mpv playlist coordinates then stay
projection-local.

The interactive in-process owner receives authenticated Emby context when that
Service becomes ready, and dormant in-process Audiobookshelf source machinery
follows the same owner-local boundary. Feed sources require no Remote Service
credential.

## Context

Stay-alive playback must outlive every disposable Client. Audiobookshelf direct
and HLS sources may be credentialed, expiring, and tied to one server-side
playback session; eagerly preparing inactive slots can create unused sessions
or replace the active same-device session. Letting a Client resolve sources or
send credentials over ctrl would make its lifetime part of playback correctness
and enlarge the secret boundary.

## Considered Options

- Resolve media in the Client and send URLs or credentials to the owner.
- Proxy provider media through the Client.
- Eagerly resolve every queue slot and treat mpv's playlist as queue authority.
- Let the owner load its own Service credential, resolve only active media, and
  retain canonical queue authority (chosen).

## Consequences

An owner becomes eligible for a Service-backed QueueItem only when it can load
the required Service setup and credential. Provider lifecycle state remains
process-local and must be reconstructed by the owner rather than transported
from a Client. Migrating an existing owner to this boundary is a capability
change, not permission to send Service credentials through ctrl.
