# Player Owners Resolve Service-Backed Playback

> **Amended (2026-08-14):** Bare-mode Audiobookshelf playback is now active — source resolution (direct/HLS, Bearer scoped to direct file, bounded HLS readiness), active-file projection (canonical queue retains all slots, mpv contains only active materialized file), and progress sync/finalization with monotonic wall-clock listening time landed in milestone #515 (PRs #520-522). "Dormant in-process Audiobookshelf source machinery" no longer holds for bare mode. Local daemon on `main` now admits Emby and Feed (audio-only subset for audio-only owners) per `CONTEXT.md` Owner admission; it still does NOT admit Audiobookshelf — tracked in milestone #524 (issues #525-528: transport `audiobookshelf-queue` + progress, setup reconciliation, daemon-owner playback, stay-alive continuity). Audiobookshelf ctrl transport still absent on `main`; additive capabilities planned in `openspec/changes/transport-audiobookshelf-daemon-state/`. Initial daemon init filtered Audiobookshelf for unified peers (`crates/mbv-core/src/daemon_core.rs:738`). Packaged `mbvd` remains Emby-gated on `main` (`crates/mbvd/src/main.rs:117`) pending PR #529 tracking #523.

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
credential. The Service-independent Local daemon does not yet load Remote
Service playback context, Audiobookshelf ctrl transport is absent, and packaged
`mbvd` remains Emby-gated with legacy ctrl authentication pending its separate
migration.

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
