# Tasks

## 1. Owner boundary

- [ ] 1.1 Add a capability-gated, correlated idle queue-load command/result and a source-only update carrying owner queue lineage in `crates/mbv-core/src/ctrl.rs` and remote-player command handling; verify round-trip/unsupported-peer tests reject without staging a private queue.
- [ ] 1.2 Implement owner stop/finalize-before-replace for accepted Stay-alive idle loads in `crates/mbv-core/src/daemon_control.rs` and Player lifecycle, reusing admission and permitting empty loads; verify mocked owner tests show one stopped snapshot, no playback start, rejection preserving the old run, and late old-run events ignored.
- [ ] 1.3 Make the owner mint replacement lineage and apply guarded source-only updates without replacing slots or changing playback; verify tests for successful Save As propagation and rejection of a delayed update after another Client's replacement.

## 2. Client authority

- [ ] 2.1 Send non-autostart playlist loads to the home Stay-alive owner at load time from `src/app/queue_actions.rs`/`queue_scope.rs`, with no local replacement or Loaded claim until acceptance; verify focused Client tests for loading during playback, empty load, rejection, and disconnect. Leave Bare, Direct remote control, Session watch, cast, and packaged `mbvd` paths unchanged.
- [ ] 2.2 Reconcile Stay-alive queue slots, source and playback solely from owner snapshots in `src/app/player_event.rs` and queue projection, retiring generation-fence authority there; verify two mocked Clients see the same stopped replacement, a later Play uses the owner slot, and old queue indices never highlight the replacement.
- [ ] 2.3 Route successful Save As source changes through the guarded owner source update and keep playlist mutation lineage checks, without resubmitting contents; verify an in-flight Save As cannot rename another Client's newly loaded queue.

## 3. Continuity and verification

- [ ] 3.1 Distinguish a deliberately cleared empty Stay-alive queue from a never-seeded owner in cold adoption/bootstrap, and persist only confirmed owner state; verify attach/reconnect tests cannot resurrect a stale saved playlist after an empty load, while a genuinely cold owner still adopts its saved queue.
- [ ] 3.2 Review queue/snapshot owner invariants and update `docs/invariants/`, sync the two delta specs into `openspec/specs/`, and update `CONTEXT.md` only if new domain terms were introduced; verify strict OpenSpec validation and no stale conflicting requirement remains.
- [ ] 3.3 Verify the coherent flow with `cargo check -p mbv-core`, `cargo check -p mbv`, focused `cargo nextest run -p mbv-core` and `cargo nextest run -p mbv`, `cargo fmt --all -- --check`, and `cargo clippy --workspace --all-targets -- -D warnings`; manually check two attached TUIs loading while one video plays (no live automated test) and record the observed result before archiving.
