# Design

## Context

Measured at HEAD (excl. `openspec/**`): 338 `Result<_, String>` sites in 64
files, 259 `map_err` calls, zero `Box<dyn Error>`. Distribution: `src/` 107
(39 in `src/app/state/types/cast.rs` alone), `mbv-config` 74, `mbv-emby` 42,
`mbv-cast` 22, `mbvd` 21, `mbv-core` 17 (all in one example file),
`mbv-remote-player` 16, `mbv-daemon` 10, `mbv-ui-model` 7, `mbv-player` /
`mbv-feed` / `mbv-ctrl` 5 each, `mbv-audiobookshelf` 4, `mbv-visualizer` 2,
`mbv-components` 1. Twelve crates are already clean and are untouched.

The surviving pattern to copy is `AudiobookshelfError`
(`crates/mbv-audiobookshelf/src/lib.rs:113`): struct wrapping a kind enum,
hand-written `Display` + `std::error::Error` + `From`. Its `From<String>`
collapses everything to `connectivity` — the lossy conversion this change
removes. `mbvd`'s `exit_code_for_error` (`crates/mbvd/src/main.rs:567`)
matches on message substrings — the one caller that proves why kinds must
be matchable. Constraints: `M-ERRORS-CANONICAL-STRUCTS`, `M-FROM-ERROR`,
AGENTS.md (custom domain error types, no `anyhow`/`thiserror`/`eyre`,
800-line file cap, hermetic unit tests, no lint suppression).

## Goals / Non-Goals

**Goals:**

- Every affected crate exposes canonical error type(s); all 338 signatures
  and 259 `map_err` sites converted; `mbvd` exit codes kind-driven.
- Each unit leaves the tree compiling with gates green, so units land
  independently in dependency order.

**Non-Goals:**

- No rewording of user-visible messages and no new failure behavior (spec:
  preserved text and exit codes).
- No new error kinds for failures that do not exist yet; kinds mirror the
  `map_err`/`.to_string()` sites found, nothing speculative.
- No backtrace fields: `Backtrace::capture()` is a per-error cost the
  standard marks optional-in-practice (empty unless `RUST_BACKTRACE` is set),
  and none of the existing types carry one. Revisit if a debugging need
  arises; do not pay the boilerplate now.

## Decisions

- **Shape: struct + private kind enum + `is_*()` predicates** (standard's
  future-proofing rule). Alternative — public kind enum (what
  `AudiobookshelfError`'s `pub class` does today): rejected, because adding
  a variant would then break every downstream `match`. New types keep the
  enum private; `AudiobookshelfError` keeps its `pub class` field (renaming
  it would churn its callers for no behavioral gain) but new kinds are added
  as enum variants with covering predicates.
- **Every error type exposes `kind_name(&self) -> &'static str`.** It
  returns a stable, low-cardinality, crate-qualified dotted name (e.g.
  `"config.parse"`, `"emby.timeout"`) for the OTel `error.type` log field
  that structured logging (#845, `M-LOG-STRUCTURED`) records alongside
  `error.message` (`Display`). A name, not the enum, so the private-kind
  rule holds; qualified, so `io` kinds from different crates stay
  distinguishable in logs. `AudiobookshelfError` gains it too. Renaming a
  kind name is a log-format change, not a refactor.
- **`From` conversions at the owning crate, `?` at call sites**
  (`M-FROM-ERROR`). `map_err` survives only where a foreign error needs
  context added at the boundary (e.g. which file failed to parse).
- **Cross-crate errors flow one way.** A crate converts only errors of crates
  it already depends on; `src/` formats domain errors via `Display` and never
  wraps them in new `String` errors. No crate gains a dependency to obtain an
  error type.
- **`mbvd` exit codes become kind predicates** (`is_restart_required()`,
  `is_usage_error()`), with the existing `contains`-based tests rewritten to
  construct each kind and assert the exit code. The `contains` mapping table
  in `main.rs` is deleted, not kept as a fallback.
- **Leaf-first unit order** (feed/ctrl/player/visualizer/components →
  config → emby → cast+`src` cast files → mbvd → core/remote-player/daemon/
  ui-model/audiobookshelf → remaining `src/`), so each unit's producers
  already expose domain types when its consumers are converted.
- **M-APP-ERROR relaxation declined.** The standard would permit
  `anyhow`-style errors in the `mbvd`/`src` binaries, but the issue and
  AGENTS.md ban them workspace-wide; domain types everywhere keeps one rule.
- **Display text frozen.** Each converted site keeps its exact current
  message string in the new type's `Display`, verified by the pre-existing
  tests plus the rewritten `mbvd` tests. Rewording is a separate change.

## Risks / Trade-offs

- [Risk] `mbv-config` (74 sites, 8 files) is the largest single unit and
  touches startup paths every service depends on → Mitigation: it goes
  second, while dependents still consume only its previously-String APIs
  through `Display`; converted alone with full `nextest -p mbv-config`.
- [Risk] A `Display` string drifts during conversion and a user-visible
  message changes silently → Mitigation: freeze rule above; `mbvd` kind
  tests pin every exit-code message; no test may be weakened to
  accommodate a reword (rewrite to kinds, not looser text).
- [Risk] Private-kind enums accumulate overlapping variants across crates
  (e.g. five `Io` kinds) → Mitigation: accepted; per-crate ownership is the
  point (standard explicitly prefers `DownloadError`/`VmError` over one
  global enum). Shared upstream conversions (`From<std::io::Error>`) live in
  each crate that needs them — a few lines each, not a shared error crate.
- [Risk] Units conflict on shared files if run in parallel → Mitigation:
  units own disjoint file sets except the final `src/` integration; run
  sequentially or verify disjointness before parallelizing.

## Migration Plan

Land units in leaf-first order, one commit per unit, each with that unit's
gates (`clippy --workspace --all-targets -D warnings`, `nextest` for touched
packages, `fmt`). No rollback needed beyond reverting a unit commit; units
are additive (new types) plus signature changes confined to the unit's
files. Final step re-runs the workspace-wide `rg` audit (zero
`Result<_, String>`) and the full gate suite before archive/sync.

## Open Questions

None — the per-site kind taxonomy is discovered during implementation from
the existing `map_err` strings, which tasks call out explicitly.
