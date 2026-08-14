## 1. Activate Owner Emission

- [ ] 1.1 Invoke `broadcast_audiobookshelf_progress` at the daemon owner's acknowledged-progress point (accepted periodic sync and final completion), passing episode identity, acknowledged position/completion, and current setup generation; remove the `#[allow(dead_code)]` marker.
- [ ] 1.2 Confirm emission stays capability-gated per connection and carries no API key, Authorization header, resolved URL, or `sessionId` (extend the existing wire-field assertion test for the emitted event).

## 2. Client Reconciliation

- [ ] 2.1 Replace the dormant `PlayerEvent::AudiobookshelfProgress` handler with reconciliation that converts the ctrl event into the shared apply path used by `LibEvent::AudiobookshelfProgressAcknowledged` (generation gate via `audiobookshelf_runtime.accepts`, queue-slot match by `(library_item_id, episode_id)`, `apply_progress`, browse-progress write, `save_queue_state`).
- [ ] 2.2 Ensure the daemon and bare entry points share one reconciliation body (no duplicated logic); a no-match event is a no-op and a superseded-generation event is dropped.

## 3. Browse Reconciliation

- [ ] 3.1 Verify the shared apply path updates every `audiobookshelf_browse` progress map and re-evaluates episode filters (Unplayed) for the current generation when driven by a daemon event, and leaves browse state unchanged for a superseded generation.

## 4. Live-Queue Adoption

- [ ] 4.1 Prove a later capable client adopts the daemon's live Audiobookshelf queue, active slot, status, and last-acknowledged progress on attach, without overwriting daemon authority from a saved local/shared snapshot, and reconciles browse state on adoption.
- [ ] 4.2 Prove an incapable peer attaching to an Audiobookshelf-holding owner receives no Audiobookshelf QueueItem variant and retains every previously supported queue behavior.

## 5. Continuity Verification

- [ ] 5.1 Prove owner playback, periodic synchronization, and bounded finalization continue after every attached client exits, and that emission resumes to a later capable client without restarting the session.
- [ ] 5.2 Prove mixed-version and multi-client coherence: only capability-negotiated clients receive Audiobookshelf progress; every peer keeps its existing state stream.
- [ ] 5.3 Cover the lifecycle matrix for daemon-owned Audiobookshelf podcasts: direct and HLS resolution, resume, pause, seek, natural completion, no-client operation, reattachment, and explicit daemon shutdown finalization.

## 6. Gate

- [ ] 6.1 Run `rtk cargo check -p mbv-core`, `rtk cargo check -p mbv`, `rtk cargo check -p mbvd`, `rtk cargo nextest run -p mbv-core -p mbv`, `rtk cargo fmt --all -- --check`, `rtk cargo clippy --workspace --all-targets`, `rtk make check-code-file-lines`, strict OpenSpec validation, and `git diff --check`.
