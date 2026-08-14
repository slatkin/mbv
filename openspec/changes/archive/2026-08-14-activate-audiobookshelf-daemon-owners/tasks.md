## 1. Daemon Admission Gate

- [x] 1.1 In the daemon submission predicate, add the ABS-admission check: `audiobookshelf_runtime.is_some()` AND the submitting client negotiated `abs-queue`; reject with a visible error when either condition fails.
- [x] 1.2 Cover the gate: ABS episode admitted when both conditions hold; rejected with `audiobookshelf_runtime` absent; rejected when client lacks `abs-queue`; non-ABS items are unaffected.

## 2. Player Context And Source Preparation

- [x] 2.1 At daemon ABS slot playback start, construct `AudiobookshelfPlayerContext::new(generation, setup, api_key, device_id)` from `audiobookshelf_runtime` and pass it to `prepare_source`, mirroring bare mode.
- [x] 2.2 Verify direct-URL and HLS paths are reachable through the daemon owner with the constructed context (existing source-preparation tests suffice; add a daemon-owner path test if direct coverage is absent).

## 3. Progress Routing

- [x] 3.1 Wire the daemon event loop's `AudiobookshelfProgressUpdate` receiver: on each acknowledged update, match the canonical Bound queue slot by `(library_item_id, episode_id)` and update position/completion.
- [x] 3.2 After the Bound queue update, call `broadcast_audiobookshelf_progress` with the acknowledged event; remove the `#[allow(dead_code)]` annotation.
- [x] 3.3 Cover: periodic sync update reflects in Bound queue slot and is broadcast to capable clients; completion marks the slot done; stale-generation update is dropped without queue or broadcast side effect.

## 4. Finalization On Setup Replacement And Disconnect

- [x] 4.1 Extend `reconcile_packaged_audiobookshelf` to trigger bounded ABS finalization of any live active session before dropping `audiobookshelf_runtime`; reuse the player's existing teardown coordination point.
- [x] 4.2 Cover: replacement with a different server finalizes active ABS session then purges Bound/persisted ABS slots; `--disconnect abs` finalizes active session, stops the queue, and purges ABS slots.

## 5. Stay-Alive Continuity

- [x] 5.1 Confirm that the existing daemon stay-alive path requires no changes: ABS playback and periodic sync proceed from the daemon owner loop after all clients exit, inheriting the same event-loop lifetime that governs Emby stay-alive.
- [x] 5.2 Add a test asserting that client exit does not trigger ABS finalization or Bound queue mutation when an ABS episode is active.

## 6. Verification

- [x] 6.1 Run `rtk cargo check -p mbv-core`, `rtk cargo check -p mbvd`, `rtk cargo check -p mbv`.
- [x] 6.2 Run `rtk cargo nextest run -p mbv-core` and `rtk cargo nextest run -p mbvd`.
- [x] 6.3 Run `rtk cargo clippy --workspace --all-targets` and `rtk cargo fmt --all -- --check`.
- [x] 6.4 Run `rtk make check-code-file-lines`; split any file that exceeds the 800-line cap.
- [x] 6.5 Run `openspec validate --strict --change activate-audiobookshelf-daemon-owners` and resolve any findings.
- [x] 6.6 Run `git diff --check` and confirm no whitespace errors.
