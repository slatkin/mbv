## 1. Prerequisites

- [ ] 1.1 Confirm #513, #516, and #517 are applied; provider-specific activation is inert, queue admission is Service-aware, direct/HLS preparation and projection are verified, and the in-process owner remains ineligible.

## 2. Playback Lifecycle And Accounting

- [ ] 2.1 Replace the Emby-or-none reporting branch with a closed active-item lifecycle strategy for Emby, Audiobookshelf, and no server reporting.
- [ ] 2.2 Track Audiobookshelf setup generation, session ID, current position, duration, last acknowledgement, one in-flight request, and monotonic wall-clock time accumulated only while mpv is Playing.
- [ ] 2.3 Synchronize periodically and on pause/seek, account elapsed playing time up to each event, exclude paused time and seek distance, preserve wall-clock semantics at non-1.0 speed, and never retry an ambiguously dispatched interval.

## 3. Ordered Finalization

- [ ] 3.1 Implement one idempotent bounded finalization path that drains ordered reporting, sends final position/listening time, closes the session, and clears lifecycle state regardless of outcome.
- [ ] 3.2 Invoke finalization on EOF, stop, skip, selected-slot change, active removal, queue replacement, source failure, credential rejection, Service replacement/removal, run shutdown, and process teardown.
- [ ] 3.3 Prevent the next Audiobookshelf session from opening until prior finalization completes or exhausts its budget, while ensuring teardown cannot block indefinitely.

## 4. Enablement And Episode Actions

- [ ] 4.1 Enable Audiobookshelf admission only for the in-process bare-mode Player while its current playback context, prepared-source boundary, and reporting lifecycle are all available; keep every ctrl owner ineligible.
- [ ] 4.2 Resolve the selected downloaded episode and progress snapshot through #513's provider-specific handler and construct the provider-native QueueItem without exposing credentials or lifecycle state to browse code.
- [ ] 4.3 Wire ordinary play to canonical slot selection/creation and eligible-owner submission, and ordinary enqueue to Composed or eligible Bound mutation without opening a session or starting playback; preserve inert behavior for non-episode and unavailable rows.

## 5. Progress Reconciliation

- [ ] 5.1 Emit acknowledged provider-qualified progress with setup generation and apply it to matching canonical queue slots and Audiobookshelf browse/filter state only while that generation remains current.

## 6. Verification

- [ ] 6.1 Verify listening-time accounting, ordered finalization, generation races, every lifecycle exit, unsupported-owner behavior, direct/HLS play, enqueue, authoritative resume, pause, seek, speed, completion, and local progress refresh against Audiobookshelf 2.36.
- [ ] 6.2 Run focused Audiobookshelf/playback/App nextest suites, `cargo check -p mbv-core`, `cargo check -p mbv`, formatting, clippy, `make check-code-file-lines`, strict OpenSpec validation, and diff checks.
- [ ] 6.3 Audit the completed milestone to confirm it adds no Audiobookshelf ctrl transport, daemon credential handling, Socket.IO, audiobook model, remote routing, or cross-Service browse abstraction.
