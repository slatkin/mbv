## Why

After PR #386 enforced an 800-line maximum on governed tracked files, 37 files remain between 500–759 lines. Several are approaching the ceiling (`mod.rs` at 759, `input_power_music_track_navigation_tests.rs` at 756, `ctrl.rs` at 734). Proactively evaluating these files for cohesive extraction boundaries prevents emergency splits when the next feature pushes them over 800 lines, and may improve maintainability now by isolating responsibilities that have already drifted.

## What Changes

- Audit each of the 37 governed tracked files currently over 500 lines for cohesive extraction boundaries
- For files with clear extraction opportunities: split into smaller modules while preserving behavior, test coverage, and module/privacy semantics
- For files without clear boundaries: document the decision to keep them intact
- Keep every resulting governed file at or below 800 lines
- Run `cargo fmt`, `cargo check`, `cargo clippy`, `cargo test`, and `make check-code-file-lines` for any implemented split

## Capabilities

### New Capabilities

None. This is internal refactoring with no new behavior.

### Modified Capabilities

None. All existing capabilities remain unchanged; this work only reorganizes code within existing modules.

## Impact

- **Source code**: Up to 37 files in `src/app/`, `src/app/render/`, and `crates/mbv-core/src/` may be split or reorganized. All changes are structural (module extraction, `use` statement updates, `mod` declarations). No logic changes.
- **Tests**: Test files may be split by functional area. Test identities and assertions preserved.
- **Build system**: No changes to `Cargo.toml`, `Makefile`, or CI workflows.
- **Dependencies**: None.
- **Risk**: Low. All splits preserve behavior and are verified by the existing test suite and line-limit checker.
