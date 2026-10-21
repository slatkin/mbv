# Tasks

## 1. Owner-output seam in `mbv-daemon` (design D1, D2)

- [x] 1.1 In `crates/mbv-daemon/src/run.rs`, remove `cast_connector` and `emby_session_output` from `DaemonRuntimeHooks`; add `LocalOutputHooks { cast_connector: CastConnector, emby_session_output: bool }`; add the private `OwnerOutputs` carrier and split the public run entry points so the packaged entry has no parameter that can carry a remote output, with one internal run body behind both (design D1, D2). Verify: `cargo check -p mbv-daemon -p mbvd -p mbv` passes, and the packaged entry point's signature mentions neither `CastConnector` nor an Emby-output flag.
- [x] 1.2 Update the two process boundaries: `src/local_daemon.rs` supplies `LocalOutputHooks` through the Local entry point exactly as it supplied the old fields; `crates/mbvd/src/main.rs` calls the packaged entry with no remote-output argument and its comment now states the boundary instead of the convention. Verify: `cargo check -p mbvd` and `cargo check -p mbv`; the existing daemon and `mbvd` tests still pass (`cargo nextest run -p mbv-daemon -p mbvd`).

## 2. Pin the derived behavior (design D3, D4)

- [x] 2.1 In `crates/mbv-daemon/src/tests/`, pin the packaged-role contracts: a packaged hello carries neither `playback-output` nor `emby-session-output`, and a `SelectPlaybackOutput` Cast or Emby-Session request to a packaged-role loop is rejected with `CommandRejected` and a stated reason while the output stays Local (spec scenarios "Packaged hello carries no remote-output capability" and "Request reaches mbvd"). Extend the existing cast-output/session test families; one test per contract, no new family. Verify: `cargo nextest run -p mbv-daemon`.
- [x] 2.2 In `src/app/tests/`, pin the Client contract: with an owner compatibility whose `supports_playback_output` and `supports_emby_session_output` are both false, the output-selection targets offer no Cast row and no Emby-remote row (spec scenario "Client connected to mbvd"). Extend the existing `panel_targets` / session-connect tests; if the gating already has a covering test for a capability-less owner, extend that one instead of adding a twin. Verify: `cargo nextest run -p mbv`.

## 3. Integration

- [ ] 3.1 Run the full gate: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --workspace`. Verify: all three pass.

## Workflow follow-up

- Run `make check-code-file-lines` before push.
- Sync the delta specs and archive the change when the user approves the PR for merge.
