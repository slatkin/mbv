## Why

`rstest` is now a workspace dev-dependency, but the completed #710 change used only its named `#[case]` tables for four audited families. The remaining fixture-varying duplication and the crate's zero `#[fixture]` uses leave the test-packaging convention only partly adopted.

## What Changes

- Convert only the fixture-varying families in the read-only candidate ledger derived from #697 that still meet the recorded selection rule to named `#[case]` tables.
- Convert a same-file shared `make_*` helper to an `rstest` `#[fixture]` only when at least two tests consume it and the fixture removes setup duplication without obscuring test-specific input or expectation.
- Preserve each existing assertion, test count, filterable case identity, mocks-only boundary, and sync-first dependency graph.
- Record the selected and deferred families, fixture decisions, and before/after test counts in this change so future file-by-file conversions remain deliberate.
- Do not add test cases, coverage targets, runtime dependencies, async features, `#[values]`, `#[files]`, `#[timeout]`, or `#[awt]`.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None.

This is test tooling and policy application only; it does not change observable product behavior. The change opts out of specs via `skip_specs: true`.

## Impact

- **Dependencies:** Reuses the existing dev-only workspace `rstest 0.27` dependency with `default-features = false`; no new dependency or feature is introduced.
- **Code:** Only Rust test modules and their test-only helpers may change. Production modules, runtime behavior, protocols, and public APIs are out of scope.
- **Policy:** Must follow the existing `AGENTS.md` named-`#[case]` convention and its guard against generating many thin tests.
- **Verification:** Every converted file retains a per-file gate; workspace verification checks count parity and confirms the rstest dependency graph remains free of an async runtime/timeout path.
