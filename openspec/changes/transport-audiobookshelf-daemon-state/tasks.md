## 1. Prerequisites And Wire Contract

- [ ] 1.1 Verify #523 and the completed #515 implementation are present; stop rather than recreating either prerequisite in this change.
- [ ] 1.2 Add additive Audiobookshelf queue and progress capability constants, hello advertisement, compatibility derivation, and per-connection support state without changing `CTRL_PROTOCOL_VERSION`.
- [ ] 1.3 Add the redacted provider-qualified acknowledged-progress ctrl payload and event, including identity, position/completion, and setup generation only.

## 2. Capability-Gated Queue Transport

- [ ] 2.1 Centralize owner queue projection for a connection so Audiobookshelf slots are included only when that peer negotiated queue support and projected active coordinates remain coherent.
- [ ] 2.2 Route initial state, mutation/rejection snapshots, track-change state, and reconnect broadcasts through the capability-aware projection.
- [ ] 2.3 Reject inbound unified adopt, replace, append, and play operations containing Audiobookshelf items when the submitting peer did not negotiate their transport.
- [ ] 2.4 Preserve the daemon's one canonical internal queue and keep its existing Audiobookshelf owner-admission rejection after successful transport negotiation.

## 3. Dormant Progress Plumbing

- [ ] 3.1 Add a daemon-side provider-progress event path and per-connection fan-out that sends Audiobookshelf progress only to peers that negotiated it.
- [ ] 3.2 Add remote Player/client decoding and delivery plumbing for the provider progress event without applying it to queue or browse state yet.
- [ ] 3.3 Add serialization and diagnostic guards proving API keys, Authorization headers, resolved URLs, and playback session IDs never enter queue or progress wire state.

## 4. Verification

- [ ] 4.1 Cover mixed-version initial snapshots, later broadcasts, reconnects, and inbound mutations with capable and older unified peers attached simultaneously.
- [ ] 4.2 Cover capability advertisement as static protocol support and prove daemon Audiobookshelf submission remains visibly ineligible with no Bound queue mutation or source preparation.
- [ ] 4.3 Run targeted ctrl/daemon/remote-player tests, `cargo check -p mbv-core`, `cargo check -p mbv`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `make check-code-file-lines`, strict OpenSpec validation, and `git diff --check`.
