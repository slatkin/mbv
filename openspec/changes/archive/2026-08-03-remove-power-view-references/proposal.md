## Why

ADR 0013 and the rename plan (2026-07-24) established that "Power View" is a legacy term — it is now the only view. The main rename (commit `b51cb82`) covered the queue-side/library-side naming collision but deliberately left other `power_`-prefixed identifiers untouched. This change finishes the job: every remaining reference to "Power View", "power view", "power-view", and "power_view" in source code and docs should be replaced with layout-independent or domain-appropriate names.

## What Changes

- **Doc comments and module docs** (~100+ references across ~25 source files): Replace "Power View" qualifiers with domain names (queue, library, panel, main) or drop them entirely since there is only one view.
- **User-facing status strings** (e.g. `"Power view width: ... cols"` in `input_queue_keys.rs` and assertions in `tests_queue_scope.rs`): Update to use the new vocabulary.
- **Test function names** (~20+ test fns with `power_view_*` names across `input_power_*_tests.rs`, `tests_queue_scope.rs`, `tests_podcast.rs`, `actions_tests_*.rs`, `render/tests_*.rs`): Rename to drop the `power_view` qualifier.
- **Docs** (~40 references across 5 files): Update ADR 0013 title/body, the rename plan, and other plan docs that still mention "Power View". ADR 0009 needs an amendment note.

## Capabilities

### New Capabilities

None. This is a pure rename/cleanup — no new behavior.

### Modified Capabilities

None at the spec level. All changes are internal naming and documentation; no user-facing capability requirements are changing.

## Impact

- **Source code**: ~25 files in `src/app/` and `src/mpris.rs` touched. All changes are mechanical renames in comments, doc strings, status messages, and test function names. No logic changes.
- **Tests**: ~20+ test functions renamed. Assertions on user-facing strings updated.
- **Docs**: 5 files in `docs/` updated. ADR 0013 may warrant a title rename or a clarifying amendment.
- **Dependencies**: None.
- **Risk**: Low. This is a text-only change with no behavioral impact. `cargo build` and `cargo test` must remain green.
