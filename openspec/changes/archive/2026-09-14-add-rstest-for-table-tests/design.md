## Context

See `proposal.md` for motivation. The constraints that shape the approach:

- **Test shape today.** Every test in the workspace is a plain `#[test]`. There is no test-framework dependency at all: the only dev-dependencies in the workspace are `uuid` (root) and `flume` (`mbv-core`, for feeding synthetic mDNS events). Test files are wired two ways — a `*_tests.rs` companion pulled in with `include!` inside a `#[cfg(test)] mod tests` block (35 sites), or an inline `#[cfg(test)] mod` at the bottom of the production file (91 files).
- **The measured target.** Four fixture-varying families, ~20 tests, all mechanically shaped (see the table in `proposal.md`). Each body is one or two lines; only the input and expected value differ.
- **Dependency convention.** Shared versions are declared in `[workspace.dependencies]` and referenced as `foo.workspace = true` by members.
- **Policy constraints that bear on this change.** Tests are mocks-only (no live externals); async is edge-only in this repo (`tokio` confined to `src/mpris.rs`/`zbus`), so a test framework that drags in an async runtime is a poor fit; CI gates are `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo nextest run --release --test-threads=4`.
- **Skip-specs justification.** No requirement in `openspec/specs/` describes test conventions; the repo's test policy lives in `AGENTS.md`. Nothing observable changes, so `.openspec.yaml` sets `skip_specs: true`.

## Goals / Non-Goals

**Goals:**

- Make the non-duplicated test form the cheapest form for fixture-varying families, so the debt stops refilling itself.
- Preserve every existing assertion and keep every case individually identifiable in failure output and in `cargo nextest list`.
- Convert the four measured families only, keeping each file's conversion a self-contained reviewable change.
- Record the convention where agents will actually read it (`AGENTS.md`), so it survives beyond this change.

**Non-Goals:**

- No repo-wide sweep. The other 13 measured families, and any family not measured, are out of scope.
- Adopting any other test framework. `proptest` is a separate evaluation (#711).
- Adopting rstest features beyond `#[case]` and `#[fixture]`: specifically **not** `#[values]`, `#[files]`, `#[timeout]`, `#[once]`, `#[awt]`, or async-test support. `#[values]`/`#[files]` generate tests from data, which is the same thin-test failure mode in a different key.
- Adding coverage. This change re-packages tests; it does not change what is asserted, nor does it touch the coverage gaps the audit identified in `crates/mbv-core/src/player_run_commands.rs` and friends.
- Touching production code.

## Decisions

**D1 — `rstest`, not `test-case` or a hand-rolled macro.** The target families need both axes: parameterized `#[case]` tables *and* shared fixtures replacing the ad-hoc `make_*` helpers called out in the issue. `test-case` covers cases only, so it would leave fixtures mid-air and invite a second dependency later; a hand-rolled macro is a bespoke tool to maintain and would violate the repo's no-bespoke-tooling stance. rstest covers both, is widely used, and desugars to ordinary `#[test]` functions, so nextest naming, filtering, and process isolation are unaffected.

**D2 — declared once in `[workspace.dependencies]`, consumed as a per-package dev-dependency.** Matches the existing convention and keeps one version for the workspace. Both `mbv` (root) and `mbv-core` need it: `backdrop.rs` lives in `src/app/render/` (root package), the other three families are in `mbv-core`.

**D3 — evaluate `default-features = false` first.** rstest's default features include `async-timeout`. This repo is sync-first with `tokio` confined to the edge, so pulling an async-timeout path into every test build is an avoidable cost. Start from `default-features = false` and confirm `#[case]`/`#[fixture]` still compile; fall back to default features only if something needed disappears. Verify with `cargo tree -p rstest` before committing the dependency.

**D4 — named cases are mandatory.** Positional cases generate `fibonacci_test::case_1`, `case_2`, which loses the diagnosability the issue explicitly requires. Named cases (`#[case::zero_base_case(0, 0)]`) generate `fibonacci_test::case_1_zero_base_case` — the description survives into the test identifier, so a failure and a nextest filter still name the scenario. Every converted family carries case names derived from the test names it replaces (e.g. `#[case::malformed_json("not json")]`), so the mapping from old test name to new case is mechanical and reviewable.

**D5 — one file per commit, assertion-preserving.** Each conversion is committed separately (socket family, parse families, backdrop family) so a reviewer can check the assertion set is unchanged without diffing three files at once. The check is explicit rather than by eye: the number of generated test cases must equal the number of tests removed, and `cargo nextest list` output for the family must show one case per former test.

**D6 — the convention goes in `AGENTS.md`, not a spec.** Specs in this repo describe system behaviour; test packaging is policy, and the repo's existing test rules (mocks-only, no forced sleeps, no live tests) already live in `AGENTS.md`. Adding the line there puts it in the same place an agent already looks, and it is what makes the scope guard durable: fixture-varying families use `#[case]` tables, `#[case]` is never a mechanism for generating many thin tests.

**D7 — fixtures only where a helper is already shared.** Convert a `make_*` helper into `#[fixture]` only when two or more tests in the same file already share it. Inventing fixtures for one-off setup would add indirection without removing duplication.

**D8 — the backdrop family converts the whole `dim_*` shape, not just the measured 3.** In `src/app/render/components/backdrop.rs` the measured `dim_rgb_*` family (3 tests) is a subset of seven identical-shape tests in the same module (`dim_white_*`, `dim_black_*`, `dim_reset_*`, `dim_rgb_*` ×3, `dim_indexed_*`) — all `assert_eq!(dim(input), expected)`. This is the only target file where the measured family sits inside a larger uniform set. Converting three of seven would leave a module that is half table, half copies, which is worse than either end state, so the whole `dim_*` set converts. The task list still counts it against the same file and uses the same case-count check against the recorded baseline (7 cases vs 7 removed tests).

## Risks / Trade-offs

- **A table hides an assertion.** The realistic failure mode: folding eight tests into one body accidentally drops a check, and the suite still passes. → Named cases plus the D5 count check (cases generated == tests removed); reviewers read the frozen test names against the new case names.
- **Thin-test generation returns through the new door.** `#[case]` makes it cheap to emit many one-line cases. → D6's convention line, the Non-Goal on `#[values]`/`#[files]`, and the scope limit to families that already exist (convert, never generate).
- **Generated code trips the `-D warnings` clippy gate.** Proc-macro output can carry lints attributed to the macro. → Run the full gate in the same commit as each conversion; if a lint fires on generated code, prefer a narrow `#[allow]` with a comment over changing repo-wide lint config, and if neither works cleanly, that is grounds to reconsider the dependency rather than suppress broadly.
- **Dependency weight and build time.** rstest is a proc-macro crate; test builds get slower. Release builds are unaffected (dev-dependency). → Measure `cargo nextest` wall time before and after; syn/quote/proc-macro2 are already in the tree via `serde_derive`, so the marginal cost is the crate itself. `default-features = false` (D3) trims the async path.
- **Formatting churn.** `cargo fmt` will reflow the converted tables. → Expected and accepted; formatting lands in the same commit as the conversion so it is reviewable in context.
- **Scope creep into the other 13 families.** → Proposal and Non-Goals fix the set at four; anything else becomes a follow-up, not a detour.

## Migration Plan

1. Add the dependency (workspace declaration + two dev-dependency entries), verify `cargo tree -p rstest` and a warm `cargo nextest run -p mbv-core`; commit alone so the dependency change is separable.
2. Convert `crates/mbv-core/src/audiobookshelf_socket.rs` (the 8× `*_returns_none` family); commit.
3. Convert the two `crates/mbv-core/src/api_tests_parsing.rs` families; commit.
4. Convert `src/app/render/components/backdrop.rs` (the 3× `dim_rgb_*` family); commit.
5. Add the convention line to `AGENTS.md`; commit.
6. Gates after every step: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo nextest run -p mbv` / `-p mbv-core`.

**Rollback**: revert the affected commits. The dependency is dev-only, so no runtime, release, or packaging impact; reverting is a plain `git revert` with no data migration.

## Open Questions

- Which of the remaining 13 measured families to convert next, and whether to schedule them or leave them opportunistic as those files are touched. Deferrable: it changes neither this plan's approach nor its tasks.
