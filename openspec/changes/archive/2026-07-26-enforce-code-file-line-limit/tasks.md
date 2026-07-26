## 1. Establish the repository-wide size gate

- [x] 1.1 Confirm the exact governed-file classifier and exclusions, including source extensions, `Makefile`, `PKGBUILD*`, `.githooks/*`, docs/config paths, and the current 13-file baseline.
- [x] 1.2 Add a repository-local code-file size checker that reports every governed file over 800 lines and returns a failing status for violations.
- [x] 1.3 Add the canonical `make check-code-file-lines` target that invokes the single checker without duplicating its logic.
- [x] 1.4 Add a dedicated CI workflow for pull requests and pushes to `main`, then configure its check as required for merge.
- [x] 1.5 Document the threshold, exact classifier, exclusions, canonical command, CI status, and source-language update rule in `docs/rules/coding-practices.md`.

## 2. Split remaining `src/app` violations

- [x] 2.1 Survey `src/app/render/tests.rs` (2,738 lines), preserve its test inventory and fixture paths, and split it into cohesive test modules at 800 lines or fewer.
- [x] 2.2 Survey `src/app/actions_tests.rs` (1,752 lines), split its tests by subsystem without changing names or behavior, and verify the pre/post test inventory.
- [x] 2.3 Survey `src/app/input_power_music_track_focus_tests.rs` (1,416 lines), split its tests into focused files while preserving private-item and fixture access.
- [x] 2.4 Split `src/app/render/chrome.rs` (1,308 lines) by cohesive rendering responsibility and update the render module declarations without arbitrary line slicing.
- [x] 2.5 Split `src/app/library_browse_actions.rs` (897 lines) along existing browse/load/pagination responsibilities, keeping all resulting files at or below 800 lines.
- [x] 2.6 Split `src/app/input_mouse.rs` (844 lines) along stable mouse-input responsibilities and preserve all callers and visibility semantics.

## 3. Split `mbv-core` violations

- [x] 3.1 Inventory inline tests, fixtures, public types, and module consumers for each oversized `mbv-core` file before moving production code.
- [x] 3.2 Record boundary maps, API/serialization invariants, and per-file lane order for the six core violations.
- [x] 3.3 Extract/split `crates/mbv-core/src/player.rs` (4,281 lines) into cohesive playback components, including its large inline test block, and verify the public API and test behavior.
- [x] 3.4 Extract/split `crates/mbv-core/src/api.rs` (2,635 lines) by API/domain responsibility and verify serialization and request behavior.
- [x] 3.5 Extract/split `crates/mbv-core/src/config.rs` (2,058 lines) by configuration concerns, including its inline tests, and verify parsing/default behavior and persisted formats.
- [x] 3.6 Extract/split `crates/mbv-core/src/daemon.rs` (1,783 lines) by daemon lifecycle/control responsibilities and verify protocol behavior.
- [x] 3.7 Extract/split `crates/mbv-core/src/remote_player.rs` (1,269 lines) by remote playback responsibility and verify transport/playback semantics.
- [x] 3.8 Split `crates/mbv-core/src/playback_queue.rs` (845 lines) at a cohesive boundary, extracting tests if required, and verify queue ordering/mutation behavior.
- [x] 3.9 Reconcile shared `mbv-core` module declarations and verify no privacy, import, test-discovery, or public API regressions remain.

## 4. Split the Lua script

- [x] 4.1 Survey `scripts/mbv.lua` (2,545 lines) for stable mpv command, event, UI, and utility boundaries before editing.
- [x] 4.2 Record Lua module boundaries, dependency/load order, and mpv-facing entrypoints before extraction.
- [x] 4.3 Extract cohesive Lua modules while preserving mpv-facing command names, event handlers, state flow, and load order.
- [x] 4.4 Update `Cargo.toml`, release tarball assembly, and both PKGBUILD packaging paths so every resulting Lua module is shipped and loadable.
- [x] 4.5 Validate the split with available Lua syntax/load checks and a focused mpv/runtime smoke test.

## 5. Final verification and review

- [x] 5.1 Run the repository-wide checker and confirm zero governed files exceed 800 lines.
- [x] 5.2 Run `cargo fmt --all -- --check`.
- [x] 5.3 Run `cargo check --workspace --all-targets`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, and `cargo test --workspace`.
- [x] 5.4 Run release test/build checks matching CI and verify the packaged Lua modules are present.
- [x] 5.5 Compare normalized Rust test identities for every test-only move, ignoring module-path changes from extraction.
- [x] 5.6 Review the complete diff for arbitrary fragmentation, accidental API changes, dropped code, incomplete packaging, and undocumented classifier gaps.
- [x] 5.7 Record the final governed-file inventory and verification results in the implementation handoff.
