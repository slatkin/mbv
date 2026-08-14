## 1. Prerequisite And Live Contract

- [x] 1.1 Confirm #516 is applied, Audiobookshelf QueueItems round-trip correctly, Service-aware admission exists, and every owner still rejects Audiobookshelf playback.
- [x] 1.2 Capture sanitized Audiobookshelf 2.36 responses for direct play, forced-transcode play, session sync, session close, and representative authentication, server, and malformed failures.
- [x] 1.3 Verify direct/HLS loading, bounded REST-only playlist readiness, and ordinary seeking with the repository's mpv/libmpv integration; stop and revise the change if Socket.IO is required.

## 2. Playback API And Context

- [x] 2.1 Add minimal public playback-session, one-track audio source, source-method, and progress payloads backed by private decoders for the captured contract.
- [x] 2.2 Add bounded Bearer-authenticated create/sync/close methods with redacted failure classification, validating media/episode identity and one-track shape while safely joining direct/HLS paths.
- [x] 2.3 Add one persistent non-secret mbv device identifier and a runtime-only generation-tagged Audiobookshelf context for the in-process Player; clear it on rejection, replacement, and removal.

## 3. Prepared Sources

- [x] 3.1 Add a fallible owner-local prepared-source boundary carrying URL, per-file mpv options, authoritative start position, and optional provider lifecycle; adapt Emby/Feed without behavior changes and keep prepared state out of serializable commands, ctrl, persistence, and UI events.
- [x] 3.2 Implement Audiobookshelf direct preparation with a per-file Bearer option and HLS preparation without credentials using the validated bounded readiness policy.
- [x] 3.3 Install opened-session lifecycle state before validation/mpv load and close it on readiness, validation, load, or start failure; verify the next non-Audiobookshelf source has no stale header.

## 4. Owner-Driven Projection

- [x] 4.1 Add a one-way transition from eager projection to owner-driven active-file projection, retaining canonical active-slot identity and leaving exactly the active file in mpv.
- [x] 4.2 Route advance, skip, explicit selection, consume, append, remove, move, replace, and stop through canonical slot identity; inactive mutations must not create mpv entries.
- [x] 4.3 Treat mpv playlist position/count observations as projection-local so they cannot resize, reorder, or reposition the canonical queue, and map natural EOF through canonical advance policy.

## 5. Verification

- [x] 5.1 Verify sanitized decoding/failures, direct/HLS preparation, authoritative resume including finished reset, header isolation, failed-session cleanup, projection transition, every canonical queue mutation, and mpv observation isolation.
- [x] 5.2 Confirm Audiobookshelf submission remains unsupported and no ctrl, progress-reporting, Socket.IO, or audiobook behavior is present; run focused nextest suites, cargo checks, formatting, clippy, line-limit, strict OpenSpec, and diff checks.
