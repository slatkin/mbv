# Proposal

## Why

Split out of #815 (issue #846: https://github.com/slatkin/mbv/issues/846).
~338 function signatures return `Result<_, String>` and 259 `map_err` calls
collapse every failure into an opaque string, so callers cannot match on the
kind of failure (e.g. `mbvd` exit codes are derived with `error.contains(...)`)
and the underlying cause is lost. Now that #814 has split the workspace into
26 crates, each crate is a natural unit for introducing its own error type.

## What Changes

- One canonical domain error type per affected crate (struct wrapping a
  private kind enum, per `M-ERRORS-CANONICAL-STRUCTS`): `mbv-config`,
  `mbv-emby`, `mbv-cast`, `mbvd`, `mbv-core`, `mbv-remote-player`,
  `mbv-daemon`, `mbv-ui-model`, `mbv-player`, `mbv-feed`, `mbv-ctrl`,
  `mbv-visualizer`, `mbv-components`; extend the existing
  `AudiobookshelfError` instead of adding a second type.
- Convert all ~338 `Result<_, String>` signatures and 259 `map_err` sites to
  the domain types with `From` conversions (`M-FROM-ERROR`); `map_err` remains
  only for foreign errors needing context. No `anyhow`/`thiserror`/`eyre`.
- Replace `mbvd`'s `exit_code_for_error` string matching with error-kind
  predicates, preserving the exit-code mapping (restart=3, usage=2, other=1).
- Remove the lossy `From<String> for AudiobookshelfError` (collapses every
  failure to `connectivity`) and classify those sites with real kinds.
- **BREAKING**: public function signatures change in every affected crate;
  internal callers (mostly `src/`) are updated in the same change.

## Capabilities

### New Capabilities

- `domain-error-types`: workspace-wide contract for canonical domain error
  types — shape, conversions, kind-based matching, and the ban on
  stringly-typed errors in crate public APIs.

### Modified Capabilities

(none — user-observable behavior, including error text and exit codes, is
preserved; only the representation of failures changes.)

## Impact

- Affected code: 12 library/binary crates plus `src/` (largest consumers:
  `src/app/state/types/cast.rs` 39 sites, `mbv-config` 74, `mbv-emby` 42).
  Crates already clean (`mbv-queue`, `mbv-net`, `mbv-keybinds`,
  `mbv-images`, `mbv-ids`, `mbv-desktop`, `mbv-render`, `mbv-theme`,
  `mbv-text`, `mbv-ui-msg`, `mbv-ws`, `mbv-emby-model`) are untouched.
- Tests asserting on error text (`mbvd` exit-path tests) are rewritten to
  assert on kinds/exit codes.
- Gates per unit: `cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo nextest run -p <package>`, `cargo fmt`; final `rg` audit proves zero
  `Result<_, String>` in crate public APIs.
