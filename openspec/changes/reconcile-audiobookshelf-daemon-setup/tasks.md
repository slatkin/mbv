## 1. Persisted Setup Revision And Owner Context

- [ ] 1.1 Add `revision: u64` with serde default `1` to `AudiobookshelfSetup` and bump it in `persist_audiobookshelf_setup_and_secret`, `replace_audiobookshelf_setup_and_secret`, and `remove_audiobookshelf_setup_and_secret_with_owned_state` seam callers; keep old configs valid.
- [ ] 1.2 Add `AudiobookshelfOwnerContext` holding loaded setup, API key (runtime-only), stable `device_id()`, in-memory `SetupGeneration`, and persisted `revision`; add `audiobookshelf: Option<...>` to `DaemonStartupContext` loaded Service-independently (no authentication, absent/incomplete → `None`).
- [ ] 1.3 Cover the revision and owner-context types with targeted core tests (initial/repair/replacement revision advance, absent-setup → `None`, credentials never in the context's serialized form).

## 2. Daemon Reconciliation

- [ ] 2.1 Add `reconcile_packaged_audiobookshelf` that rereads owner storage, compares persisted revision, advances the generation, and installs or drops the context; return `ServiceSetupApplied` or `ServiceSetupRejected` with the existing reason set.
- [ ] 2.2 Extend the `ApplyServiceSetup` handler to accept `ServiceKind::Audiobookshelf` and widen `owner_admin_transport_allowed` to `CtrlTransport::Local` for both `DaemonRole::Local` and `DaemonRole::Packaged`, keeping TCP and cross-owner paths rejected.
- [ ] 2.3 Prove Audiobookshelf admission and playback stay disabled after a successful reconciliation (no Bound queue mutation, no source preparation).

## 3. Packaged Administration

- [ ] 3.1 Add `mbvd --connect abs`: local interactive prompt for server URL and hidden API key, `validate_setup_bounded` (`GET /api/me`) before commit, transactional commit via the existing persist/replace seams, and reconcile a running owner with `ApplyServiceSetup { kind: Audiobookshelf, revision }`.
- [ ] 3.2 Add `mbvd --disconnect abs`: no confirmation, durable removal of setup/secret/Audiobookshelf-owned state, explicit credential-removal reporting, and reconcile a running owner; report restart required and possible in-memory key retention when reconciliation fails.
- [ ] 3.3 Match Emby connect's exit codes (`0`/`1`/`2`/`3`) and interactive/usage rejection for both subcommands; add an `abs`-scoped administration lock.

## 4. Bare Mode Applies Changes To A Running Local Daemon

- [ ] 4.1 After bare-mode Audiobookshelf setup, repair, replacement, and removal commit, signal a running same-user Local daemon with `ApplyServiceSetup { kind: Audiobookshelf, revision }` when one is reachable.
- [ ] 4.2 Preserve the durable commit and report restart required when live reconciliation is unavailable; never claim the change is active in the daemon without acknowledgment.

## 5. Verification

- [ ] 5.1 Cover packaged and Local-daemon reconciliation: matching revision applies, mismatched returns `RevisionMismatch`, unreadable storage returns `StorageUnavailable`, TCP and cross-owner submissions are rejected without state change.
- [ ] 5.2 Cover `mbvd --connect abs` and `--disconnect abs` end-to-end including failed candidates preserving working setup and disconnect leaving no setup/secret/owned state.
- [ ] 5.3 Run `cargo check -p mbv-core`, `cargo check -p mbv`, `cargo check -p mbvd`, `cargo nextest run -p mbv-core`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets`, `make check-code-file-lines`, strict OpenSpec validation, and `git diff --check`.
